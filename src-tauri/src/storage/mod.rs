pub mod custom;
pub mod gdrive;

use crate::config::{AppConfig, Provider};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFile {
    pub id: String,
    pub name: String,
    pub created_at: Option<String>,
    pub size_bytes: u64,
}

#[async_trait::async_trait]
pub trait StorageProvider: Send + Sync {
    /// Nahrá súbor do vzdialeného priečinka (segmenty cesty). Existujúci súbor s rovnakým menom prepíše.
    async fn upload_file(&self, dir: &[String], name: &str, local: &Path) -> Result<String, String>;
    async fn list_files(&self, dir: &[String]) -> Result<Vec<RemoteFile>, String>;
    async fn download_file(&self, file_id: &str, dest: &Path) -> Result<(), String>;
    #[allow(dead_code)]
    async fn delete_file(&self, file_id: &str) -> Result<(), String>;
}

pub const KEYRING_SERVICE: &str = "backup-sync";

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
            let client_secret = config.settings.gdrive_client_secret.clone();
            let refresh_token = keyring_get(gdrive::REFRESH_TOKEN_KEY)
                .map_err(|_| "Google Drive token sa nenašiel. Prihlás sa znova v Nastaveniach.".to_string())?;
            Ok(Box::new(gdrive::GDrive::new(client_id, client_secret, refresh_token)))
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
            Ok(Box::new(custom::CustomCloud::new(endpoint, token)))
        }
    }
}
