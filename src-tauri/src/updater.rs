use crate::{sync, AppState};
use serde::Serialize;
use std::time::{Duration, Instant};
use tauri::{ipc::Channel, AppHandle, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct UpdateState(tokio::sync::Mutex<UpdateSession>);

#[derive(Default)]
struct UpdateSession {
    update: Option<Update>,
    bytes: Option<Vec<u8>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    version: String,
    notes: Option<String>,
    downloaded: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    current_version: String,
    feed_available: bool,
    update: Option<AvailableUpdate>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

impl UpdateSession {
    fn info(&self, app: &AppHandle, feed_available: bool) -> AppUpdateInfo {
        AppUpdateInfo {
            current_version: app.package_info().version.to_string(),
            feed_available,
            update: self.update.as_ref().map(|update| AvailableUpdate {
                version: update.version.clone(),
                notes: update.body.clone(),
                downloaded: self.bytes.is_some(),
            }),
        }
    }
}

#[tauri::command]
pub async fn check_app_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<AppUpdateInfo, String> {
    let mut session = state
        .0
        .try_lock()
        .map_err(|_| "Aktualizácia sa už spracúva.".to_string())?;
    if session.bytes.is_some() {
        return Ok(session.info(&app, true));
    }
    let result = app
        .updater_builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| error.to_string())?
        .check()
        .await;
    match result {
        Ok(update) => {
            session.update = update;
            Ok(session.info(&app, true))
        }
        // Releases published before the updater have no latest.json asset.
        Err(tauri_plugin_updater::Error::ReleaseNotFound) => {
            session.update = None;
            Ok(session.info(&app, false))
        }
        Err(error) => Err(format!("Kontrola aktualizácií zlyhala: {error}")),
    }
}

#[tauri::command]
pub async fn download_app_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
    on_progress: Channel<UpdateProgress>,
) -> Result<AppUpdateInfo, String> {
    let mut session = state
        .0
        .try_lock()
        .map_err(|_| "Aktualizácia sa už spracúva.".to_string())?;
    if session.bytes.is_some() {
        return Ok(session.info(&app, true));
    }
    let mut update = session
        .update
        .clone()
        .ok_or("Najprv vyhľadajte novú verziu.")?;
    update.timeout = Some(Duration::from_secs(180));
    let mut downloaded = 0u64;
    let mut last_event = Instant::now() - Duration::from_secs(1);
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                if last_event.elapsed() >= Duration::from_millis(100) || total == Some(downloaded) {
                    let _ = on_progress.send(UpdateProgress { downloaded, total });
                    last_event = Instant::now();
                }
            },
            || {},
        )
        .await
        .map_err(|error| format!("Stiahnutie alebo overenie podpisu zlyhalo: {error}"))?;
    // download() verifies the signature before returning the bytes.
    session.bytes = Some(bytes);
    Ok(session.info(&app, true))
}

#[tauri::command]
pub async fn install_app_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<(), String> {
    let session = state
        .0
        .try_lock()
        .map_err(|_| "Aktualizácia sa už spracúva.".to_string())?;
    let update = session
        .update
        .as_ref()
        .ok_or("Najprv vyhľadajte novú verziu.")?;
    let bytes = session
        .bytes
        .as_ref()
        .ok_or("Najprv stiahnite aktualizáciu.")?;
    let app_state = app.state::<AppState>();
    let _install_guard = sync::begin_app_update_install(app_state.active_syncs.clone())?;
    let _config_guard = app_state.config.lock().await;
    // On Windows the plugin launches NSIS, exits this process and relaunches the app.
    update
        .install(bytes)
        .map_err(|error| format!("Inštaláciu sa nepodarilo spustiť: {error}"))
}
