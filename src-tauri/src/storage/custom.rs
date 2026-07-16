use super::{RemoteFile, StorageProvider};
use serde::Deserialize;
use std::path::Path;

pub const TOKEN_KEY: &str = "custom_cloud_token";

/// Jednoduchý REST provider pre vlastný cloud. Očakávané API (viď docs/custom-cloud-api.md):
///   POST   {endpoint}/api/login                 {username, password} -> {token}
///   PUT    {endpoint}/api/files/{cesta}         telo = bytes (Bearer token)
///   GET    {endpoint}/api/files?prefix={cesta}/ -> [{path, size, createdAt}]
///   GET    {endpoint}/api/files/{cesta}         -> bytes
///   DELETE {endpoint}/api/files/{cesta}
pub struct CustomCloud {
    endpoint: String,
    token: String,
    http: reqwest::Client,
}

impl CustomCloud {
    pub fn new(endpoint: String, token: String) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            token,
            http: reqwest::Client::new(),
        }
    }

    fn file_url(&self, segments: &[&str]) -> Result<url::Url, String> {
        let mut u = url::Url::parse(&format!("{}/api/files", self.endpoint))
            .map_err(|e| format!("Neplatná adresa cloudu: {e}"))?;
        u.path_segments_mut()
            .map_err(|_| "Neplatná adresa cloudu.".to_string())?
            .extend(segments);
        Ok(u)
    }
}

pub async fn login(endpoint: &str, username: &str, password: &str) -> Result<String, String> {
    let endpoint = endpoint.trim_end_matches('/');
    #[derive(Deserialize)]
    struct LoginResp {
        token: String,
    }
    let resp: LoginResp = reqwest::Client::new()
        .post(format!("{endpoint}/api/login"))
        .json(&serde_json::json!({ "username": username, "password": password }))
        .send()
        .await
        .map_err(|e| format!("Cloud nedostupný: {e}"))?
        .error_for_status()
        .map_err(|_| "Nesprávne meno alebo heslo.".to_string())?
        .json()
        .await
        .map_err(|e| format!("Neplatná odpoveď cloudu: {e}"))?;
    Ok(resp.token)
}

#[async_trait::async_trait]
impl StorageProvider for CustomCloud {
    async fn upload_file(&self, dir: &[String], name: &str, local: &Path) -> Result<String, String> {
        let mut segments: Vec<&str> = dir.iter().map(|s| s.as_str()).collect();
        segments.push(name);
        let id = segments.join("/");
        let url = self.file_url(&segments)?;
        let bytes = tokio::fs::read(local)
            .await
            .map_err(|e| format!("Čítanie súboru zlyhalo: {e}"))?;
        self.http
            .put(url)
            .bearer_auth(&self.token)
            .body(bytes)
            .send()
            .await
            .map_err(|e| format!("Upload zlyhal: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Cloud odmietol upload: {e}"))?;
        Ok(id)
    }

    async fn list_files(&self, dir: &[String]) -> Result<Vec<RemoteFile>, String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Item {
            path: String,
            #[serde(default)]
            size: u64,
            #[serde(default)]
            created_at: Option<String>,
        }
        let prefix = format!("{}/", dir.join("/"));
        let items: Vec<Item> = self
            .http
            .get(format!("{}/api/files", self.endpoint))
            .query(&[("prefix", prefix.as_str())])
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Cloud nedostupný: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Výpis zlyhal: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Neplatná odpoveď cloudu: {e}"))?;
        Ok(items
            .into_iter()
            .map(|i| RemoteFile {
                name: i.path.rsplit('/').next().unwrap_or(&i.path).to_string(),
                id: i.path,
                created_at: i.created_at,
                size_bytes: i.size,
            })
            .collect())
    }

    async fn download_file(&self, file_id: &str, dest: &Path) -> Result<(), String> {
        let segments: Vec<&str> = file_id.split('/').collect();
        let url = self.file_url(&segments)?;
        let bytes = self
            .http
            .get(url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Stiahnutie zlyhalo: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Cloud odmietol stiahnutie: {e}"))?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;
        tokio::fs::write(dest, &bytes)
            .await
            .map_err(|e| e.to_string())
    }

    async fn delete_file(&self, file_id: &str) -> Result<(), String> {
        let segments: Vec<&str> = file_id.split('/').collect();
        let url = self.file_url(&segments)?;
        self.http
            .delete(url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Mazanie zlyhalo: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Cloud odmietol mazanie: {e}"))?;
        Ok(())
    }
}
