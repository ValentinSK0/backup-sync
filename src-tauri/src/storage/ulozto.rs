use super::{http_client, RemoteFile, RemoteFolder, RemoteTreeFile, StorageProvider};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const AUTH_TOKEN_KEY: &str = "ulozto_auth_token";
pub const APP_TOKEN_KEY: &str = "ulozto_app_token";
pub const SESSION_KEY: &str = "ulozto_session_token";

const API: &str = "https://apis.uloz.to";
/// Verejný testovací X-Auth-Token z dokumentácie; použije sa, kým si používateľ
/// nevyžiada od podpory Ulož.to vlastný aplikačný API kľúč.
pub const TEST_APP_TOKEN: &str = ";HG%7jW6@6/8vx\">R;f(";
const CHUNK_SIZE: u64 = 4 * 1024 * 1024; // 4 MB, povolená hodnota resumable API

fn device_id(login: &str) -> String {
    format!("backup-sync-{login}")
}

#[derive(Deserialize)]
struct AuthResponse {
    token_id: String,
}

/// Overí prihlásenie (login + aplikačný login token) a vráti session token_id.
pub async fn login(login: &str, auth_token: &str, app_token: &str) -> Result<String, String> {
    let http = http_client()?;
    let resp = http
        .post(format!("{API}/v6/auth/token"))
        .header("X-Auth-Token", app_token)
        .json(&json!({ "login": login, "token": auth_token, "device_id": device_id(login) }))
        .send()
        .await
        .map_err(|e| format!("Ulož.to nedostupné: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        // 150001 = dočasná ochrana proti príliš častým prihláseniam
        if body.contains("150001") || body.contains("Try again later") {
            return Err(
                "Ulož.to dočasne blokuje prihlásenie (príliš veľa pokusov za sebou). \
                Počkaj pár minút a skús znova — session sa potom uloží a už sa to nebude opakovať."
                    .into(),
            );
        }
        let brief: String = body.chars().take(220).collect();
        return Err(format!("Ulož.to zamietlo prihlásenie ({status}): {brief}"));
    }
    let auth: AuthResponse = resp
        .json()
        .await
        .map_err(|e| format!("Neplatná odpoveď Ulož.to: {e}"))?;
    Ok(auth.token_id)
}

pub struct Ulozto {
    login: String,
    auth_token: String,
    app_token: String,
    http: reqwest::Client,
    session: tokio::sync::Mutex<Option<String>>,
    root_folder: tokio::sync::Mutex<Option<String>>,
}

impl Ulozto {
    pub fn new(login: String, auth_token: String, app_token: String) -> Result<Self, String> {
        Ok(Self {
            login,
            auth_token,
            app_token,
            http: http_client()?,
            session: tokio::sync::Mutex::new(None),
            root_folder: tokio::sync::Mutex::new(None),
        })
    }

    async fn token(&self) -> Result<String, String> {
        let mut guard = self.session.lock().await;
        if let Some(token) = guard.as_ref() {
            return Ok(token.clone());
        }
        // najprv session uložená z minula (šetrí prihlasovacie volania, ktoré Ulož.to limituje)
        if let Ok(saved) = super::keyring_get(SESSION_KEY) {
            if !saved.trim().is_empty() {
                *guard = Some(saved.clone());
                return Ok(saved);
            }
        }
        let token = login(&self.login, &self.auth_token, &self.app_token).await?;
        let _ = super::keyring_set(SESSION_KEY, &token);
        *guard = Some(token.clone());
        Ok(token)
    }

    async fn api_request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<reqwest::Response, String> {
        // jeden pokus + re-login pri vypršanej session
        for attempt in 0..2 {
            let token = self.token().await?;
            let mut req = self
                .http
                .request(method.clone(), format!("{API}{path}"))
                .header("X-Auth-Token", &self.app_token)
                .header("X-User-Token", token);
            if let Some(b) = &body {
                req = req.json(b);
            }
            let resp = req
                .send()
                .await
                .map_err(|e| format!("Ulož.to nedostupné: {e}"))?;
            if resp.status() == reqwest::StatusCode::UNAUTHORIZED && attempt == 0 {
                // session vypršala → zahoď ju a prihlás sa nanovo
                *self.session.lock().await = None;
                super::keyring_delete(SESSION_KEY);
                continue;
            }
            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                let brief: String = text.chars().take(300).collect();
                return Err(format!(
                    "Ulož.to {method} {path} zlyhalo ({status}): {brief}"
                ));
            }
            return Ok(resp);
        }
        unreachable!()
    }

    async fn root_folder_slug(&self) -> Result<String, String> {
        let mut guard = self.root_folder.lock().await;
        if let Some(slug) = guard.as_ref() {
            return Ok(slug.clone());
        }
        #[derive(Deserialize)]
        struct Me {
            user: MeUser,
        }
        #[derive(Deserialize)]
        struct MeUser {
            root_folder_slug: String,
        }
        let me: Me = self
            .api_request(reqwest::Method::GET, "/v6/me", None)
            .await?
            .json()
            .await
            .map_err(|e| format!("Neplatná odpoveď Ulož.to: {e}"))?;
        *guard = Some(me.user.root_folder_slug.clone());
        Ok(me.user.root_folder_slug)
    }

    async fn subfolders(&self, folder_slug: &str) -> Result<Vec<RemoteFolder>, String> {
        #[derive(Deserialize)]
        struct FolderList {
            #[serde(default)]
            subfolders: Vec<Sub>,
        }
        #[derive(Deserialize)]
        struct Sub {
            name: String,
            slug: String,
        }
        let path = format!("/v9/user/{}/folder/{}/folder-list", self.login, folder_slug);
        let list: FolderList = self
            .api_request(reqwest::Method::GET, &path, None)
            .await?
            .json()
            .await
            .map_err(|e| format!("Neplatná odpoveď Ulož.to: {e}"))?;
        Ok(list
            .subfolders
            .into_iter()
            .map(|s| RemoteFolder {
                id: s.slug,
                name: s.name,
            })
            .collect())
    }

    async fn create_folder(&self, parent_slug: &str, name: &str) -> Result<RemoteFolder, String> {
        #[derive(Deserialize)]
        struct Created {
            slug: String,
            name: String,
        }
        let created: Created = self
            .api_request(
                reqwest::Method::POST,
                &format!("/v6/user/{}/folder", self.login),
                Some(json!({ "name": name, "parent_folder_slug": parent_slug })),
            )
            .await?
            .json()
            .await
            .map_err(|e| format!("Neplatná odpoveď Ulož.to: {e}"))?;
        Ok(RemoteFolder {
            id: created.slug,
            name: created.name,
        })
    }

    /// Nájde alebo vytvorí zanorený priečinok podľa segmentov cesty od rootu.
    async fn ensure_dir_from(
        &self,
        start_slug: &str,
        segments: &[String],
    ) -> Result<RemoteFolder, String> {
        let mut current = RemoteFolder {
            id: start_slug.to_string(),
            name: String::new(),
        };
        for segment in segments {
            let existing = self
                .subfolders(&current.id)
                .await?
                .into_iter()
                .find(|f| f.name == *segment);
            current = match existing {
                Some(folder) => folder,
                None => self.create_folder(&current.id, segment).await?,
            };
        }
        Ok(current)
    }

    async fn files_in(&self, folder_slug: &str) -> Result<Vec<RemoteFile>, String> {
        #[derive(Deserialize)]
        struct FileList {
            #[serde(default)]
            items: Vec<Item>,
        }
        #[derive(Deserialize)]
        struct Item {
            slug: String,
            name: String,
            #[serde(default)]
            filesize: u64,
            #[serde(default)]
            created: Option<String>,
        }
        let path = format!("/v8/user/{}/folder/{}/file-list", self.login, folder_slug);
        let list: FileList = self
            .api_request(reqwest::Method::GET, &path, None)
            .await?
            .json()
            .await
            .map_err(|e| format!("Neplatná odpoveď Ulož.to: {e}"))?;
        Ok(list
            .items
            .into_iter()
            .map(|i| RemoteFile {
                id: i.slug,
                name: i.name,
                created_at: i.created,
                size_bytes: i.filesize,
            })
            .collect())
    }

    async fn find_dir_from(
        &self,
        start_slug: &str,
        segments: &[String],
    ) -> Result<Option<RemoteFolder>, String> {
        let mut current = RemoteFolder {
            id: start_slug.to_string(),
            name: String::new(),
        };
        for segment in segments {
            let Some(folder) = self
                .subfolders(&current.id)
                .await?
                .into_iter()
                .find(|folder| folder.name == *segment)
            else {
                return Ok(None);
            };
            current = folder;
        }
        Ok(Some(current))
    }

    /// Nahrá jeden súbor cez resumable upload API a vráti slug súboru.
    async fn upload_to_folder_slug(
        &self,
        folder_slug: &str,
        name: &str,
        local: &Path,
    ) -> Result<String, String> {
        #[derive(Deserialize)]
        struct UploadLink {
            upload_resumable_base_url: String,
            upload_signature: String,
            private_slug: String,
        }
        let link: UploadLink = self
            .api_request(
                reqwest::Method::POST,
                "/v6/upload/link",
                Some(json!({ "user_login": self.login })),
            )
            .await?
            .json()
            .await
            .map_err(|e| format!("Neplatná odpoveď Ulož.to: {e}"))?;
        let base = link.upload_resumable_base_url.trim_end_matches('/');

        let metadata = tokio::fs::metadata(local)
            .await
            .map_err(|e| format!("Čítanie súboru zlyhalo: {e}"))?;
        let filesize = metadata.len();

        // registrácia súboru (batch s jediným súborom, id = 1)
        let register = self
            .http
            .post(format!("{base}/v1/chunked-file-upload"))
            .json(&json!({
                "batch_file_id": 1,
                "upload_signature": link.upload_signature,
                "name": name,
                "filesize": filesize,
                "chunksize": CHUNK_SIZE,
            }))
            .send()
            .await
            .map_err(|e| format!("Ulož.to upload server nedostupný: {e}"))?;
        if !register.status().is_success() {
            let status = register.status();
            let text = register.text().await.unwrap_or_default();
            let brief: String = text.chars().take(300).collect();
            return Err(format!("Registrácia uploadu zlyhala ({status}): {brief}"));
        }

        // bloky po CHUNK_SIZE
        let mut file = tokio::fs::File::open(local)
            .await
            .map_err(|e| format!("Čítanie súboru zlyhalo: {e}"))?;
        let mut chunk_id: u64 = 1;
        let mut buffer = vec![0u8; CHUNK_SIZE as usize];
        loop {
            let mut filled = 0usize;
            while filled < buffer.len() {
                let read = file
                    .read(&mut buffer[filled..])
                    .await
                    .map_err(|e| format!("Čítanie súboru zlyhalo: {e}"))?;
                if read == 0 {
                    break;
                }
                filled += read;
            }
            if filled == 0 {
                break;
            }
            let chunk = buffer[..filled].to_vec();
            let mut last_error = String::new();
            let mut uploaded = false;
            for _ in 0..3 {
                let resp = self
                    .http
                    .post(format!(
                        "{base}/v1/chunked-file-upload/{}/1/{chunk_id}",
                        link.private_slug
                    ))
                    .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                    .body(chunk.clone())
                    .timeout(Duration::from_secs(600))
                    .send()
                    .await;
                match resp {
                    Ok(r) if r.status().is_success() => {
                        uploaded = true;
                        break;
                    }
                    Ok(r) => last_error = format!("HTTP {}", r.status()),
                    Err(e) => last_error = e.to_string(),
                }
            }
            if !uploaded {
                return Err(format!(
                    "Blok {chunk_id} sa nepodarilo nahrať: {last_error}"
                ));
            }
            if filled < buffer.len() {
                break;
            }
            chunk_id += 1;
        }

        // čakanie na zloženie súboru na serveri
        #[derive(Deserialize)]
        struct Status {
            status: String,
            #[serde(default)]
            irpc: Option<Irpc>,
        }
        #[derive(Deserialize)]
        struct Irpc {
            slug: String,
        }
        let mut file_slug = None;
        for _ in 0..150 {
            let status: Status = self
                .http
                .get(format!(
                    "{base}/v1/chunked-file-upload/{}/1/status",
                    link.private_slug
                ))
                .send()
                .await
                .map_err(|e| format!("Ulož.to upload server nedostupný: {e}"))?
                .error_for_status()
                .map_err(|e| format!("Kontrola stavu uploadu zlyhala: {e}"))?
                .json()
                .await
                .map_err(|e| format!("Neplatná odpoveď Ulož.to: {e}"))?;
            match status.status.as_str() {
                "finished" => {
                    file_slug = status.irpc.map(|i| i.slug);
                    break;
                }
                "pending" | "processing" => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                other => return Err(format!("Upload skončil so stavom: {other}")),
            }
        }
        let file_slug =
            file_slug.ok_or_else(|| "Server súbor nezložil včas. Skús znova.".to_string())?;

        // nastavenie cieľového priečinka + commit dávky
        self.api_request(
            reqwest::Method::PATCH,
            "/v8/file-list/private",
            Some(json!({
                "slugs": [file_slug],
                "upload_tokens": { file_slug.clone(): format!("{}:1", link.private_slug) },
                "folder_slug": folder_slug,
            })),
        )
        .await?;
        self.api_request(
            reqwest::Method::PATCH,
            &format!("/v8/upload-batch/private/{}", link.private_slug),
            Some(json!({ "status": "confirmed", "owner_login": self.login })),
        )
        .await?;
        Ok(file_slug)
    }
}

#[async_trait::async_trait]
impl StorageProvider for Ulozto {
    async fn upload_file(
        &self,
        dir: &[String],
        name: &str,
        local: &Path,
    ) -> Result<String, String> {
        let root = self.root_folder_slug().await?;
        let folder = self.ensure_dir_from(&root, dir).await?;
        self.upload_to_folder_slug(&folder.id, name, local).await
    }

    async fn ensure_folder(&self, dir: &[String]) -> Result<RemoteFolder, String> {
        let root = self.root_folder_slug().await?;
        self.ensure_dir_from(&root, dir).await
    }

    async fn list_folders(&self, dir: &[String]) -> Result<Vec<RemoteFolder>, String> {
        let root = self.root_folder_slug().await?;
        let Some(folder) = self.find_dir_from(&root, dir).await? else {
            return Ok(Vec::new());
        };
        self.subfolders(&folder.id).await
    }

    async fn upload_file_to_folder(
        &self,
        folder_id: &str,
        relative_dir: &[String],
        name: &str,
        local: &Path,
    ) -> Result<String, String> {
        let folder = self.ensure_dir_from(folder_id, relative_dir).await?;
        self.upload_to_folder_slug(&folder.id, name, local).await
    }

    async fn list_files_recursive(&self, folder_id: &str) -> Result<Vec<RemoteTreeFile>, String> {
        // BFS cez podpriečinky, relatívne cesty od zadaného priečinka
        let mut out = Vec::new();
        let mut queue = vec![(folder_id.to_string(), String::new())];
        while let Some((slug, prefix)) = queue.pop() {
            for file in self.files_in(&slug).await? {
                let relative_path = if prefix.is_empty() {
                    file.name.clone()
                } else {
                    format!("{prefix}/{}", file.name)
                };
                out.push(RemoteTreeFile {
                    id: file.id,
                    relative_path,
                    modified_at: file.created_at,
                    size_bytes: file.size_bytes,
                });
            }
            for sub in self.subfolders(&slug).await? {
                let next_prefix = if prefix.is_empty() {
                    sub.name.clone()
                } else {
                    format!("{prefix}/{}", sub.name)
                };
                queue.push((sub.id, next_prefix));
            }
        }
        Ok(out)
    }

    async fn list_files(&self, dir: &[String]) -> Result<Vec<RemoteFile>, String> {
        let root = self.root_folder_slug().await?;
        let Some(folder) = self.find_dir_from(&root, dir).await? else {
            return Ok(Vec::new());
        };
        self.files_in(&folder.id).await
    }

    async fn download_file(&self, file_id: &str, dest: &Path) -> Result<(), String> {
        #[derive(Deserialize)]
        struct DownloadInfo {
            link: String,
        }
        // Účty nemôžu použiť každý typ odkazu pre každý súbor → skúšaj postupne.
        let body = json!({
            "file_slug": file_id,
            "user_login": self.login,
            "device_id": device_id(&self.login),
        });
        let mut link = None;
        let mut last_error = String::new();
        for kind in ["fast", "vipdata", "free"] {
            let result = self
                .api_request(
                    reqwest::Method::POST,
                    &format!("/v5/file/download-link/{kind}"),
                    Some(body.clone()),
                )
                .await;
            match result {
                Ok(resp) => match resp.json::<DownloadInfo>().await {
                    Ok(info) => {
                        link = Some(info.link);
                        break;
                    }
                    Err(e) => last_error = format!("Neplatná odpoveď Ulož.to: {e}"),
                },
                Err(e) => last_error = e,
            }
        }
        let link = link.ok_or_else(|| {
            format!("Ulož.to nevydalo odkaz na stiahnutie (skúšané fast/vipdata/free). Posledná chyba: {last_error}")
        })?;

        let mut resp = self
            .http
            .get(&link)
            .timeout(Duration::from_secs(3600))
            .send()
            .await
            .map_err(|e| format!("Stiahnutie zlyhalo: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Ulož.to odmietlo stiahnutie: {e}"))?;
        let mut file = tokio::fs::File::create(dest)
            .await
            .map_err(|e| e.to_string())?;
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| format!("Stiahnutie zlyhalo: {e}"))?
        {
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())
    }

    async fn delete_file(&self, file_id: &str) -> Result<(), String> {
        self.api_request(
            reqwest::Method::DELETE,
            &format!("/v6/file/{file_id}/private"),
            None,
        )
        .await?;
        Ok(())
    }
}
