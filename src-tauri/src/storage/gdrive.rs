use super::{RemoteFile, StorageProvider};
use base64::Engine;
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub const REFRESH_TOKEN_KEY: &str = "gdrive_refresh_token";

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const API: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD_API: &str = "https://www.googleapis.com/upload/drive/v3";
const SCOPE: &str = "https://www.googleapis.com/auth/drive.file https://www.googleapis.com/auth/userinfo.email";

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

fn default_expiry() -> u64 {
    3600
}

/// Spustí OAuth 2.0 PKCE flow: otvorí prehliadač, počká na kód na localhost porte,
/// vymení ho za tokeny. Vráti (refresh_token, e-mail účtu).
pub async fn connect(
    app: &tauri::AppHandle,
    client_id: &str,
    client_secret: &str,
) -> Result<(String, String), String> {
    if client_id.trim().is_empty() {
        return Err("Najprv vlož Google OAuth Client ID v Nastaveniach.".into());
    }
    if client_secret.trim().is_empty() {
        return Err("Najprv vlož Google OAuth Client Secret v Nastaveniach (Google ho pri Desktop app klientoch vyžaduje).".into());
    }

    let mut verifier_bytes = [0u8; 48];
    rand::thread_rng().fill_bytes(&mut verifier_bytes);
    let verifier = b64url(&verifier_bytes);
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));

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
        ],
    )
    .map_err(|e| e.to_string())?;

    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_url(auth_url.as_str(), None::<&str>)
            .map_err(|e| format!("Nepodarilo sa otvoriť prehliadač: {e}"))?;
    }

    let code = tokio::time::timeout(Duration::from_secs(300), wait_for_code(listener))
        .await
        .map_err(|_| "Prihlásenie vypršalo (5 min). Skús znova.".to_string())??;

    let http = reqwest::Client::new();
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

    #[derive(Deserialize)]
    struct UserInfo {
        #[serde(default)]
        email: Option<String>,
    }
    let email = http
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(&token.access_token)
        .send()
        .await
        .ok()
        .and_then(|r| r.error_for_status().ok());
    let email = match email {
        Some(r) => r.json::<UserInfo>().await.ok().and_then(|u| u.email),
        None => None,
    }
    .unwrap_or_else(|| "Google účet".into());

    Ok((refresh_token, email))
}

async fn wait_for_code(listener: TcpListener) -> Result<String, String> {
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
        for (k, v) in parsed.query_pairs() {
            match k.as_ref() {
                "code" => code = Some(v.to_string()),
                "error" => err = Some(v.to_string()),
                _ => {}
            }
        }
        let body = if code.is_some() {
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
    pub fn new(client_id: String, client_secret: String, refresh_token: String) -> Self {
        Self {
            client_id,
            client_secret,
            refresh_token,
            http: reqwest::Client::new(),
            access: tokio::sync::Mutex::new(None),
        }
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
            .map_err(|_| "Google Drive prihlásenie vypršalo. Prihlás sa znova v Nastaveniach.".to_string())?
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
            q.push_str(" and mimeType = 'application/vnd.google-apps.folder'");
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
            .query(&[("q", q.as_str()), ("fields", "files(id)"), ("pageSize", "5")])
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

    async fn ensure_dir(&self, token: &str, segments: &[String]) -> Result<String, String> {
        let mut parent = "root".to_string();
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
                            "mimeType": "application/vnd.google-apps.folder",
                            "parents": [parent],
                        }))
                        .send()
                        .await
                        .map_err(|e| e.to_string())?;
                    let created: Created = check_api("Google Drive vytvorenie priečinka zlyhalo", resp)
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
}

#[async_trait::async_trait]
impl StorageProvider for GDrive {
    async fn upload_file(&self, dir: &[String], name: &str, local: &Path) -> Result<String, String> {
        let token = self.token().await?;
        let folder_id = self.ensure_dir(&token, dir).await?;
        let bytes = tokio::fs::read(local)
            .await
            .map_err(|e| format!("Čítanie súboru zlyhalo: {e}"))?;

        // Existujúci súbor prepíš (nová revízia), inak vytvor nový — resumable upload.
        let existing = self.find_in_parent(&token, &folder_id, name, false).await?;
        let init = match &existing {
            Some(id) => self
                .http
                .patch(format!("{UPLOAD_API}/files/{id}?uploadType=resumable"))
                .bearer_auth(&token)
                .json(&serde_json::json!({})),
            None => self
                .http
                .post(format!("{UPLOAD_API}/files?uploadType=resumable"))
                .bearer_auth(&token)
                .json(&serde_json::json!({ "name": name, "parents": [folder_id] })),
        };
        let resp = init.send().await.map_err(|e| e.to_string())?;
        let resp = check_api("Google Drive upload init zlyhal", resp).await?;
        let session = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| "Google Drive nevrátil upload URL.".to_string())?
            .to_string();

        #[derive(Deserialize)]
        struct Uploaded {
            id: String,
        }
        let resp = self
            .http
            .put(session)
            .header("Content-Length", bytes.len())
            .body(bytes)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let uploaded: Uploaded = check_api("Google Drive upload zlyhal", resp)
            .await?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        Ok(uploaded.id)
    }

    async fn list_files(&self, dir: &[String]) -> Result<Vec<RemoteFile>, String> {
        let token = self.token().await?;
        let folder_id = self.ensure_dir(&token, dir).await?;
        #[derive(Deserialize)]
        struct ListResp {
            #[serde(default)]
            files: Vec<FileMeta>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct FileMeta {
            id: String,
            name: String,
            #[serde(default)]
            created_time: Option<String>,
            #[serde(default)]
            size: Option<String>,
        }
        let q = format!("'{folder_id}' in parents and trashed = false and mimeType != 'application/vnd.google-apps.folder'");
        let resp = self
            .http
            .get(format!("{API}/files"))
            .bearer_auth(&token)
            .query(&[
                ("q", q.as_str()),
                ("fields", "files(id,name,createdTime,size)"),
                ("orderBy", "createdTime desc"),
                ("pageSize", "100"),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let resp: ListResp = check_api("Google Drive výpis zlyhal", resp)
            .await?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        Ok(resp
            .files
            .into_iter()
            .map(|f| RemoteFile {
                id: f.id,
                name: f.name,
                created_at: f.created_time,
                size_bytes: f.size.and_then(|s| s.parse().ok()).unwrap_or(0),
            })
            .collect())
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
        let bytes = check_api("Google Drive stiahnutie zlyhalo", resp)
            .await?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;
        tokio::fs::write(dest, &bytes)
            .await
            .map_err(|e| e.to_string())
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
