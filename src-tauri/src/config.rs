use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provider {
    #[serde(rename = "gdrive")]
    Gdrive,
    #[serde(rename = "customCloud")]
    CustomCloud,
    #[serde(rename = "ulozto")]
    Ulozto,
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
    #[serde(default)]
    pub cloud_id: Option<String>,
    #[serde(default)]
    pub cloud_name: String,
    #[serde(default)]
    pub auto_backup: Option<bool>,
    #[serde(default = "default_sync_state")]
    pub sync_state: SyncState,
    #[serde(default)]
    pub last_sync: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub last_error_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub name: String,
    pub save_path: String,
    /// Stabilný názov priečinka hry na cloude. Názov v aplikácii môže používateľ meniť.
    #[serde(default)]
    pub cloud_name: String,
    /// Miesto pre automatické lokálne ZIP zálohy a ochranné zálohy pred obnovou.
    #[serde(default)]
    pub backup_path: String,
    pub provider: Provider,
    #[serde(default)]
    pub auto_upload: Option<bool>,
    #[serde(default = "default_sync_state")]
    pub sync_state: SyncState,
    #[serde(default)]
    pub last_change: Option<String>,
    #[serde(default)]
    pub last_upload: Option<String>,
    /// ID cloudovej verzie, z ktorej lokálny save naposledy pochádza.
    #[serde(default)]
    pub last_cloud_version_id: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub local: GameLocalState,
    #[serde(default)]
    pub artwork: Option<GameArtwork>,
    #[serde(default)]
    pub artwork_checked_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameArtwork {
    #[serde(default = "legacy_artwork_source")]
    pub source: String,
    pub game_id: u64,
    pub game_name: String,
    pub image_id: u64,
    pub kind: String,
    pub url: String,
    pub author: String,
    #[serde(default)]
    pub selected_manually: bool,
}

fn legacy_artwork_source() -> String {
    "steamgriddb".into()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GameLocalState {
    pub observed_hash: Option<String>,
    pub backup_hash: Option<String>,
    pub uploaded_hash: Option<String>,
    pub last_backup: Option<String>,
    pub backup_file: Option<String>,
    pub last_check: Option<String>,
    pub watch_error: Option<String>,
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
    #[serde(default)]
    pub ulozto: ProviderStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_debounce")]
    pub debounce_seconds: u64,
    #[serde(default)]
    pub auto_backup_folders: bool,
    #[serde(default)]
    pub auto_upload_saves: bool,
    #[serde(default = "default_compression")]
    pub compression_level: CompressionLevel,
    #[serde(default = "default_gdrive_client_id")]
    pub gdrive_client_id: String,
    #[serde(default)]
    pub gdrive_client_secret_configured: bool,
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
fn default_gdrive_client_id() -> String {
    option_env!("BACKUP_SYNC_GOOGLE_CLIENT_ID")
        .unwrap_or_default()
        .trim()
        .to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            debounce_seconds: default_debounce(),
            auto_backup_folders: false,
            auto_upload_saves: false,
            compression_level: default_compression(),
            gdrive_client_id: default_gdrive_client_id(),
            gdrive_client_secret_configured: false,
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
        let backup = sidecar_path(path, ".bak");
        match read_config(path) {
            Ok(config) => config,
            Err(primary_error) => match read_config(&backup) {
                Ok(mut config) => {
                    let recovery = restore_primary_from_backup(path, &backup);
                    config.providers.gdrive.error = Some(match recovery {
                        Ok(()) => format!(
                            "Konfigurácia bola obnovená zo záložnej kópie ({primary_error})."
                        ),
                        Err(error) => format!(
                            "Konfigurácia sa načítala zo záložnej kópie, ale hlavný súbor sa nepodarilo opraviť: {error}"
                        ),
                    });
                    config
                }
                Err(_) if !path.exists() && !backup.exists() => Self::default(),
                Err(_) => {
                    let mut config = Self::default();
                    config.providers.gdrive.error = Some(
                        "Konfigurácia bola poškodená. Pôvodný súbor sa zachová ako config.json.bak."
                            .into(),
                    );
                    config
                }
            },
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        write_atomic(path, text.as_bytes())
    }
}

fn read_config(path: &Path) -> Result<AppConfig, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| "data".into());
    name.push(suffix);
    path.with_file_name(name)
}

pub fn contains_legacy_client_secret(path: &Path) -> bool {
    [path.to_path_buf(), sidecar_path(path, ".bak")]
        .iter()
        .any(|candidate| {
            fs::read_to_string(candidate).is_ok_and(|text| text.contains("\"gdriveClientSecret\""))
        })
}

pub fn legacy_client_secret(path: &Path) -> Option<String> {
    [path.to_path_buf(), sidecar_path(path, ".bak")]
        .iter()
        .find_map(|candidate| {
            let text = fs::read_to_string(candidate).ok()?;
            let value: serde_json::Value = serde_json::from_str(&text).ok()?;
            value
                .get("settings")?
                .get("gdriveClientSecret")?
                .as_str()
                .map(str::trim)
                .filter(|secret| !secret.is_empty())
                .map(str::to_string)
        })
}

fn restore_primary_from_backup(path: &Path, backup: &Path) -> Result<(), String> {
    let corrupt = sidecar_path(path, ".corrupt");
    if corrupt.exists() {
        fs::remove_file(&corrupt).map_err(|e| e.to_string())?;
    }
    if path.exists() {
        fs::rename(path, &corrupt).map_err(|e| e.to_string())?;
    }
    if let Err(error) = fs::copy(backup, path) {
        if !path.exists() && corrupt.exists() {
            let _ = fs::rename(&corrupt, path);
        }
        return Err(error.to_string());
    }
    fs::OpenOptions::new()
        .read(true)
        .open(path)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let temporary = sidecar_path(path, ".tmp");
    let backup = sidecar_path(path, ".bak");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);

    let had_original = path.exists();
    if had_original {
        if backup.exists() {
            fs::remove_file(&backup).map_err(|e| e.to_string())?;
        }
        fs::rename(path, &backup).map_err(|e| e.to_string())?;
    }

    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        if had_original && !path.exists() {
            let _ = fs::rename(&backup, path);
        }
        return Err(error.to_string());
    }
    Ok(())
}

pub fn config_file(config_dir: &Path) -> PathBuf {
    config_dir.join("config.json")
}

pub fn now_iso() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_folder_config_gets_cloud_defaults() {
        let config: AppConfig = serde_json::from_str(
            r#"{
                "watchedFolders": [{
                    "id": "folder-id",
                    "path": "C:/Saves",
                    "provider": "gdrive"
                }]
            }"#,
        )
        .unwrap();

        let folder = &config.watched_folders[0];
        assert_eq!(folder.cloud_id, None);
        assert_eq!(folder.cloud_name, "");
        assert_eq!(folder.auto_backup, None);
        assert_eq!(folder.last_error_action, None);
        assert!(!config.settings.auto_backup_folders);
    }

    #[test]
    fn config_save_is_recoverable_from_backup() {
        let root =
            std::env::temp_dir().join(format!("backup-sync-config-test-{}", uuid::Uuid::new_v4()));
        let path = root.join("config.json");
        let mut first = AppConfig::default();
        first.settings.debounce_seconds = 3;
        first.save(&path).unwrap();

        let mut second = first.clone();
        second.settings.debounce_seconds = 9;
        second.save(&path).unwrap();
        fs::write(&path, b"{broken").unwrap();

        let recovered = AppConfig::load(&path);
        assert_eq!(recovered.settings.debounce_seconds, 3);
        assert!(recovered.providers.gdrive.error.is_some());
        assert_eq!(
            AppConfig::load(&path).settings.debounce_seconds,
            recovered.settings.debounce_seconds
        );
        assert!(sidecar_path(&path, ".corrupt").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_client_secret_is_removed_from_primary_and_backup() {
        let root =
            std::env::temp_dir().join(format!("backup-sync-secret-test-{}", uuid::Uuid::new_v4()));
        let path = root.join("config.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            &path,
            r#"{"settings":{"gdriveClientSecret":"legacy-plaintext-secret"}}"#,
        )
        .unwrap();

        assert_eq!(
            legacy_client_secret(&path).as_deref(),
            Some("legacy-plaintext-secret")
        );
        let should_scrub_backup = contains_legacy_client_secret(&path);
        let config = AppConfig::load(&path);
        config.save(&path).unwrap();
        if should_scrub_backup {
            config.save(&path).unwrap();
        }

        assert!(!contains_legacy_client_secret(&path));
        let _ = fs::remove_dir_all(root);
    }
}
