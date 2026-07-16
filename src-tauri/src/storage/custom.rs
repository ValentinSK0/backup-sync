use super::{http_client, RemoteFile, RemoteFolder, RemoteTreeFile, StorageProvider};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::net::IpAddr;
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudItem {
    path: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    created_at: Option<String>,
}

impl CustomCloud {
    pub fn new(endpoint: String, token: String) -> Result<Self, String> {
        Ok(Self {
            endpoint: normalize_endpoint(&endpoint)?,
            token,
            http: http_client()?,
        })
    }

    fn file_url(&self, segments: &[&str]) -> Result<url::Url, String> {
        let mut u = url::Url::parse(&format!("{}/api/files", self.endpoint))
            .map_err(|e| format!("Neplatná adresa cloudu: {e}"))?;
        u.path_segments_mut()
            .map_err(|_| "Neplatná adresa cloudu.".to_string())?
            .extend(segments);
        Ok(u)
    }

    async fn list_items(&self, prefix: &str) -> Result<Vec<CloudItem>, String> {
        self.http
            .get(format!("{}/api/files", self.endpoint))
            .query(&[("prefix", prefix)])
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Cloud nedostupný: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Výpis zlyhal: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Neplatná odpoveď cloudu: {e}"))
    }
}

pub fn normalize_endpoint(endpoint: &str) -> Result<String, String> {
    let mut url =
        url::Url::parse(endpoint.trim()).map_err(|e| format!("Neplatná adresa cloudu: {e}"))?;
    if url.host_str().is_none() {
        return Err("Endpoint musí obsahovať názov servera.".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Endpoint nesmie obsahovať používateľské meno ani heslo.".into());
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("Endpoint nesmie obsahovať query ani fragment.".into());
    }

    let secure = url.scheme() == "https";
    let loopback = match url.host_str() {
        Some("localhost") => true,
        Some(host) => host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback()),
        None => false,
    };
    if !(secure || url.scheme() == "http" && loopback) {
        return Err(
            "Vlastný cloud musí používať HTTPS. HTTP je povolené iba pre localhost.".into(),
        );
    }
    if url.path() == "/" {
        url.set_path("");
    }
    Ok(url.as_str().trim_end_matches('/').to_string())
}

pub async fn login(endpoint: &str, username: &str, password: &str) -> Result<String, String> {
    let endpoint = normalize_endpoint(endpoint)?;
    #[derive(Deserialize)]
    struct LoginResp {
        token: String,
    }
    let resp: LoginResp = http_client()?
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
    if resp.token.trim().is_empty() {
        return Err("Cloud vrátil prázdny prihlasovací token.".into());
    }
    Ok(resp.token)
}

#[async_trait::async_trait]
impl StorageProvider for CustomCloud {
    async fn upload_file(
        &self,
        dir: &[String],
        name: &str,
        local: &Path,
    ) -> Result<String, String> {
        let mut segments: Vec<&str> = dir.iter().map(|s| s.as_str()).collect();
        segments.push(name);
        let id = segments.join("/");
        let url = self.file_url(&segments)?;
        let file = tokio::fs::File::open(local)
            .await
            .map_err(|e| format!("Čítanie súboru zlyhalo: {e}"))?;
        let size = file
            .metadata()
            .await
            .map_err(|e| format!("Veľkosť súboru sa nedá zistiť: {e}"))?
            .len();
        self.http
            .put(url)
            .bearer_auth(&self.token)
            .header(reqwest::header::CONTENT_LENGTH, size)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(reqwest::Body::wrap_stream(ReaderStream::new(file)))
            .send()
            .await
            .map_err(|e| format!("Upload zlyhal: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Cloud odmietol upload: {e}"))?;
        Ok(id)
    }

    async fn ensure_folder(&self, dir: &[String]) -> Result<RemoteFolder, String> {
        let id = dir.join("/");
        let name = dir.last().cloned().unwrap_or_default();
        Ok(RemoteFolder { id, name })
    }

    async fn list_folders(&self, dir: &[String]) -> Result<Vec<RemoteFolder>, String> {
        let prefix = format!("{}/", dir.join("/"));
        let items = self.list_items(&prefix).await?;
        let mut names = BTreeSet::new();
        for item in items {
            if let Some(relative) = item.path.strip_prefix(&prefix) {
                if let Some((name, _)) = relative.split_once('/') {
                    if !name.is_empty() {
                        names.insert(name.to_string());
                    }
                }
            }
        }
        Ok(names
            .into_iter()
            .map(|name| RemoteFolder {
                id: format!("{}/{name}", prefix.trim_end_matches('/')),
                name,
            })
            .collect())
    }

    async fn upload_file_to_folder(
        &self,
        folder_id: &str,
        relative_dir: &[String],
        name: &str,
        local: &Path,
    ) -> Result<String, String> {
        let mut dir: Vec<String> = folder_id
            .split('/')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        dir.extend(relative_dir.iter().cloned());
        self.upload_file(&dir, name, local).await
    }

    async fn list_files_recursive(&self, folder_id: &str) -> Result<Vec<RemoteTreeFile>, String> {
        let prefix = format!("{}/", folder_id.trim_end_matches('/'));
        let items = self.list_items(&prefix).await?;
        Ok(items
            .into_iter()
            .filter_map(|item| {
                let relative_path = item.path.strip_prefix(&prefix)?.to_string();
                if relative_path.is_empty() {
                    return None;
                }
                Some(RemoteTreeFile {
                    id: item.path,
                    relative_path,
                    modified_at: item.created_at,
                    size_bytes: item.size,
                })
            })
            .collect())
    }

    async fn list_files(&self, dir: &[String]) -> Result<Vec<RemoteFile>, String> {
        let prefix = format!("{}/", dir.join("/"));
        let items = self.list_items(&prefix).await?;
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
        let mut response = self
            .http
            .get(url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Stiahnutie zlyhalo: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Cloud odmietol stiahnutie: {e}"))?;
        let mut file = tokio::fs::File::create(dest)
            .await
            .map_err(|e| e.to_string())?;
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())
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

#[cfg(test)]
mod tests {
    use super::normalize_endpoint;

    #[test]
    fn custom_cloud_requires_https_except_loopback() {
        assert_eq!(
            normalize_endpoint("https://cloud.example.com/").unwrap(),
            "https://cloud.example.com"
        );
        assert!(normalize_endpoint("http://cloud.example.com").is_err());
        assert!(normalize_endpoint("http://localhost:8080").is_ok());
        assert!(normalize_endpoint("http://127.0.0.1:8080").is_ok());
        assert!(normalize_endpoint("https://user:pass@example.com").is_err());
    }
}
