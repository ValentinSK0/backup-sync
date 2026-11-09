mod archive;
mod artwork;
mod commands;
mod config;
mod game_save;
mod storage;
mod sync;
mod tray;
mod updater;
mod watcher;

use config::{AppConfig, SyncState};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Instant;
use tauri::Manager;

pub struct AppState {
    pub artwork: artwork::ArtworkService,
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
        // druhé spustenie appky iba zobrazí okno bežiacej inštancie (musí byť prvý plugin)
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::UpdateState::default())
        // zatvorenie okna appku neukončí, iba schová do tray
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
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
                if g.cloud_name.is_empty() {
                    g.cloud_name = sync::game_cloud_name(&g.name);
                }
                if g.backup_path.is_empty() {
                    g.backup_path = sync::default_game_backup_path(&g.save_path);
                }
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
                artwork: artwork::ArtworkService::default(),
                config: tokio::sync::Mutex::new(cfg.clone()),
                config_dir,
                suppress: Arc::new(StdMutex::new(HashMap::new())),
                active_syncs: Arc::new(StdMutex::new(HashSet::new())),
                pending_changes: Arc::new(StdMutex::new(HashSet::new())),
                watcher_stop: StdMutex::new(None),
            };
            app.manage(state);
            tray::setup(app.handle())?;
            watcher::rebuild(app.handle(), &cfg);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            updater::check_app_update,
            updater::download_app_update,
            updater::install_app_update,
            commands::get_config,
            commands::update_settings,
            commands::save_gdrive_credentials,
            artwork::search_artwork_games,
            artwork::list_artwork_images,
            artwork::load_game_artwork,
            artwork::select_game_artwork,
            artwork::clear_game_artwork,
            commands::add_watched_folder,
            commands::remove_watched_folder,
            commands::set_folder_auto_backup,
            commands::add_game,
            commands::update_game,
            commands::remove_game,
            commands::set_game_auto_upload,
            commands::backup_folder_now,
            commands::list_folder_backups,
            commands::connect_folder_backup,
            commands::restore_folder_from_cloud,
            commands::upload_game_save,
            commands::backup_game_local,
            commands::list_game_versions,
            commands::list_game_cloud_statuses,
            commands::restore_game_version,
            commands::restore_latest_game_version,
            commands::list_game_backups,
            commands::connect_game_backup,
            commands::open_local_folder,
            commands::gdrive_connect,
            commands::gdrive_disconnect,
            commands::custom_cloud_login,
            commands::custom_cloud_logout,
            commands::ulozto_login,
            commands::ulozto_logout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
