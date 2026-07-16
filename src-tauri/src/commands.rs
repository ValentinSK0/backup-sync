use crate::config::{now_iso, AppConfig, Provider, Settings, SyncState};
use crate::storage::{self, custom, gdrive, RemoteFile};
use crate::sync;
use crate::watcher;
use crate::AppState;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

type CmdResult<T> = Result<T, String>;

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> CmdResult<AppConfig> {
    Ok(state.config.lock().await.clone())
}

#[tauri::command]
pub async fn update_settings(app: AppHandle, settings: Settings) -> CmdResult<AppConfig> {
    if !(1..=3600).contains(&settings.debounce_seconds) {
        return Err("Oneskorenie musí byť celé číslo od 1 do 3600 sekúnd.".into());
    }
    let mut settings = settings;
    settings.gdrive_client_id = settings.gdrive_client_id.trim().to_string();
    let previous = sync::snapshot(&app).await;
    let client_changed = previous.settings.gdrive_client_id != settings.gdrive_client_id;
    if client_changed && sync::has_active_syncs(&app) {
        return Err("Počkaj na dokončenie synchronizácie a potom zmeň Google Client ID.".into());
    }
    let cfg = sync::with_config(&app, |c| {
        c.settings = settings;
        if client_changed {
            c.providers.gdrive = Default::default();
        }
    })
    .await?;
    if client_changed {
        storage::keyring_delete(gdrive::REFRESH_TOKEN_KEY);
    }
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

fn canonical_directory(path: &str) -> CmdResult<PathBuf> {
    let path = Path::new(path);
    if !path.is_dir() {
        return Err(format!("Priečinok neexistuje: {}", path.display()));
    }
    std::fs::canonicalize(path).map_err(|e| format!("Priečinok sa nedá otvoriť: {e}"))
}

#[tauri::command]
pub async fn add_watched_folder(
    app: AppHandle,
    path: String,
    provider: Provider,
) -> CmdResult<AppConfig> {
    let canonical = canonical_directory(&path)?;
    let current = sync::snapshot(&app).await;
    if current
        .watched_folders
        .iter()
        .any(|folder| canonical_directory(&folder.path).is_ok_and(|existing| existing == canonical))
    {
        return Err("Tento priečinok už aplikácia sleduje.".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let cloud_name = sync::folder_cloud_name(&path, &id);
    let cfg = sync::with_config(&app, |c| {
        c.watched_folders.push(crate::config::WatchedFolder {
            id,
            path,
            provider,
            cloud_id: None,
            cloud_name,
            auto_backup: None,
            sync_state: SyncState::Changed,
            last_sync: None,
            last_error: None,
            last_error_action: None,
        });
    })
    .await?;
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn remove_watched_folder(app: AppHandle, id: String) -> CmdResult<AppConfig> {
    if sync::is_sync_active(&app, "folder", &id) {
        return Err("Počkaj, kým sa zálohovanie priečinka dokončí.".into());
    }
    let cfg = sync::with_config(&app, |c| c.watched_folders.retain(|f| f.id != id)).await?;
    let state = app.state::<AppState>();
    let _ = std::fs::remove_file(state.state_dir().join(format!("folder-{id}.json")));
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn set_folder_auto_backup(
    app: AppHandle,
    id: String,
    value: Option<bool>,
) -> CmdResult<AppConfig> {
    sync::with_config(&app, |c| {
        if let Some(f) = c.watched_folders.iter_mut().find(|f| f.id == id) {
            f.auto_backup = value;
        }
    })
    .await
}

#[tauri::command]
pub async fn add_game(
    app: AppHandle,
    name: String,
    save_path: String,
    provider: Provider,
) -> CmdResult<AppConfig> {
    if name.trim().is_empty() {
        return Err("Zadaj názov hry.".into());
    }
    let canonical = canonical_directory(&save_path)?;
    let current = sync::snapshot(&app).await;
    if current.games.iter().any(|game| {
        canonical_directory(&game.save_path).is_ok_and(|existing| existing == canonical)
    }) {
        return Err("Tento save priečinok už je pridaný k hre.".into());
    }
    let cfg = sync::with_config(&app, |c| {
        c.games.push(crate::config::Game {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.trim().to_string(),
            save_path,
            provider,
            auto_upload: None,
            sync_state: SyncState::Changed,
            last_change: Some(now_iso()),
            last_upload: None,
            last_error: None,
        });
    })
    .await?;
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn remove_game(app: AppHandle, id: String) -> CmdResult<AppConfig> {
    if sync::is_sync_active(&app, "game", &id) {
        return Err("Počkaj, kým sa synchronizácia hry dokončí.".into());
    }
    let cfg = sync::with_config(&app, |c| c.games.retain(|g| g.id != id)).await?;
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn set_game_auto_upload(
    app: AppHandle,
    id: String,
    value: Option<bool>,
) -> CmdResult<AppConfig> {
    sync::with_config(&app, |c| {
        if let Some(g) = c.games.iter_mut().find(|g| g.id == id) {
            g.auto_upload = value;
        }
    })
    .await
}

#[tauri::command]
pub async fn backup_folder_now(app: AppHandle, id: String) -> CmdResult<AppConfig> {
    sync::backup_folder(&app, &id).await?;
    Ok(sync::snapshot(&app).await)
}

#[tauri::command]
pub async fn list_folder_backups(app: AppHandle) -> CmdResult<Vec<sync::CloudFolderBackup>> {
    sync::list_folder_backups(&app, Provider::Gdrive).await
}

#[tauri::command]
pub async fn connect_folder_backup(
    app: AppHandle,
    cloud_id: String,
    path: String,
) -> CmdResult<AppConfig> {
    let cfg = sync::connect_folder_backup(&app, Provider::Gdrive, &cloud_id, &path).await?;
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn restore_folder_from_cloud(app: AppHandle, id: String) -> CmdResult<AppConfig> {
    sync::restore_folder_from_cloud(&app, &id).await?;
    Ok(sync::snapshot(&app).await)
}

#[tauri::command]
pub async fn upload_game_save(app: AppHandle, id: String) -> CmdResult<AppConfig> {
    sync::upload_game(&app, &id).await?;
    Ok(sync::snapshot(&app).await)
}

#[tauri::command]
pub async fn list_game_versions(app: AppHandle, id: String) -> CmdResult<Vec<RemoteFile>> {
    sync::list_game_versions(&app, &id).await
}

#[tauri::command]
pub async fn restore_game_version(app: AppHandle, id: String, version_id: String) -> CmdResult<()> {
    sync::restore_game_version(&app, &id, &version_id).await
}

#[tauri::command]
pub async fn gdrive_connect(app: AppHandle) -> CmdResult<AppConfig> {
    let settings = sync::snapshot(&app).await.settings;
    let (refresh_token, account) = gdrive::connect(&app, &settings.gdrive_client_id).await?;
    storage::keyring_set(gdrive::REFRESH_TOKEN_KEY, &refresh_token)?;
    let result = sync::with_config(&app, |c| {
        c.providers.gdrive.connected = true;
        c.providers.gdrive.account = Some(account);
        c.providers.gdrive.error = None;
    })
    .await;
    if result.is_err() {
        storage::keyring_delete(gdrive::REFRESH_TOKEN_KEY);
    }
    result
}

#[tauri::command]
pub async fn gdrive_disconnect(app: AppHandle) -> CmdResult<AppConfig> {
    if sync::has_active_syncs(&app) {
        return Err("Počkaj na dokončenie synchronizácie a potom odpoj Google Drive.".into());
    }
    let config = sync::with_config(&app, |c| {
        c.providers.gdrive = Default::default();
    })
    .await?;
    storage::keyring_delete(gdrive::REFRESH_TOKEN_KEY);
    Ok(config)
}

#[tauri::command]
pub async fn custom_cloud_login(
    app: AppHandle,
    endpoint: String,
    username: String,
    password: String,
) -> CmdResult<AppConfig> {
    let username = username.trim().to_string();
    if username.is_empty() || password.is_empty() {
        return Err("Vyplň používateľské meno aj heslo.".into());
    }
    let endpoint = custom::normalize_endpoint(&endpoint)?;
    let token = custom::login(&endpoint, &username, &password).await?;
    storage::keyring_set(custom::TOKEN_KEY, &token)?;
    let result = sync::with_config(&app, |c| {
        c.providers.custom_cloud.connected = true;
        c.providers.custom_cloud.endpoint = Some(endpoint);
        c.providers.custom_cloud.account = Some(username);
        c.providers.custom_cloud.error = None;
    })
    .await;
    if result.is_err() {
        storage::keyring_delete(custom::TOKEN_KEY);
    }
    result
}

#[tauri::command]
pub async fn custom_cloud_logout(app: AppHandle) -> CmdResult<AppConfig> {
    if sync::has_active_syncs(&app) {
        return Err("Počkaj na dokončenie synchronizácie a potom odpoj cloud.".into());
    }
    let config = sync::with_config(&app, |c| {
        c.providers.custom_cloud = Default::default();
    })
    .await?;
    storage::keyring_delete(custom::TOKEN_KEY);
    Ok(config)
}
