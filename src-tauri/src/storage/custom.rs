use super::{http_client, RemoteFile, RemoteFolder, RemoteTreeFile, StorageProvider};
use serde::Deserialize;
use std::collections::BTreeSet;
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
    let loopback = match url.host() {
        Some(url::Host::Domain("localhost")) => true,
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        _ => false,
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
            .filter_map(|i| {
                let relative = i.path.strip_prefix(&prefix)?;
                if relative.is_empty() || relative.contains('/') {
                    return None;
                }
                Some(RemoteFile {
                    name: i.path.rsplit('/').next().unwrap_or(&i.path).to_string(),
                    id: i.path,
                    created_at: i.created_at,
                    size_bytes: i.size,
                })
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
    use super::*;

    #[test]
    fn custom_cloud_requires_https_except_loopback() {
        assert_eq!(
            normalize_endpoint("https://cloud.example.com/").unwrap(),
            "https://cloud.example.com"
        );
        assert!(normalize_endpoint("http://cloud.example.com").is_err());
        assert!(normalize_endpoint("http://localhost:8080").is_ok());
        assert!(normalize_endpoint("http://127.0.0.1:8080").is_ok());
        assert!(normalize_endpoint("http://[::1]:8080").is_ok());
        assert!(normalize_endpoint("http://[2001:db8::1]:8080").is_err());
        assert!(normalize_endpoint("https://user:pass@example.com").is_err());
    }

    #[tokio::test]
    async fn custom_cloud_round_trip_uses_encoded_paths_and_preserves_bytes() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let data: Vec<u8> = (0..100_000).map(|index| (index % 251) as u8).collect();
        let server_data = data.clone();
        let file_id = "BackupSync/games/Test Game/save ?.zip";
        let items = serde_json::json!([
            {"path":file_id,"size":data.len(),"createdAt":"2026-11-09T12:00:00Z"},
            {"path":"BackupSync/games/Test Game/nested/other.zip","size":1},
            {"path":"unrelated/ignored.zip","size":1}
        ])
        .to_string();
        let worker = std::thread::spawn(move || {
            for index in 0..7 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 8192];
                while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                    let count = socket.read(&mut buffer).unwrap();
                    assert_ne!(count, 0);
                    request.extend_from_slice(&buffer[..count]);
                }
                let header_end = request
                    .windows(4)
                    .position(|part| part == b"\r\n\r\n")
                    .unwrap()
                    + 4;
                let headers = String::from_utf8_lossy(&request[..header_end]).into_owned();
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                while request.len() < header_end + length {
                    let count = socket.read(&mut buffer).unwrap();
                    assert_ne!(count, 0);
                    request.extend_from_slice(&buffer[..count]);
                }
                let line = headers.lines().next().unwrap();
                let method = line.split_whitespace().next().unwrap();
                let url = url::Url::parse(&format!(
                    "http://localhost{}",
                    line.split_whitespace().nth(1).unwrap()
                ))
                .unwrap();
                let body = match index {
                    0 => {
                        assert_eq!(method, "POST");
                        assert_eq!(url.path(), "/api/login");
                        let login: serde_json::Value =
                            serde_json::from_slice(&request[header_end..]).unwrap();
                        assert_eq!(login["username"], "audit-user");
                        br#"{"token":"audit-token"}"#.to_vec()
                    }
                    1 => {
                        assert_eq!(method, "PUT");
                        assert!(url.path().contains("Test%20Game/save%20%3F.zip"));
                        assert!(url.query().is_none());
                        assert_eq!(&request[header_end..], server_data.as_slice());
                        Vec::new()
                    }
                    2..=4 => {
                        assert_eq!(method, "GET");
                        assert_eq!(url.path(), "/api/files");
                        assert!(url.query_pairs().any(|(key, value)| key == "prefix"
                            && value.starts_with("BackupSync/games/")));
                        items.as_bytes().to_vec()
                    }
                    5 => {
                        assert_eq!(method, "GET");
                        assert!(url.path().contains("Test%20Game/save%20%3F.zip"));
                        server_data.clone()
                    }
                    _ => {
                        assert_eq!(method, "DELETE");
                        Vec::new()
                    }
                };
                if index > 0 {
                    assert!(headers
                        .to_ascii_lowercase()
                        .contains("authorization: bearer audit-token"));
                }
                let response=format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",body.len());
                socket.write_all(response.as_bytes()).unwrap();
                socket.write_all(&body).unwrap();
            }
        });
        let token = login(&endpoint, "audit-user", "audit-password")
            .await
            .unwrap();
        let cloud = CustomCloud::new(endpoint, token).unwrap();
        let root =
            std::env::temp_dir().join(format!("backup-sync-custom-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source.bin");
        std::fs::write(&source, &data).unwrap();
        let directory = vec!["BackupSync".into(), "games".into(), "Test Game".into()];
        assert_eq!(
            cloud
                .upload_file(&directory, "save ?.zip", &source)
                .await
                .unwrap(),
            file_id
        );
        let files = cloud.list_files(&directory).await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].id, file_id);
        assert_eq!(
            cloud.list_folders(&directory[..2]).await.unwrap()[0].name,
            "Test Game"
        );
        assert_eq!(
            cloud
                .list_files_recursive(&directory.join("/"))
                .await
                .unwrap()
                .len(),
            2
        );
        let destination = root.join("download.bin");
        cloud.download_file(file_id, &destination).await.unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), data);
        cloud.delete_file(file_id).await.unwrap();
        worker.join().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
