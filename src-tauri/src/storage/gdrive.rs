use super::{http_client, RemoteFile, RemoteFolder, RemoteTreeFile, StorageProvider};
use base64::Engine;
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::io::ReaderStream;

pub const REFRESH_TOKEN_KEY: &str = "gdrive_refresh_token";
pub const CLIENT_SECRET_KEY: &str = "gdrive_client_secret";

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const API: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD_API: &str = "https://www.googleapis.com/upload/drive/v3";
const SCOPE: &str = "https://www.googleapis.com/auth/drive.file";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";

fn b64url(data: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default = "default_expiry")]
    expires_in: u64,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DriveItem {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    mime_type: String,
    #[serde(default)]
    created_time: Option<String>,
    #[serde(default)]
    modified_time: Option<String>,
    #[serde(default)]
    size: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DriveListResponse {
    #[serde(default)]
    files: Vec<DriveItem>,
    #[serde(default)]
    next_page_token: Option<String>,
}

fn default_expiry() -> u64 {
    3600
}

/// Spustí OAuth 2.0 PKCE flow: otvorí prehliadač, počká na kód na localhost porte
/// a vymení ho za tokeny. Vráti (refresh_token, názov účtu).
pub async fn connect(
    app: &tauri::AppHandle,
    client_id: &str,
    client_secret: &str,
) -> Result<(String, String), String> {
    if client_id.trim().is_empty() {
        return Err("Najprv vlož Google OAuth Client ID v Nastaveniach.".into());
    }
    if client_secret.trim().is_empty() {
        return Err("Najprv vlož Google OAuth Client Secret v Nastaveniach.".into());
    }

    let mut verifier_bytes = [0u8; 48];
    rand::thread_rng().fill_bytes(&mut verifier_bytes);
    let verifier = b64url(&verifier_bytes);
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));
    let mut state_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut state_bytes);
    let oauth_state = b64url(&state_bytes);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("Nepodarilo sa otvoriť lokálny port: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let auth_url = url::Url::parse_with_params(
        AUTH_URL,
        &[
            ("client_id", client_id),
            ("redirect_uri", redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", SCOPE),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", oauth_state.as_str()),
        ],
    )
    .map_err(|e| e.to_string())?;

    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_url(auth_url.as_str(), None::<&str>)
            .map_err(|e| format!("Nepodarilo sa otvoriť prehliadač: {e}"))?;
    }

    let code = tokio::time::timeout(
        Duration::from_secs(300),
        wait_for_code(listener, &oauth_state),
    )
    .await
    .map_err(|_| "Prihlásenie vypršalo (5 min). Skús znova.".to_string())??;

    let http = http_client()?;
    let resp = http
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code.as_str()),
            ("code_verifier", verifier.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Výmena tokenu zlyhala ({status}): {body}"));
    }
    let token: TokenResponse = resp.json().await.map_err(|e| e.to_string())?;

    // Google má granulárny súhlas — užívateľ mohol nechať Drive checkbox nezaškrtnutý.
    if let Some(scope) = &token.scope {
        if !scope.contains("drive.file") {
            return Err("Prihlásenie prešlo, ale NEZAŠKRTOL si prístup k Disku Google. \
                Klikni Pripojiť znova a na Google obrazovke zaškrtni políčko o prístupe k súborom na Disku."
                .into());
        }
    }

    let refresh_token = token
        .refresh_token
        .ok_or_else(|| "Google nevrátil refresh token. Skús znova.".to_string())?;

    Ok((refresh_token, "Google Drive".into()))
}

async fn wait_for_code(listener: TcpListener, expected_state: &str) -> Result<String, String> {
    loop {
        let (mut stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 4096];
        let n = stream.read(&mut buf).await.map_err(|e| e.to_string())?;
        let request = String::from_utf8_lossy(&buf[..n]);
        let first_line = request.lines().next().unwrap_or_default();
        // GET /?code=... HTTP/1.1
        let path = first_line.split_whitespace().nth(1).unwrap_or("/");
        let full = format!("http://127.0.0.1{path}");
        let parsed = url::Url::parse(&full).map_err(|e| e.to_string())?;
        let mut code = None;
        let mut err = None;
        let mut returned_state = None;
        for (k, v) in parsed.query_pairs() {
            match k.as_ref() {
                "code" => code = Some(v.to_string()),
                "error" => err = Some(v.to_string()),
                "state" => returned_state = Some(v.to_string()),
                _ => {}
            }
        }
        let oauth_response = code.is_some() || err.is_some();
        let state_matches = returned_state
            .as_deref()
            .is_some_and(|state| state == expected_state);
        let body = if code.is_some() && state_matches {
            "<html><body style='font-family:sans-serif;background:#0f172a;color:#e2e8f0;display:flex;align-items:center;justify-content:center;height:100vh'><h2>&#10003; Prihl&aacute;senie hotov&eacute;. M&ocirc;&#382;e&#353; zavrie&#357; toto okno.</h2></body></html>"
        } else {
            "<html><body style='font-family:sans-serif'><h2>Prihl&aacute;senie zlyhalo.</h2></body></html>"
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;

        if oauth_response && !state_matches {
            continue;
        }
        if let Some(e) = err {
            return Err(format!("Google zamietol prihlásenie: {e}"));
        }
        if let Some(c) = code {
            return Ok(c);
        }
        // favicon a podobné požiadavky ignoruj, čakaj ďalej
    }
}

/// Skontroluje odpoveď Drive API; pri chybe vytiahne zrozumiteľnú správu z JSON tela.
async fn check_api(context: &str, resp: reqwest::Response) -> Result<reqwest::Response, String> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if body.contains("accessNotConfigured") || body.contains("has not been used in project") {
        return Err("Google Drive API nie je zapnuté v tvojom Google Cloud projekte. \
            Otvor https://console.cloud.google.com/apis/library/drive.googleapis.com, klikni Enable a skús o 2 minúty znova."
            .into());
    }
    let msg = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(String::from))
        .unwrap_or(body);
    Err(format!("{context} ({status}): {msg}"))
}

pub struct GDrive {
    client_id: String,
    client_secret: String,
    refresh_token: String,
    http: reqwest::Client,
    access: tokio::sync::Mutex<Option<(String, Instant)>>,
}

impl GDrive {
    pub fn new(
        client_id: String,
        client_secret: String,
        refresh_token: String,
    ) -> Result<Self, String> {
        Ok(Self {
            client_id,
            client_secret,
            refresh_token,
            http: http_client()?,
            access: tokio::sync::Mutex::new(None),
        })
    }

    async fn token(&self) -> Result<String, String> {
        let mut guard = self.access.lock().await;
        if let Some((tok, expires)) = guard.as_ref() {
            if Instant::now() < *expires {
                return Ok(tok.clone());
            }
        }
        let resp: TokenResponse = self
            .http
            .post(TOKEN_URL)
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("refresh_token", self.refresh_token.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|_| {
                "Google Drive prihlásenie vypršalo. Prihlás sa znova v Nastaveniach.".to_string()
            })?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let expires = Instant::now() + Duration::from_secs(resp.expires_in.saturating_sub(60));
        *guard = Some((resp.access_token.clone(), expires));
        Ok(resp.access_token)
    }

    async fn find_in_parent(
        &self,
        token: &str,
        parent: &str,
        name: &str,
        folders_only: bool,
    ) -> Result<Option<String>, String> {
        let escaped = name.replace('\\', "\\\\").replace('\'', "\\'");
        let mut q = format!("name = '{escaped}' and '{parent}' in parents and trashed = false");
        if folders_only {
            q.push_str(&format!(" and mimeType = '{FOLDER_MIME}'"));
        } else {
            q.push_str(&format!(" and mimeType != '{FOLDER_MIME}'"));
        }
        #[derive(Deserialize)]
        struct ListResp {
            #[serde(default)]
            files: Vec<FileMeta>,
        }
        #[derive(Deserialize)]
        struct FileMeta {
            id: String,
        }
        let resp = self
            .http
            .get(format!("{API}/files"))
            .bearer_auth(token)
            .query(&[
                ("q", q.as_str()),
                ("fields", "files(id)"),
                ("pageSize", "5"),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let resp: ListResp = check_api("Google Drive vyhľadanie zlyhalo", resp)
            .await?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        Ok(resp.files.into_iter().next().map(|f| f.id))
    }

    async fn ensure_dir_from(
        &self,
        token: &str,
        mut parent: String,
        segments: &[String],
    ) -> Result<String, String> {
        for seg in segments {
            parent = match self.find_in_parent(token, &parent, seg, true).await? {
                Some(id) => id,
                None => {
                    #[derive(Deserialize)]
                    struct Created {
                        id: String,
                    }
                    let resp = self
                        .http
                        .post(format!("{API}/files"))
                        .bearer_auth(token)
                        .json(&serde_json::json!({
                            "name": seg,
                            "mimeType": FOLDER_MIME,
                            "parents": [parent],
                        }))
                        .send()
                        .await
                        .map_err(|e| e.to_string())?;
                    let created: Created =
                        check_api("Google Drive vytvorenie priečinka zlyhalo", resp)
                            .await?
                            .json()
                            .await
                            .map_err(|e| e.to_string())?;
                    created.id
                }
            };
        }
        Ok(parent)
    }

    async fn ensure_dir(&self, token: &str, segments: &[String]) -> Result<String, String> {
        self.ensure_dir_from(token, "root".to_string(), segments)
            .await
    }

    async fn find_dir(&self, token: &str, segments: &[String]) -> Result<Option<String>, String> {
        let mut parent = "root".to_string();
        for segment in segments {
            let Some(id) = self.find_in_parent(token, &parent, segment, true).await? else {
                return Ok(None);
            };
            parent = id;
        }
        Ok(Some(parent))
    }

    async fn list_children(&self, token: &str, parent: &str) -> Result<Vec<DriveItem>, String> {
        let q = format!("'{parent}' in parents and trashed = false");
        let mut page_token: Option<String> = None;
        let mut out = Vec::new();
        loop {
            let mut request = self
                .http
                .get(format!("{API}/files"))
                .bearer_auth(token)
                .query(&[
                    ("q", q.as_str()),
                    (
                        "fields",
                        "nextPageToken,files(id,name,mimeType,createdTime,modifiedTime,size)",
                    ),
                    ("pageSize", "1000"),
                ]);
            if let Some(page) = page_token.as_deref() {
                request = request.query(&[("pageToken", page)]);
            }
            let response = request.send().await.map_err(|e| e.to_string())?;
            let page: DriveListResponse = check_api("Google Drive výpis zlyhal", response)
                .await?
                .json()
                .await
                .map_err(|e| e.to_string())?;
            out.extend(page.files);
            page_token = page.next_page_token;
            if page_token.is_none() {
                break;
            }
        }
        Ok(out)
    }

    async fn upload_in_parent(
        &self,
        token: &str,
        folder_id: &str,
        name: &str,
        local: &Path,
    ) -> Result<String, String> {
        let existing = self.find_in_parent(token, folder_id, name, false).await?;
        let init = match &existing {
            Some(id) => self
                .http
                .patch(format!("{UPLOAD_API}/files/{id}?uploadType=resumable"))
                .bearer_auth(token)
                .json(&serde_json::json!({})),
            None => self
                .http
                .post(format!("{UPLOAD_API}/files?uploadType=resumable"))
                .bearer_auth(token)
                .json(&serde_json::json!({ "name": name, "parents": [folder_id] })),
        };
        let response = init.send().await.map_err(|e| e.to_string())?;
        let response = check_api("Google Drive upload init zlyhal", response).await?;
        let session = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| "Google Drive nevrátil upload URL.".to_string())?
            .to_string();

        #[derive(Deserialize)]
        struct Uploaded {
            id: String,
        }
        let file = tokio::fs::File::open(local)
            .await
            .map_err(|e| format!("Čítanie súboru zlyhalo: {e}"))?;
        let size = file
            .metadata()
            .await
            .map_err(|e| format!("Veľkosť súboru sa nedá zistiť: {e}"))?
            .len();
        let response = self
            .http
            .put(session)
            .header(reqwest::header::CONTENT_LENGTH, size)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(reqwest::Body::wrap_stream(ReaderStream::new(file)))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let uploaded: Uploaded = check_api("Google Drive upload zlyhal", response)
            .await?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        Ok(uploaded.id)
    }
}

#[async_trait::async_trait]
impl StorageProvider for GDrive {
    async fn upload_file(
        &self,
        dir: &[String],
        name: &str,
        local: &Path,
    ) -> Result<String, String> {
        let token = self.token().await?;
        let folder_id = self.ensure_dir(&token, dir).await?;
        self.upload_in_parent(&token, &folder_id, name, local).await
    }

    async fn ensure_folder(&self, dir: &[String]) -> Result<RemoteFolder, String> {
        let token = self.token().await?;
        let id = self.ensure_dir(&token, dir).await?;
        Ok(RemoteFolder {
            id,
            name: dir.last().cloned().unwrap_or_default(),
        })
    }

    async fn list_folders(&self, dir: &[String]) -> Result<Vec<RemoteFolder>, String> {
        let token = self.token().await?;
        let Some(parent) = self.find_dir(&token, dir).await? else {
            return Ok(Vec::new());
        };
        let mut folders: Vec<RemoteFolder> = self
            .list_children(&token, &parent)
            .await?
            .into_iter()
            .filter(|item| item.mime_type == FOLDER_MIME)
            .map(|item| RemoteFolder {
                id: item.id,
                name: item.name,
            })
            .collect();
        folders.sort_by_key(|folder| folder.name.to_lowercase());
        Ok(folders)
    }

    async fn upload_file_to_folder(
        &self,
        folder_id: &str,
        relative_dir: &[String],
        name: &str,
        local: &Path,
    ) -> Result<String, String> {
        let token = self.token().await?;
        let parent = self
            .ensure_dir_from(&token, folder_id.to_string(), relative_dir)
            .await?;
        self.upload_in_parent(&token, &parent, name, local).await
    }

    async fn list_files_recursive(&self, folder_id: &str) -> Result<Vec<RemoteTreeFile>, String> {
        let token = self.token().await?;
        let mut pending = VecDeque::from([(folder_id.to_string(), String::new())]);
        let mut files = Vec::new();
        while let Some((parent, prefix)) = pending.pop_front() {
            for item in self.list_children(&token, &parent).await? {
                let relative_path = if prefix.is_empty() {
                    item.name.clone()
                } else {
                    format!("{prefix}/{}", item.name)
                };
                if item.mime_type == FOLDER_MIME {
                    pending.push_back((item.id, relative_path));
                } else {
                    files.push(RemoteTreeFile {
                        id: item.id,
                        relative_path,
                        modified_at: item.modified_time.or(item.created_time),
                        size_bytes: item.size.and_then(|s| s.parse().ok()).unwrap_or(0),
                    });
                }
            }
        }
        files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(files)
    }

    async fn list_files(&self, dir: &[String]) -> Result<Vec<RemoteFile>, String> {
        let token = self.token().await?;
        let Some(folder_id) = self.find_dir(&token, dir).await? else {
            return Ok(Vec::new());
        };
        let mut files: Vec<RemoteFile> = self
            .list_children(&token, &folder_id)
            .await?
            .into_iter()
            .filter(|item| item.mime_type != FOLDER_MIME)
            .map(|item| RemoteFile {
                id: item.id,
                name: item.name,
                created_at: item.created_time,
                size_bytes: item.size.and_then(|s| s.parse().ok()).unwrap_or(0),
            })
            .collect();
        files.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(files)
    }

    async fn download_file(&self, file_id: &str, dest: &Path) -> Result<(), String> {
        let token = self.token().await?;
        let resp = self
            .http
            .get(format!("{API}/files/{file_id}?alt=media"))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let mut resp = check_api("Google Drive stiahnutie zlyhalo", resp).await?;
        let mut file = tokio::fs::File::create(dest)
            .await
            .map_err(|e| e.to_string())?;
        while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())
    }

    async fn delete_file(&self, file_id: &str) -> Result<(), String> {
        let token = self.token().await?;
        let resp = self
            .http
            .delete(format!("{API}/files/{file_id}"))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        check_api("Google Drive mazanie zlyhalo", resp).await?;
        Ok(())
    }
}
