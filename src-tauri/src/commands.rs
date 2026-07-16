use crate::config::{now_iso, AppConfig, Provider, Settings, SyncState};
use crate::storage::{self, custom, gdrive, RemoteFile};
use crate::sync;
use crate::watcher;
use crate::AppState;
use tauri::{AppHandle, Manager, State};

type CmdResult<T> = Result<T, String>;

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> CmdResult<AppConfig> {
    Ok(state.config.lock().await.clone())
}

#[tauri::command]
pub async fn update_settings(app: AppHandle, settings: Settings) -> CmdResult<AppConfig> {
    let cfg = sync::with_config(&app, |c| c.settings = settings).await;
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn add_watched_folder(app: AppHandle, path: String, provider: Provider) -> CmdResult<AppConfig> {
    if !std::path::Path::new(&path).is_dir() {
        return Err(format!("Priečinok neexistuje: {path}"));
    }
    let cfg = sync::with_config(&app, |c| {
        c.watched_folders.push(crate::config::WatchedFolder {
            id: uuid::Uuid::new_v4().to_string(),
            path,
            provider,
            sync_state: SyncState::Changed,
            last_sync: None,
            last_error: None,
        });
    })
    .await;
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn remove_watched_folder(app: AppHandle, id: String) -> CmdResult<AppConfig> {
    let cfg = sync::with_config(&app, |c| c.watched_folders.retain(|f| f.id != id)).await;
    let state = app.state::<AppState>();
    let _ = std::fs::remove_file(state.state_dir().join(format!("folder-{id}.json")));
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn add_game(app: AppHandle, name: String, save_path: String, provider: Provider) -> CmdResult<AppConfig> {
    if name.trim().is_empty() {
        return Err("Zadaj názov hry.".into());
    }
    if !std::path::Path::new(&save_path).is_dir() {
        return Err(format!("Priečinok neexistuje: {save_path}"));
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
    .await;
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn remove_game(app: AppHandle, id: String) -> CmdResult<AppConfig> {
    let cfg = sync::with_config(&app, |c| c.games.retain(|g| g.id != id)).await;
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn set_game_auto_upload(app: AppHandle, id: String, value: Option<bool>) -> CmdResult<AppConfig> {
    Ok(sync::with_config(&app, |c| {
        if let Some(g) = c.games.iter_mut().find(|g| g.id == id) {
            g.auto_upload = value;
        }
    })
    .await)
}

#[tauri::command]
pub async fn backup_folder_now(app: AppHandle, id: String) -> CmdResult<AppConfig> {
    sync::backup_folder(&app, &id).await?;
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
    let (refresh_token, email) =
        gdrive::connect(&app, &settings.gdrive_client_id, &settings.gdrive_client_secret).await?;
    storage::keyring_set(gdrive::REFRESH_TOKEN_KEY, &refresh_token)?;
    Ok(sync::with_config(&app, |c| {
        c.providers.gdrive.connected = true;
        c.providers.gdrive.account = Some(email);
        c.providers.gdrive.error = None;
    })
    .await)
}

#[tauri::command]
pub async fn gdrive_disconnect(app: AppHandle) -> CmdResult<AppConfig> {
    storage::keyring_delete(gdrive::REFRESH_TOKEN_KEY);
    Ok(sync::with_config(&app, |c| {
        c.providers.gdrive = Default::default();
    })
    .await)
}

#[tauri::command]
pub async fn custom_cloud_login(
    app: AppHandle,
    endpoint: String,
    username: String,
    password: String,
) -> CmdResult<AppConfig> {
    let endpoint = endpoint.trim().to_string();
    if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
        return Err("Adresa cloudu musí začínať http:// alebo https://".into());
    }
    let token = custom::login(&endpoint, &username, &password).await?;
    storage::keyring_set(custom::TOKEN_KEY, &token)?;
    Ok(sync::with_config(&app, |c| {
        c.providers.custom_cloud.connected = true;
        c.providers.custom_cloud.endpoint = Some(endpoint);
        c.providers.custom_cloud.account = Some(username);
        c.providers.custom_cloud.error = None;
    })
    .await)
}

#[tauri::command]
pub async fn custom_cloud_logout(app: AppHandle) -> CmdResult<AppConfig> {
    storage::keyring_delete(custom::TOKEN_KEY);
    Ok(sync::with_config(&app, |c| {
        c.providers.custom_cloud = Default::default();
    })
    .await)
}
