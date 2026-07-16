use crate::config::{now_iso, AppConfig, Provider, SyncState};
use crate::storage::{make_provider, RemoteFile};
use crate::AppState;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub target_type: &'static str,
    pub id: String,
    pub status: &'static str,
    pub message: Option<String>,
}

pub fn emit_progress(app: &AppHandle, target_type: &'static str, id: &str, status: &'static str, message: Option<String>) {
    let _ = app.emit(
        "sync-progress",
        SyncProgress {
            target_type,
            id: id.to_string(),
            status,
            message,
        },
    );
}

/// Zamkne config, aplikuje zmenu, uloží, vyšle config-changed a vráti snapshot.
pub async fn with_config<F: FnOnce(&mut AppConfig)>(app: &AppHandle, f: F) -> AppConfig {
    let state = app.state::<AppState>();
    let snapshot = {
        let mut cfg = state.config.lock().await;
        f(&mut cfg);
        let _ = cfg.save(&state.config_path());
        cfg.clone()
    };
    let _ = app.emit("config-changed", &snapshot);
    snapshot
}

pub async fn snapshot(app: &AppHandle) -> AppConfig {
    app.state::<AppState>().config.lock().await.clone()
}

/// Súbory, ktoré watcher aj sync ignorujú (dočasné/lock súbory).
pub fn is_ignored_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".tmp")
        || lower.ends_with(".temp")
        || lower.ends_with(".part")
        || lower.ends_with(".crswap")
        || lower.ends_with(".lock")
        || lower.ends_with(".swp")
        || lower.starts_with('~')
        || lower == "thumbs.db"
        || lower == "desktop.ini"
}

fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let trimmed = cleaned.trim_matches('-').to_string();
    if trimmed.is_empty() { "bez-mena".into() } else { trimmed }
}

fn folder_remote_dir(path: &str, id: &str) -> Vec<String> {
    let base = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "priecinok".into());
    vec![
        "BackupSync".into(),
        "folders".into(),
        format!("{}-{}", sanitize(&base), &id[..8.min(id.len())]),
    ]
}

fn game_remote_dir(name: &str) -> Vec<String> {
    vec!["BackupSync".into(), "games".into(), sanitize(name)]
}

fn hash_state_path(app: &AppHandle, folder_id: &str) -> PathBuf {
    app.state::<AppState>()
        .state_dir()
        .join(format!("folder-{folder_id}.json"))
}

fn load_hash_state(path: &Path) -> HashMap<String, String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_hash_state(path: &Path, map: &HashMap<String, String>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, serde_json::to_string(map).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

/// Prejde priečinok a vráti (rel_cesta, abs_cesta, sha256) pre každý súbor.
fn scan_folder(root: &Path) -> Result<Vec<(String, PathBuf, String)>, String> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if is_ignored_file(&name) {
            continue;
        }
        let abs = entry.path().to_path_buf();
        let rel = abs
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let mut hasher = Sha256::new();
        let mut file = std::fs::File::open(&abs).map_err(|e| e.to_string())?;
        std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
        let hash = format!("{:x}", hasher.finalize());
        out.push((rel, abs, hash));
    }
    Ok(out)
}

fn set_folder_result(cfg: &mut AppConfig, id: &str, result: &Result<(), String>) {
    if let Some(f) = cfg.watched_folders.iter_mut().find(|f| f.id == id) {
        match result {
            Ok(()) => {
                f.sync_state = SyncState::Synced;
                f.last_sync = Some(now_iso());
                f.last_error = None;
            }
            Err(e) => {
                f.sync_state = SyncState::Error;
                f.last_error = Some(e.clone());
            }
        }
    }
}

/// Inkrementálna záloha priečinka: nahrá len súbory, ktorých hash sa zmenil.
pub async fn backup_folder(app: &AppHandle, id: &str) -> Result<(), String> {
    let cfg = with_config(app, |c| {
        if let Some(f) = c.watched_folders.iter_mut().find(|f| f.id == id) {
            f.sync_state = SyncState::Syncing;
        }
    })
    .await;
    let folder = cfg
        .watched_folders
        .iter()
        .find(|f| f.id == id)
        .cloned()
        .ok_or_else(|| "Priečinok sa nenašiel.".to_string())?;

    emit_progress(app, "folder", id, "uploading", Some("Kontrolujem zmeny…".into()));
    let result = backup_folder_inner(app, &folder.path, &folder.id, folder.provider, &cfg).await;

    with_config(app, |c| set_folder_result(c, id, &result)).await;
    match &result {
        Ok(()) => emit_progress(app, "folder", id, "done", Some("Záloha hotová".into())),
        Err(e) => emit_progress(app, "folder", id, "error", Some(e.clone())),
    }
    result
}

async fn backup_folder_inner(
    app: &AppHandle,
    path: &str,
    id: &str,
    provider: Provider,
    cfg: &AppConfig,
) -> Result<(), String> {
    let root = PathBuf::from(path);
    let files = {
        let root = root.clone();
        tokio::task::spawn_blocking(move || scan_folder(&root))
            .await
            .map_err(|e| e.to_string())??
    };

    let state_path = hash_state_path(app, id);
    let mut hashes = load_hash_state(&state_path);
    let changed: Vec<_> = files
        .iter()
        .filter(|(rel, _, hash)| hashes.get(rel) != Some(hash))
        .cloned()
        .collect();

    if changed.is_empty() {
        return Ok(());
    }

    let storage = make_provider(provider, cfg)?;
    let base = folder_remote_dir(path, id);
    let total = changed.len();
    for (i, (rel, abs, hash)) in changed.into_iter().enumerate() {
        let mut dir = base.clone();
        let rel_path = Path::new(&rel);
        for comp in rel_path.parent().into_iter().flat_map(|p| p.components()) {
            dir.push(comp.as_os_str().to_string_lossy().to_string());
        }
        let name = rel_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| format!("Neplatné meno súboru: {rel}"))?;
        emit_progress(
            app,
            "folder",
            id,
            "uploading",
            Some(format!("({}/{}) {}", i + 1, total, rel)),
        );
        storage.upload_file(&dir, &name, &abs).await?;
        hashes.insert(rel, hash);
        // priebežne ukladaj stav, nech prerušenie nezahodí hotové uploady
        let _ = save_hash_state(&state_path, &hashes);
    }
    save_hash_state(&state_path, &hashes)?;
    Ok(())
}

fn set_game_result(cfg: &mut AppConfig, id: &str, result: &Result<(), String>) {
    if let Some(g) = cfg.games.iter_mut().find(|g| g.id == id) {
        match result {
            Ok(()) => {
                g.sync_state = SyncState::Synced;
                g.last_upload = Some(now_iso());
                g.last_error = None;
            }
            Err(e) => {
                g.sync_state = SyncState::Error;
                g.last_error = Some(e.clone());
            }
        }
    }
}

/// Zabalí save priečinok do zipu a nahrá ako novú verziu na cloud.
pub async fn upload_game(app: &AppHandle, id: &str) -> Result<(), String> {
    let cfg = with_config(app, |c| {
        if let Some(g) = c.games.iter_mut().find(|g| g.id == id) {
            g.sync_state = SyncState::Syncing;
        }
    })
    .await;
    let game = cfg
        .games
        .iter()
        .find(|g| g.id == id)
        .cloned()
        .ok_or_else(|| "Hra sa nenašla.".to_string())?;

    emit_progress(app, "game", id, "uploading", Some("Balím save…".into()));
    let result = upload_game_inner(app, &game, &cfg).await;

    with_config(app, |c| set_game_result(c, id, &result)).await;
    match &result {
        Ok(()) => emit_progress(app, "game", id, "done", Some("Save nahraný".into())),
        Err(e) => emit_progress(app, "game", id, "error", Some(e.clone())),
    }
    result
}

async fn upload_game_inner(app: &AppHandle, game: &crate::config::Game, cfg: &AppConfig) -> Result<(), String> {
    let save_path = PathBuf::from(&game.save_path);
    let level = cfg.settings.compression_level;
    let zip_path = {
        let save_path = save_path.clone();
        tokio::task::spawn_blocking(move || crate::archive::zip_dir(&save_path, level))
            .await
            .map_err(|e| e.to_string())??
    };

    let storage = make_provider(game.provider, cfg)?;
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let name = format!("{}_{}.zip", sanitize(&game.name), stamp);
    emit_progress(app, "game", &game.id, "uploading", Some(format!("Nahrávam {name}…")));
    let upload = storage
        .upload_file(&game_remote_dir(&game.name), &name, &zip_path)
        .await;
    let _ = std::fs::remove_file(&zip_path);
    upload.map(|_| ())
}

/// Zoznam verzií save-ov hry na cloude (najnovšie prvé).
pub async fn list_game_versions(app: &AppHandle, id: &str) -> Result<Vec<RemoteFile>, String> {
    let cfg = snapshot(app).await;
    let game = cfg
        .games
        .iter()
        .find(|g| g.id == id)
        .cloned()
        .ok_or_else(|| "Hra sa nenašla.".to_string())?;
    let storage = make_provider(game.provider, &cfg)?;
    let mut files = storage.list_files(&game_remote_dir(&game.name)).await?;
    files.retain(|f| f.name.to_lowercase().ends_with(".zip"));
    files.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(files)
}

/// Stiahne zvolenú verziu a rozbalí ju do save priečinka.
pub async fn restore_game_version(app: &AppHandle, id: &str, version_id: &str) -> Result<(), String> {
    let cfg = snapshot(app).await;
    let game = cfg
        .games
        .iter()
        .find(|g| g.id == id)
        .cloned()
        .ok_or_else(|| "Hra sa nenašla.".to_string())?;
    let storage = make_provider(game.provider, &cfg)?;

    let tmp = std::env::temp_dir().join(format!("backup-sync-restore-{}.zip", uuid::Uuid::new_v4()));
    emit_progress(app, "game", id, "uploading", Some("Sťahujem verziu…".into()));
    storage.download_file(version_id, &tmp).await?;

    // Potlač watcher, nech obnova nespustí nové "zmenené" (a auto-upload).
    {
        let state = app.state::<AppState>();
        let mut suppress = state.suppress.lock().unwrap();
        suppress.insert(game.save_path.clone(), Instant::now() + Duration::from_secs(15));
    }

    let dest = PathBuf::from(&game.save_path);
    let result = {
        let tmp = tmp.clone();
        tokio::task::spawn_blocking(move || crate::archive::unzip_to(&tmp, &dest))
            .await
            .map_err(|e| e.to_string())?
    };
    let _ = std::fs::remove_file(&tmp);
    result?;

    with_config(app, |c| {
        if let Some(g) = c.games.iter_mut().find(|g| g.id == id) {
            g.sync_state = SyncState::Synced;
            g.last_error = None;
        }
    })
    .await;
    emit_progress(app, "game", id, "done", Some("Verzia obnovená".into()));
    Ok(())
}
