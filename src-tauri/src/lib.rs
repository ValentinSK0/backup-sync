mod archive;
mod commands;
mod config;
mod storage;
mod sync;
mod watcher;

use config::{AppConfig, SyncState};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Instant;
use tauri::Manager;

pub struct AppState {
    pub config: tokio::sync::Mutex<AppConfig>,
    pub config_dir: PathBuf,
    /// cesty, ktorých zmeny watcher dočasne ignoruje (počas obnovy zálohy)
    pub suppress: Arc<StdMutex<HashMap<PathBuf, Instant>>>,
    pub active_syncs: Arc<StdMutex<HashSet<String>>>,
    pub pending_changes: Arc<StdMutex<HashSet<String>>>,
    pub watcher_stop: StdMutex<Option<Arc<AtomicBool>>>,
}

impl AppState {
    pub fn config_path(&self) -> PathBuf {
        config::config_file(&self.config_dir)
    }
    pub fn state_dir(&self) -> PathBuf {
        self.config_dir.join("state")
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let config_path = config::config_file(&config_dir);
            let scrub_legacy_secret = config::contains_legacy_client_secret(&config_path);
            let legacy_client_secret = config::legacy_client_secret(&config_path);
            let mut cfg = AppConfig::load(&config_path);
            if storage::keyring_get(storage::gdrive::CLIENT_SECRET_KEY).is_err() {
                if let Some(secret) = legacy_client_secret {
                    if let Err(error) =
                        storage::keyring_set(storage::gdrive::CLIENT_SECRET_KEY, &secret)
                    {
                        cfg.providers.gdrive.error = Some(format!(
                            "Client Secret sa nepodarilo presunúť do Windows Credential Manager: {error}"
                        ));
                    }
                }
            }
            cfg.settings.gdrive_client_secret_configured =
                storage::keyring_get(storage::gdrive::CLIENT_SECRET_KEY)
                    .is_ok_and(|secret| !secret.trim().is_empty());
            // prerušené uploady z minulého behu → označ ako zmenené
            for f in cfg.watched_folders.iter_mut() {
                if f.cloud_name.is_empty() {
                    f.cloud_name = sync::folder_cloud_name(&f.path, &f.id);
                }
                if f.sync_state == SyncState::Syncing {
                    f.sync_state = SyncState::Changed;
                }
            }
            for g in cfg.games.iter_mut() {
                if g.sync_state == SyncState::Syncing {
                    g.sync_state = SyncState::Changed;
                }
            }
            cfg.save(&config_path).map_err(std::io::Error::other)?;
            if scrub_legacy_secret {
                // Prvý save odstráni pole z hlavného configu, druhý aj zo záložnej kópie.
                cfg.save(&config_path).map_err(std::io::Error::other)?;
            }

            let state = AppState {
                config: tokio::sync::Mutex::new(cfg.clone()),
                config_dir,
                suppress: Arc::new(StdMutex::new(HashMap::new())),
                active_syncs: Arc::new(StdMutex::new(HashSet::new())),
                pending_changes: Arc::new(StdMutex::new(HashSet::new())),
                watcher_stop: StdMutex::new(None),
            };
            app.manage(state);
            watcher::rebuild(app.handle(), &cfg);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::update_settings,
            commands::save_gdrive_credentials,
            commands::add_watched_folder,
            commands::remove_watched_folder,
            commands::set_folder_auto_backup,
            commands::add_game,
            commands::remove_game,
            commands::set_game_auto_upload,
            commands::backup_folder_now,
            commands::list_folder_backups,
            commands::connect_folder_backup,
            commands::restore_folder_from_cloud,
            commands::upload_game_save,
            commands::list_game_versions,
            commands::restore_game_version,
            commands::list_game_backups,
            commands::connect_game_backup,
            commands::open_local_folder,
            commands::gdrive_connect,
            commands::gdrive_disconnect,
            commands::custom_cloud_login,
            commands::custom_cloud_logout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
