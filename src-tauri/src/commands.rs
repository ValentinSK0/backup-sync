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
    let previous = sync::snapshot(&app).await;
    settings.gdrive_client_id = previous.settings.gdrive_client_id;
    settings.gdrive_client_secret_configured = previous.settings.gdrive_client_secret_configured;
    let cfg = sync::with_config(&app, |c| {
        c.settings = settings;
    })
    .await?;
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn save_gdrive_credentials(
    app: AppHandle,
    client_id: String,
    client_secret: String,
) -> CmdResult<AppConfig> {
    let client_id = client_id.trim().to_string();
    let submitted_secret = client_secret.trim();
    if client_id.is_empty() {
        return Err("Vyplň Google OAuth Client ID.".into());
    }
    if !client_id.ends_with(".apps.googleusercontent.com") {
        return Err("Google OAuth Client ID musí končiť na .apps.googleusercontent.com.".into());
    }

    let previous = sync::snapshot(&app).await;
    let previous_secret = storage::keyring_get(gdrive::CLIENT_SECRET_KEY)
        .ok()
        .filter(|secret| !secret.trim().is_empty());
    let client_changed = previous.settings.gdrive_client_id != client_id;
    if client_changed && submitted_secret.is_empty() {
        return Err("Pri zmene Client ID zadaj aj jeho Client Secret.".into());
    }
    let desired_secret = if submitted_secret.is_empty() {
        previous_secret
            .clone()
            .ok_or_else(|| "Vyplň Google OAuth Client Secret.".to_string())?
    } else {
        submitted_secret.to_string()
    };
    let secret_changed = previous_secret.as_deref() != Some(desired_secret.as_str());
    if (client_changed || secret_changed) && sync::has_active_syncs(&app) {
        return Err("Počkaj na dokončenie synchronizácie a potom zmeň Google údaje.".into());
    }

    storage::keyring_set(gdrive::CLIENT_SECRET_KEY, &desired_secret)?;
    let result = sync::with_config(&app, |c| {
        c.settings.gdrive_client_id = client_id;
        c.settings.gdrive_client_secret_configured = true;
        if client_changed || secret_changed {
            c.providers.gdrive = Default::default();
        }
    })
    .await;

    let config = match result {
        Ok(config) => config,
        Err(error) => {
            if let Some(secret) = previous_secret {
                let _ = storage::keyring_set(gdrive::CLIENT_SECRET_KEY, &secret);
            } else {
                storage::keyring_delete(gdrive::CLIENT_SECRET_KEY);
            }
            return Err(error);
        }
    };
    if client_changed || secret_changed {
        storage::keyring_delete(gdrive::REFRESH_TOKEN_KEY);
    }
    Ok(config)
}

fn canonical_directory(path: &str) -> CmdResult<PathBuf> {
    let path = Path::new(path);
    if !path.is_dir() {
        return Err(format!("Priečinok neexistuje: {}", path.display()));
    }
    std::fs::canonicalize(path).map_err(|e| format!("Priečinok sa nedá otvoriť: {e}"))
}

fn ensure_directory(path: &str) -> CmdResult<PathBuf> {
    let path = Path::new(path.trim());
    if path.as_os_str().is_empty() {
        return Err("Vyber priečinok pre lokálne ZIP zálohy.".into());
    }
    std::fs::create_dir_all(path)
        .map_err(|e| format!("Priečinok pre lokálne ZIP zálohy sa nedá vytvoriť: {e}"))?;
    canonical_directory(path.to_string_lossy().as_ref())
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
    backup_path: String,
    provider: Provider,
) -> CmdResult<AppConfig> {
    if name.trim().is_empty() {
        return Err("Zadaj názov hry.".into());
    }
    let canonical = canonical_directory(&save_path)?;
    let backup_canonical = ensure_directory(&backup_path)?;
    if backup_canonical.starts_with(&canonical) {
        return Err("ZIP zálohy nemôžu byť uložené vo vnútri save priečinka.".into());
    }
    let current = sync::snapshot(&app).await;
    if current.games.iter().any(|game| {
        canonical_directory(&game.save_path).is_ok_and(|existing| existing == canonical)
    }) {
        return Err("Tento save priečinok už je pridaný k hre.".into());
    }
    let cloud_name = sync::game_cloud_name(name.trim());
    let backup_path = backup_canonical.to_string_lossy().to_string();
    let cfg = sync::with_config(&app, |c| {
        c.games.push(crate::config::Game {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.trim().to_string(),
            save_path,
            cloud_name,
            backup_path,
            provider,
            auto_upload: None,
            sync_state: SyncState::Changed,
            last_change: Some(now_iso()),
            last_upload: None,
            last_cloud_version_id: None,
            last_error: None,
            local: Default::default(),
        });
    })
    .await?;
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn update_game(
    app: AppHandle,
    id: String,
    name: String,
    save_path: String,
    backup_path: String,
    provider: Provider,
) -> CmdResult<AppConfig> {
    if sync::is_sync_active(&app, "game", &id) {
        return Err("Počkaj, kým sa synchronizácia hry dokončí.".into());
    }
    let name = name.trim();
    if name.is_empty() {
        return Err("Zadaj názov hry.".into());
    }
    let save_canonical = canonical_directory(&save_path)?;
    let backup_canonical = ensure_directory(&backup_path)?;
    if backup_canonical.starts_with(&save_canonical) {
        return Err("ZIP zálohy nemôžu byť uložené vo vnútri save priečinka.".into());
    }
    let current = sync::snapshot(&app).await;
    let existing = current
        .games
        .iter()
        .find(|game| game.id == id)
        .ok_or_else(|| "Hra sa nenašla.".to_string())?;
    if current.games.iter().any(|game| {
        game.id != id
            && canonical_directory(&game.save_path).is_ok_and(|path| path == save_canonical)
    }) {
        return Err("Tento save priečinok už je pridaný k inej hre.".into());
    }
    let save_changed = canonical_directory(&existing.save_path)
        .map(|path| path != save_canonical)
        .unwrap_or(true);
    let provider_changed = existing.provider != provider;
    let backup_path = backup_canonical.to_string_lossy().to_string();
    let name = name.to_string();
    let cfg = sync::with_config(&app, |config| {
        if let Some(game) = config.games.iter_mut().find(|game| game.id == id) {
            if game.cloud_name.trim().is_empty() {
                game.cloud_name = sync::game_cloud_name(&game.name);
            }
            game.name = name;
            game.save_path = save_path;
            game.backup_path = backup_path;
            if save_changed || provider_changed {
                game.provider = provider;
                game.sync_state = SyncState::Changed;
                game.last_change = Some(now_iso());
                game.last_upload = None;
                game.last_cloud_version_id = None;
                game.last_error = None;
                game.local.uploaded_hash = None;
                if save_changed {
                    game.local = Default::default();
                }
            }
        }
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
pub async fn list_folder_backups(
    app: AppHandle,
    provider: Provider,
) -> CmdResult<Vec<sync::CloudFolderBackup>> {
    sync::list_folder_backups(&app, provider).await
}

#[tauri::command]
pub async fn connect_folder_backup(
    app: AppHandle,
    provider: Provider,
    cloud_id: String,
    path: String,
) -> CmdResult<AppConfig> {
    let cfg = sync::connect_folder_backup(&app, provider, &cloud_id, &path).await?;
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
pub async fn backup_game_local(app: AppHandle, id: String) -> CmdResult<AppConfig> {
    sync::backup_game_local(&app, &id).await?;
    Ok(sync::snapshot(&app).await)
}

#[tauri::command]
pub async fn list_game_versions(app: AppHandle, id: String) -> CmdResult<Vec<RemoteFile>> {
    sync::list_game_versions(&app, &id).await
}

#[tauri::command]
pub async fn list_game_cloud_statuses(app: AppHandle) -> CmdResult<Vec<sync::GameCloudStatus>> {
    sync::list_game_cloud_statuses(&app).await
}

#[tauri::command]
pub async fn restore_game_version(app: AppHandle, id: String, version_id: String) -> CmdResult<()> {
    sync::restore_game_version(&app, &id, &version_id).await
}

/// Otvorí v prieskumníkovi priečinok, ktorý aplikácia sleduje (save hry alebo zálohovaný priečinok).
#[tauri::command]
pub async fn open_local_folder(app: AppHandle, path: String) -> CmdResult<()> {
    let target = canonical_directory(&path)?;
    let cfg = sync::snapshot(&app).await;
    let known = cfg.games.iter().any(|game| {
        canonical_directory(&game.save_path).is_ok_and(|dir| dir == target)
            || canonical_directory(&game.backup_path).is_ok_and(|dir| dir == target)
    }) || cfg
        .watched_folders
        .iter()
        .any(|folder| canonical_directory(&folder.path).is_ok_and(|dir| dir == target));
    if !known {
        return Err("Tento priečinok aplikácia nesleduje.".into());
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| format!("Priečinok sa nepodarilo otvoriť: {e}"))
}

#[tauri::command]
pub async fn list_game_backups(
    app: AppHandle,
    provider: Provider,
) -> CmdResult<Vec<sync::CloudGameBackup>> {
    sync::list_game_backups(&app, provider).await
}

#[tauri::command]
pub async fn connect_game_backup(
    app: AppHandle,
    provider: Provider,
    cloud_id: String,
    path: String,
    name: String,
    backup_path: String,
) -> CmdResult<AppConfig> {
    let destination = canonical_directory(&path)?;
    let backups = ensure_directory(&backup_path)?;
    if backups.starts_with(destination) {
        return Err("ZIP zálohy nemôžu byť uložené vo vnútri save priečinka.".into());
    }
    let cfg = sync::connect_game_backup(
        &app,
        provider,
        &cloud_id,
        &path,
        &name,
        &backups.to_string_lossy(),
    )
    .await?;
    watcher::rebuild(&app, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn gdrive_connect(app: AppHandle) -> CmdResult<AppConfig> {
    let settings = sync::snapshot(&app).await.settings;
    let client_secret = storage::keyring_get(gdrive::CLIENT_SECRET_KEY)
        .map_err(|_| "Chýba Google OAuth Client Secret. Ulož ho v Nastaveniach.".to_string())?;
    let (refresh_token, account) =
        gdrive::connect(&app, &settings.gdrive_client_id, &client_secret).await?;
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
pub async fn ulozto_login(
    app: AppHandle,
    login: String,
    auth_token: String,
    app_token: String,
) -> CmdResult<AppConfig> {
    use crate::storage::ulozto;
    let login = login.trim().to_string();
    let auth_token = auth_token.trim().to_string();
    let app_token = {
        let trimmed = app_token.trim();
        if trimmed.is_empty() {
            ulozto::TEST_APP_TOKEN.to_string()
        } else {
            trimmed.to_string()
        }
    };
    if login.is_empty() || auth_token.is_empty() {
        return Err("Vyplň Ulož.to login aj aplikačný token.".into());
    }
    // over prihlásenie ešte pred uložením tokenov; session si odlož na ďalšie volania
    let session = ulozto::login(&login, &auth_token, &app_token).await?;
    storage::keyring_set(ulozto::AUTH_TOKEN_KEY, &auth_token)?;
    storage::keyring_set(ulozto::APP_TOKEN_KEY, &app_token)?;
    let _ = storage::keyring_set(ulozto::SESSION_KEY, &session);
    let result = sync::with_config(&app, |c| {
        c.providers.ulozto.connected = true;
        c.providers.ulozto.account = Some(login);
        c.providers.ulozto.error = None;
    })
    .await;
    if result.is_err() {
        storage::keyring_delete(ulozto::AUTH_TOKEN_KEY);
        storage::keyring_delete(ulozto::APP_TOKEN_KEY);
        storage::keyring_delete(ulozto::SESSION_KEY);
    }
    result
}

#[tauri::command]
pub async fn ulozto_logout(app: AppHandle) -> CmdResult<AppConfig> {
    use crate::storage::ulozto;
    if sync::has_active_syncs(&app) {
        return Err("Počkaj na dokončenie synchronizácie a potom odpoj Ulož.to.".into());
    }
    let config = sync::with_config(&app, |c| {
        c.providers.ulozto = Default::default();
    })
    .await?;
    storage::keyring_delete(ulozto::AUTH_TOKEN_KEY);
    storage::keyring_delete(ulozto::APP_TOKEN_KEY);
    storage::keyring_delete(ulozto::SESSION_KEY);
    Ok(config)
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
