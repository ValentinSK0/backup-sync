pub mod custom;
pub mod gdrive;

use crate::config::{AppConfig, Provider};
use serde::Serialize;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFile {
    pub id: String,
    pub name: String,
    pub created_at: Option<String>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct RemoteFolder {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct RemoteTreeFile {
    pub id: String,
    pub relative_path: String,
    pub modified_at: Option<String>,
    pub size_bytes: u64,
}

#[async_trait::async_trait]
pub trait StorageProvider: Send + Sync {
    /// Nahrá súbor do vzdialeného priečinka (segmenty cesty). Existujúci súbor s rovnakým menom prepíše.
    async fn upload_file(&self, dir: &[String], name: &str, local: &Path)
        -> Result<String, String>;
    /// Vytvorí alebo nájde priečinok podľa cesty a vráti jeho stabilné provider ID.
    async fn ensure_folder(&self, dir: &[String]) -> Result<RemoteFolder, String>;
    /// Vypíše priame podpriečinky danej cesty.
    async fn list_folders(&self, dir: &[String]) -> Result<Vec<RemoteFolder>, String>;
    /// Nahrá súbor relatívne k už známemu vzdialenému priečinku.
    async fn upload_file_to_folder(
        &self,
        folder_id: &str,
        relative_dir: &[String],
        name: &str,
        local: &Path,
    ) -> Result<String, String>;
    /// Rekurzívne vypíše všetky súbory pod vzdialeným priečinkom.
    async fn list_files_recursive(&self, folder_id: &str) -> Result<Vec<RemoteTreeFile>, String>;
    async fn list_files(&self, dir: &[String]) -> Result<Vec<RemoteFile>, String>;
    async fn download_file(&self, file_id: &str, dest: &Path) -> Result<(), String>;
    #[allow(dead_code)]
    async fn delete_file(&self, file_id: &str) -> Result<(), String>;
}

pub const KEYRING_SERVICE: &str = "backup-sync";

pub fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(30 * 60))
        .user_agent(concat!("BackupSync/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("HTTP klient sa nepodarilo vytvoriť: {e}"))
}

pub fn keyring_get(key: &str) -> Result<String, String> {
    keyring::Entry::new(KEYRING_SERVICE, key)
        .and_then(|e| e.get_password())
        .map_err(|e| e.to_string())
}

pub fn keyring_set(key: &str, value: &str) -> Result<(), String> {
    keyring::Entry::new(KEYRING_SERVICE, key)
        .and_then(|e| e.set_password(value))
        .map_err(|e| e.to_string())
}

pub fn keyring_delete(key: &str) {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, key) {
        let _ = entry.delete_credential();
    }
}

/// Vyrobí storage providera podľa konfigurácie. Zlyhá zrozumiteľnou chybou, keď nie je pripojený.
pub fn make_provider(
    provider: Provider,
    config: &AppConfig,
) -> Result<Box<dyn StorageProvider>, String> {
    match provider {
        Provider::Gdrive => {
            if !config.providers.gdrive.connected {
                return Err("Google Drive nie je pripojený. Pripoj ho v Nastaveniach.".into());
            }
            let client_id = config.settings.gdrive_client_id.clone();
            if client_id.trim().is_empty() {
                return Err("Chýba Google OAuth Client ID. Doplň ho v Nastaveniach.".into());
            }
            let client_secret = keyring_get(gdrive::CLIENT_SECRET_KEY).map_err(|_| {
                "Chýba Google OAuth Client Secret. Doplň ho v Nastaveniach.".to_string()
            })?;
            let refresh_token = keyring_get(gdrive::REFRESH_TOKEN_KEY).map_err(|_| {
                "Google Drive token sa nenašiel. Prihlás sa znova v Nastaveniach.".to_string()
            })?;
            Ok(Box::new(gdrive::GDrive::new(
                client_id,
                client_secret,
                refresh_token,
            )?))
        }
        Provider::CustomCloud => {
            let status = &config.providers.custom_cloud;
            if !status.connected {
                return Err("Vlastný cloud nie je pripojený. Prihlás sa v Nastaveniach.".into());
            }
            let endpoint = status
                .endpoint
                .clone()
                .ok_or_else(|| "Chýba adresa vlastného cloudu.".to_string())?;
            let token = keyring_get(custom::TOKEN_KEY)
                .map_err(|_| "Token vlastného cloudu sa nenašiel. Prihlás sa znova.".to_string())?;
            Ok(Box::new(custom::CustomCloud::new(endpoint, token)?))
        }
    }
}
