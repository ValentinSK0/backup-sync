use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provider {
    #[serde(rename = "gdrive")]
    Gdrive,
    #[serde(rename = "customCloud")]
    CustomCloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncState {
    Idle,
    Changed,
    Syncing,
    Synced,
    Error,
}

fn default_sync_state() -> SyncState {
    SyncState::Idle
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionLevel {
    Fast,
    Best,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchedFolder {
    pub id: String,
    pub path: String,
    pub provider: Provider,
    #[serde(default = "default_sync_state")]
    pub sync_state: SyncState,
    #[serde(default)]
    pub last_sync: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub name: String,
    pub save_path: String,
    pub provider: Provider,
    #[serde(default)]
    pub auto_upload: Option<bool>,
    #[serde(default = "default_sync_state")]
    pub sync_state: SyncState,
    #[serde(default)]
    pub last_change: Option<String>,
    #[serde(default)]
    pub last_upload: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub account: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Providers {
    #[serde(default)]
    pub gdrive: ProviderStatus,
    #[serde(default)]
    pub custom_cloud: ProviderStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_debounce")]
    pub debounce_seconds: u64,
    #[serde(default)]
    pub auto_upload_saves: bool,
    #[serde(default = "default_compression")]
    pub compression_level: CompressionLevel,
    #[serde(default)]
    pub gdrive_client_id: String,
    #[serde(default)]
    pub gdrive_client_secret: String,
}

fn default_theme() -> String {
    "dark".into()
}
fn default_debounce() -> u64 {
    2
}
fn default_compression() -> CompressionLevel {
    CompressionLevel::Fast
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            debounce_seconds: default_debounce(),
            auto_upload_saves: false,
            compression_level: default_compression(),
            gdrive_client_id: String::new(),
            gdrive_client_secret: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub watched_folders: Vec<WatchedFolder>,
    #[serde(default)]
    pub games: Vec<Game>,
    #[serde(default)]
    pub providers: Providers,
    #[serde(default)]
    pub settings: Settings,
}

impl AppConfig {
    pub fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, text).map_err(|e| e.to_string())
    }
}

pub fn config_file(config_dir: &Path) -> PathBuf {
    config_dir.join("config.json")
}

pub fn now_iso() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
