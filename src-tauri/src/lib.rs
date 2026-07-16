mod archive;
mod commands;
mod config;
mod storage;
mod sync;
mod watcher;

use config::{AppConfig, SyncState};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Instant;
use tauri::Manager;

pub struct AppState {
    pub config: tokio::sync::Mutex<AppConfig>,
    pub config_dir: PathBuf,
    /// cesty, ktorých zmeny watcher dočasne ignoruje (počas obnovy zálohy)
    pub suppress: Arc<StdMutex<HashMap<String, Instant>>>,
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
            let mut cfg = AppConfig::load(&config::config_file(&config_dir));
            // prerušené uploady z minulého behu → označ ako zmenené
            for f in cfg.watched_folders.iter_mut() {
                if f.sync_state == SyncState::Syncing {
                    f.sync_state = SyncState::Changed;
                }
            }
            for g in cfg.games.iter_mut() {
                if g.sync_state == SyncState::Syncing {
                    g.sync_state = SyncState::Changed;
                }
            }

            let state = AppState {
                config: tokio::sync::Mutex::new(cfg.clone()),
                config_dir,
                suppress: Arc::new(StdMutex::new(HashMap::new())),
                watcher_stop: StdMutex::new(None),
            };
            app.manage(state);
            watcher::rebuild(app.handle(), &cfg);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::update_settings,
            commands::add_watched_folder,
            commands::remove_watched_folder,
            commands::add_game,
            commands::remove_game,
            commands::set_game_auto_upload,
            commands::backup_folder_now,
            commands::upload_game_save,
            commands::list_game_versions,
            commands::restore_game_version,
            commands::gdrive_connect,
            commands::gdrive_disconnect,
            commands::custom_cloud_login,
            commands::custom_cloud_logout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
