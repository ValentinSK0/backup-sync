use crate::config::{now_iso, AppConfig, Provider, SyncState, WatchedFolder};
use crate::storage::{make_provider, RemoteFile, RemoteFolder, RemoteTreeFile, StorageProvider};
use crate::AppState;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudFolderBackup {
    pub cloud_id: String,
    pub name: String,
    pub modified_at: Option<String>,
    pub file_count: usize,
    pub size_bytes: u64,
    pub already_connected: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudGameBackup {
    pub cloud_id: String,
    pub name: String, // čitateľný názov hry (z názvu cloud priečinka)
    pub latest_at: Option<String>,
    pub version_count: usize,
    pub size_bytes: u64,
    pub already_connected: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCloudStatus {
    pub id: String,
    pub update_available: bool,
    pub latest_at: Option<String>,
}

pub(crate) struct SyncGuard {
    active: Arc<StdMutex<std::collections::HashSet<String>>>,
    key: String,
}

impl Drop for SyncGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(&self.key);
        }
    }
}

fn sync_key(kind: &str, id: &str) -> String {
    format!("{kind}:{id}")
}

const APP_UPDATE_KEY: &str = "app:update";

pub(crate) fn begin_app_update_install(
    active: Arc<StdMutex<std::collections::HashSet<String>>>,
) -> Result<SyncGuard, String> {
    {
        let mut set = active
            .lock()
            .map_err(|_| "Interný zámok synchronizácie je poškodený.".to_string())?;
        if !set.is_empty() {
            return Err(
                "Počkajte na dokončenie zálohovania alebo obnovy a skúste inštaláciu znova.".into(),
            );
        }
        set.insert(APP_UPDATE_KEY.to_string());
    }
    Ok(SyncGuard {
        active,
        key: APP_UPDATE_KEY.to_string(),
    })
}

pub(crate) fn begin_sync(app: &AppHandle, kind: &str, id: &str) -> Result<SyncGuard, String> {
    let active = app.state::<AppState>().active_syncs.clone();
    let key = sync_key(kind, id);
    {
        let mut set = active
            .lock()
            .map_err(|_| "Interný zámok synchronizácie je poškodený.".to_string())?;
        if set.contains(APP_UPDATE_KEY) {
            return Err("Prebieha inštalácia aktualizácie aplikácie.".into());
        }
        if !set.insert(key.clone()) {
            return Err("Táto synchronizácia už prebieha.".into());
        }
    }
    Ok(SyncGuard { active, key })
}

pub(crate) fn is_sync_active(app: &AppHandle, kind: &str, id: &str) -> bool {
    app.state::<AppState>()
        .active_syncs
        .lock()
        .map_or(true, |active| active.contains(&sync_key(kind, id)))
}

pub(crate) fn has_active_syncs(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .active_syncs
        .lock()
        .map_or(true, |active| !active.is_empty())
}

/// Vráti true iba pri prvej zmene zaznamenanej počas aktuálneho syncu.
pub(crate) fn record_pending_change(app: &AppHandle, kind: &str, id: &str) -> bool {
    app.state::<AppState>()
        .pending_changes
        .lock()
        .is_ok_and(|mut pending| pending.insert(sync_key(kind, id)))
}

fn take_pending_change(app: &AppHandle, kind: &str, id: &str) -> bool {
    app.state::<AppState>()
        .pending_changes
        .lock()
        .map_or(true, |mut pending| pending.remove(&sync_key(kind, id)))
}

pub fn emit_progress(
    app: &AppHandle,
    target_type: &'static str,
    id: &str,
    status: &'static str,
    message: Option<String>,
) {
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
pub async fn with_config<F: FnOnce(&mut AppConfig)>(
    app: &AppHandle,
    f: F,
) -> Result<AppConfig, String> {
    let state = app.state::<AppState>();
    let snapshot = {
        let mut cfg = state.config.lock().await;
        let mut next = cfg.clone();
        f(&mut next);
        next.save(&state.config_path())?;
        *cfg = next.clone();
        next
    };
    let _ = app.emit("config-changed", &snapshot);
    Ok(snapshot)
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
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "bez-mena".into()
    } else {
        trimmed
    }
}

fn contains_local_save_files(root: &Path) -> Result<bool, String> {
    if !root.exists() {
        return Ok(false);
    }
    for entry in walkdir::WalkDir::new(root).min_depth(1) {
        let entry = entry.map_err(|e| format!("Čítanie pôvodného save priečinka zlyhalo: {e}"))?;
        if entry.file_type().is_file() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn backup_local_save_before_cloud(
    destination: &Path,
    configured_backup_path: &str,
    game_name: &str,
    level: crate::config::CompressionLevel,
) -> Result<Option<PathBuf>, String> {
    if !contains_local_save_files(destination)? {
        return Ok(None);
    }
    let backup_dir = if configured_backup_path.trim().is_empty() {
        let parent = destination.parent().ok_or_else(|| {
            format!(
                "Pôvodný save sa nedá zazálohovať: priečinok nemá nadradenú cestu: {}",
                destination.display()
            )
        })?;
        parent.join("Backup Sync local saves")
    } else {
        PathBuf::from(configured_backup_path)
    };
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%3f");
    let archive = backup_dir.join(format!(
        "{}_local-before-cloud_{}-{}.zip",
        sanitize(game_name),
        stamp,
        uuid::Uuid::new_v4()
    ));
    let before = crate::game_save::scan(destination)?;
    let backup_result = (|| {
        crate::archive::zip_dir_to(destination, &archive, level)
            .map_err(|e| format!("Pôvodný save sa nepodarilo zazálohovať: {e}"))?;
        if crate::game_save::scan_archive(&archive)? != before
            || crate::game_save::scan(destination)? != before
        {
            return Err(
                "Hra počas ochrannej zálohy zmenila save. Obnovu zopakuj po dokončení zápisu hry."
                    .into(),
            );
        }
        Ok::<(), String>(())
    })();
    if let Err(error) = backup_result {
        let _ = std::fs::remove_file(&archive);
        return Err(error);
    }
    Ok(Some(archive))
}

pub fn folder_cloud_name(path: &str, id: &str) -> String {
    let base = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "priecinok".into());
    format!("{}-{}", sanitize(&base), &id[..8.min(id.len())])
}

pub(crate) fn comparable_path(path: &Path) -> PathBuf {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    #[cfg(windows)]
    {
        let mut value = resolved.to_string_lossy().replace('/', "\\");
        if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
            value = format!(r"\\{rest}");
        } else if let Some(rest) = value.strip_prefix(r"\\?\") {
            value = rest.to_string();
        }
        PathBuf::from(value.to_lowercase())
    }
    #[cfg(not(windows))]
    {
        resolved
    }
}

fn folder_backups_root() -> Vec<String> {
    vec!["BackupSync".into(), "folders".into()]
}

fn folder_remote_dir(cloud_name: &str) -> Vec<String> {
    let mut dir = folder_backups_root();
    dir.push(cloud_name.to_string());
    dir
}

fn games_backups_root() -> Vec<String> {
    vec!["BackupSync".into(), "games".into()]
}

pub fn game_cloud_name(name: &str) -> String {
    sanitize(name)
}

pub fn default_game_backup_path(save_path: &str) -> String {
    let save = Path::new(save_path);
    save.parent()
        .unwrap_or(save)
        .join("Backup Sync local saves")
        .to_string_lossy()
        .to_string()
}

fn game_remote_dir(cloud_name: &str) -> Vec<String> {
    let mut dir = games_backups_root();
    dir.push(cloud_name.to_string());
    dir
}

fn game_remote_dir_for(game: &crate::config::Game) -> Vec<String> {
    let cloud_name = if game.cloud_name.trim().is_empty() {
        game_cloud_name(&game.name)
    } else {
        game.cloud_name.clone()
    };
    game_remote_dir(&cloud_name)
}

/// Z názvu cloud priečinka spraví čitateľný názov hry (pomlčky/podčiarkovníky → medzery).
fn prettify_cloud_name(cloud_name: &str) -> String {
    let pretty = cloud_name
        .replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if pretty.is_empty() {
        cloud_name.to_string()
    } else {
        pretty
    }
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
    let bytes = serde_json::to_vec(map).map_err(|e| e.to_string())?;
    crate::config::write_atomic(path, &bytes)
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Prejde priečinok a vráti (rel_cesta, abs_cesta, sha256) pre každý súbor.
fn scan_folder(root: &Path) -> Result<Vec<(String, PathBuf, String)>, String> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.map_err(|e| format!("Čítanie priečinka zlyhalo: {e}"))?;
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
        let hash = hash_file(&abs)?;
        out.push((rel, abs, hash));
    }
    Ok(out)
}

fn is_windows_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=9).contains(&n))
        || stem
            .strip_prefix("LPT")
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=9).contains(&n))
}

fn safe_relative_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let normalized = relative.replace('\\', "/");
    if normalized.is_empty() || normalized.starts_with('/') {
        return Err(format!("Neplatná cesta v cloud zálohe: {relative}"));
    }
    let mut out = root.to_path_buf();
    for part in normalized.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains('\0') {
            return Err(format!("Nebezpečná cesta v cloud zálohe: {relative}"));
        }
        #[cfg(windows)]
        if part.contains(['<', '>', ':', '"', '|', '?', '*'])
            || part.ends_with([' ', '.'])
            || is_windows_reserved_name(part)
        {
            return Err(format!(
                "Cesta z cloudu nie je platná vo Windows: {relative}"
            ));
        }
        out.push(part);
    }
    Ok(out)
}

fn prepare_file_target(root: &Path, target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| "Neplatný cieľový priečinok.".to_string())?;
    let relative_parent = parent
        .strip_prefix(root)
        .map_err(|_| "Cloud súbor smeruje mimo zvoleného cieľového priečinka.".to_string())?;
    let mut current = root.to_path_buf();
    for component in relative_parent.components() {
        current.push(component.as_os_str());
        if current.exists() {
            let metadata = std::fs::symlink_metadata(&current).map_err(|e| e.to_string())?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "Obnova odmietnutá: cesta obsahuje odkaz/junction: {}",
                    current.display()
                ));
            }
            if !metadata.is_dir() {
                return Err(format!("Cieľ nie je priečinok: {}", current.display()));
            }
        } else {
            std::fs::create_dir(&current).map_err(|e| e.to_string())?;
        }
    }
    if target.exists()
        && std::fs::symlink_metadata(target)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
    {
        return Err(format!(
            "Obnova odmietnutá: cieľový súbor je odkaz: {}",
            target.display()
        ));
    }
    Ok(())
}

fn copy_downloaded_files(
    staging: &Path,
    destination: &Path,
    files: &[RemoteTreeFile],
) -> Result<HashMap<String, String>, String> {
    std::fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    let mut hashes = HashMap::new();
    for file in files {
        let source = safe_relative_path(staging, &file.relative_path)?;
        let target = safe_relative_path(destination, &file.relative_path)?;
        prepare_file_target(destination, &target)?;
        std::fs::copy(&source, &target)
            .map_err(|e| format!("Kopírovanie {} zlyhalo: {e}", file.relative_path))?;
        hashes.insert(file.relative_path.clone(), hash_file(&target)?);
    }
    Ok(hashes)
}

fn copy_staged_tree(staging: &Path, destination: &Path) -> Result<usize, String> {
    std::fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    let mut copied = 0usize;
    for entry in walkdir::WalkDir::new(staging) {
        let entry = entry.map_err(|e| format!("Čítanie obnovenej zálohy zlyhalo: {e}"))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(staging)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let target = safe_relative_path(destination, &relative)?;
        prepare_file_target(destination, &target)?;
        std::fs::copy(entry.path(), &target)
            .map_err(|e| format!("Kopírovanie {relative} zlyhalo: {e}"))?;
        copied += 1;
    }
    Ok(copied)
}

fn suppress_watcher(app: &AppHandle, path: &str, duration: Duration) {
    let state = app.state::<AppState>();
    let mut suppress = state.suppress.lock().unwrap();
    suppress.insert(comparable_path(Path::new(path)), Instant::now() + duration);
}

async fn download_folder_snapshot(
    app: &AppHandle,
    storage: &dyn StorageProvider,
    cloud_id: &str,
    destination: &Path,
    local_folder_id: &str,
    suppress_existing_watcher: bool,
) -> Result<usize, String> {
    let files = storage.list_files_recursive(cloud_id).await?;
    let staging = std::env::temp_dir().join(format!(
        "backup-sync-folder-restore-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;

    let mut targets = std::collections::HashSet::new();
    let download_result = async {
        for file in &files {
            let target = safe_relative_path(&staging, &file.relative_path)?;
            let collision_key = target.to_string_lossy().to_lowercase();
            if !targets.insert(collision_key) {
                return Err(format!(
                    "Cloud záloha obsahuje kolidujúcu cestu: {}",
                    file.relative_path
                ));
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            storage.download_file(&file.id, &target).await?;
        }
        Ok::<(), String>(())
    }
    .await;

    if let Err(error) = download_result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }

    let destination_string = destination.to_string_lossy().to_string();
    if suppress_existing_watcher {
        suppress_watcher(app, &destination_string, Duration::from_secs(3600));
    }
    let staging_for_copy = staging.clone();
    let destination_for_copy = destination.to_path_buf();
    let files_for_copy = files.clone();
    let copy_result = tokio::task::spawn_blocking(move || {
        copy_downloaded_files(&staging_for_copy, &destination_for_copy, &files_for_copy)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|result| result);
    if suppress_existing_watcher {
        suppress_watcher(app, &destination_string, Duration::from_secs(15));
    }
    let _ = std::fs::remove_dir_all(&staging);
    let hashes = copy_result?;
    save_hash_state(&hash_state_path(app, local_folder_id), &hashes)?;
    Ok(files.len())
}

fn set_folder_result(
    cfg: &mut AppConfig,
    id: &str,
    action: &str,
    result: &Result<(), String>,
    changed_during_sync: bool,
) {
    if let Some(f) = cfg.watched_folders.iter_mut().find(|f| f.id == id) {
        match result {
            Ok(()) => {
                f.sync_state = if changed_during_sync {
                    SyncState::Changed
                } else {
                    SyncState::Synced
                };
                f.last_sync = Some(now_iso());
                f.last_error = None;
                f.last_error_action = None;
            }
            Err(e) => {
                f.sync_state = SyncState::Error;
                f.last_error = Some(e.clone());
                f.last_error_action = Some(action.to_string());
            }
        }
    }
}

async fn resolve_folder_for_upload(
    app: &AppHandle,
    storage: &dyn StorageProvider,
    folder: &WatchedFolder,
) -> Result<String, String> {
    if let Some(cloud_id) = &folder.cloud_id {
        return Ok(cloud_id.clone());
    }
    let cloud_name = if folder.cloud_name.is_empty() {
        folder_cloud_name(&folder.path, &folder.id)
    } else {
        folder.cloud_name.clone()
    };
    let remote = storage
        .ensure_folder(&folder_remote_dir(&cloud_name))
        .await?;
    let remote_id = remote.id.clone();
    with_config(app, |cfg| {
        if let Some(item) = cfg
            .watched_folders
            .iter_mut()
            .find(|item| item.id == folder.id)
        {
            item.cloud_id = Some(remote.id);
            item.cloud_name = cloud_name;
        }
    })
    .await?;
    Ok(remote_id)
}

/// Inkrementálna záloha priečinka: nahrá len súbory, ktorých hash sa zmenil.
pub async fn backup_folder(app: &AppHandle, id: &str) -> Result<(), String> {
    let _sync = begin_sync(app, "folder", id)?;
    let cfg = with_config(app, |c| {
        if let Some(f) = c.watched_folders.iter_mut().find(|f| f.id == id) {
            f.sync_state = SyncState::Syncing;
            f.last_error = None;
            f.last_error_action = None;
        }
    })
    .await?;
    let folder = cfg
        .watched_folders
        .iter()
        .find(|f| f.id == id)
        .cloned()
        .ok_or_else(|| "Priečinok sa nenašiel.".to_string())?;

    emit_progress(
        app,
        "folder",
        id,
        "uploading",
        Some("Kontrolujem zmeny…".into()),
    );
    let result = backup_folder_inner(app, &folder, &cfg).await;

    with_config(app, |c| {
        let changed = take_pending_change(app, "folder", id);
        set_folder_result(c, id, "upload", &result, changed);
    })
    .await?;
    match &result {
        Ok(()) => emit_progress(app, "folder", id, "done", Some("Záloha hotová".into())),
        Err(e) => emit_progress(app, "folder", id, "error", Some(e.clone())),
    }
    result
}

async fn backup_folder_inner(
    app: &AppHandle,
    folder: &WatchedFolder,
    cfg: &AppConfig,
) -> Result<(), String> {
    let root = PathBuf::from(&folder.path);
    let files = {
        let root = root.clone();
        tokio::task::spawn_blocking(move || scan_folder(&root))
            .await
            .map_err(|e| e.to_string())??
    };

    let storage = make_provider(folder.provider, cfg)?;
    let cloud_id = resolve_folder_for_upload(app, storage.as_ref(), folder).await?;
    let state_path = hash_state_path(app, &folder.id);
    let mut hashes = load_hash_state(&state_path);
    let changed: Vec<_> = files
        .iter()
        .filter(|(rel, _, hash)| hashes.get(rel) != Some(hash))
        .cloned()
        .collect();

    if changed.is_empty() {
        return Ok(());
    }

    let total = changed.len();
    for (i, (rel, abs, hash)) in changed.into_iter().enumerate() {
        let mut dir = Vec::new();
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
            &folder.id,
            "uploading",
            Some(format!("({}/{}) {}", i + 1, total, rel)),
        );
        storage
            .upload_file_to_folder(&cloud_id, &dir, &name, &abs)
            .await?;
        hashes.insert(rel, hash);
        // priebežne ukladaj stav, nech prerušenie nezahodí hotové uploady
        let _ = save_hash_state(&state_path, &hashes);
    }
    save_hash_state(&state_path, &hashes)?;
    Ok(())
}

pub async fn list_folder_backups(
    app: &AppHandle,
    provider: Provider,
) -> Result<Vec<CloudFolderBackup>, String> {
    let cfg = snapshot(app).await;
    let storage = make_provider(provider, &cfg)?;
    let folders = storage.list_folders(&folder_backups_root()).await?;
    let mut backups = Vec::with_capacity(folders.len());
    for folder in folders {
        let files = storage.list_files_recursive(&folder.id).await?;
        let modified_at = files
            .iter()
            .filter_map(|file| file.modified_at.clone())
            .max();
        let size_bytes = files
            .iter()
            .fold(0u64, |total, file| total.saturating_add(file.size_bytes));
        let already_connected = cfg.watched_folders.iter().any(|item| {
            item.provider == provider
                && (item.cloud_id.as_deref() == Some(folder.id.as_str())
                    || (item.cloud_id.is_none() && item.cloud_name == folder.name))
        });
        backups.push(CloudFolderBackup {
            cloud_id: folder.id,
            name: folder.name,
            modified_at,
            file_count: files.len(),
            size_bytes,
            already_connected,
        });
    }
    backups.sort_by(|a, b| {
        b.modified_at
            .cmp(&a.modified_at)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(backups)
}

fn canonical_path(path: &str) -> Result<PathBuf, String> {
    std::fs::canonicalize(path).map_err(|e| format!("Neplatný lokálny priečinok: {e}"))
}

async fn find_remote_folder(
    storage: &dyn StorageProvider,
    cloud_id: &str,
) -> Result<RemoteFolder, String> {
    storage
        .list_folders(&folder_backups_root())
        .await?
        .into_iter()
        .find(|folder| folder.id == cloud_id)
        .ok_or_else(|| "Vybraná cloud záloha už neexistuje.".to_string())
}

pub async fn connect_folder_backup(
    app: &AppHandle,
    provider: Provider,
    cloud_id: &str,
    destination: &str,
) -> Result<AppConfig, String> {
    if !Path::new(destination).is_dir() {
        return Err(format!("Cieľový priečinok neexistuje: {destination}"));
    }
    let destination_canonical = canonical_path(destination)?;
    let _sync = begin_sync(
        app,
        "connection",
        &comparable_path(&destination_canonical).to_string_lossy(),
    )?;
    let destination_path = PathBuf::from(destination);
    let cfg = snapshot(app).await;
    for folder in &cfg.watched_folders {
        if canonical_path(&folder.path).ok().as_ref() == Some(&destination_canonical) {
            return Err("Tento lokálny priečinok už aplikácia sleduje.".into());
        }
    }

    let storage = make_provider(provider, &cfg)?;
    let remote = find_remote_folder(storage.as_ref(), cloud_id).await?;
    if cfg.watched_folders.iter().any(|folder| {
        folder.provider == provider
            && (folder.cloud_id.as_deref() == Some(remote.id.as_str())
                || (folder.cloud_id.is_none() && folder.cloud_name == remote.name))
    }) {
        return Err("Táto cloud záloha je už na tomto PC pripojená.".into());
    }

    let local_id = uuid::Uuid::new_v4().to_string();
    download_folder_snapshot(
        app,
        storage.as_ref(),
        &remote.id,
        &destination_path,
        &local_id,
        false,
    )
    .await?;

    let config = with_config(app, |config| {
        config.watched_folders.push(WatchedFolder {
            id: local_id,
            path: destination.to_string(),
            provider,
            cloud_id: Some(remote.id),
            cloud_name: remote.name,
            auto_backup: Some(false),
            sync_state: SyncState::Synced,
            last_sync: Some(now_iso()),
            last_error: None,
            last_error_action: None,
        });
    })
    .await?;
    Ok(config)
}

async fn resolve_folder_for_download(
    app: &AppHandle,
    storage: &dyn StorageProvider,
    folder: &WatchedFolder,
) -> Result<String, String> {
    if let Some(cloud_id) = &folder.cloud_id {
        return Ok(cloud_id.clone());
    }
    let remote = storage
        .list_folders(&folder_backups_root())
        .await?
        .into_iter()
        .find(|remote| remote.name == folder.cloud_name)
        .ok_or_else(|| "Cloud záloha sa nenašla. Najprv priečinok zálohuj.".to_string())?;
    let cloud_id = remote.id.clone();
    with_config(app, |config| {
        if let Some(item) = config
            .watched_folders
            .iter_mut()
            .find(|item| item.id == folder.id)
        {
            item.cloud_id = Some(remote.id);
        }
    })
    .await?;
    Ok(cloud_id)
}

pub async fn restore_folder_from_cloud(app: &AppHandle, id: &str) -> Result<(), String> {
    let _sync = begin_sync(app, "folder", id)?;
    let cfg = with_config(app, |config| {
        if let Some(folder) = config
            .watched_folders
            .iter_mut()
            .find(|folder| folder.id == id)
        {
            folder.sync_state = SyncState::Syncing;
            folder.last_error = None;
            folder.last_error_action = None;
        }
    })
    .await?;
    let folder = cfg
        .watched_folders
        .iter()
        .find(|folder| folder.id == id)
        .cloned()
        .ok_or_else(|| "Priečinok sa nenašiel.".to_string())?;

    emit_progress(
        app,
        "folder",
        id,
        "uploading",
        Some("Sťahujem zálohu z cloudu…".into()),
    );
    let result = async {
        let storage = make_provider(folder.provider, &cfg)?;
        let cloud_id = resolve_folder_for_download(app, storage.as_ref(), &folder).await?;
        download_folder_snapshot(
            app,
            storage.as_ref(),
            &cloud_id,
            Path::new(&folder.path),
            &folder.id,
            true,
        )
        .await?;
        Ok::<(), String>(())
    }
    .await;

    with_config(app, |config| {
        let changed = take_pending_change(app, "folder", id);
        set_folder_result(config, id, "download", &result, changed)
    })
    .await?;
    match &result {
        Ok(()) => emit_progress(
            app,
            "folder",
            id,
            "done",
            Some("Cloud záloha stiahnutá".into()),
        ),
        Err(error) => emit_progress(app, "folder", id, "error", Some(error.clone())),
    }
    result
}

fn set_game_result(
    cfg: &mut AppConfig,
    id: &str,
    result: &Result<String, String>,
    changed_during_sync: bool,
) {
    if let Some(g) = cfg.games.iter_mut().find(|g| g.id == id) {
        match result {
            Ok(version_id) => {
                g.sync_state = if changed_during_sync {
                    SyncState::Changed
                } else {
                    SyncState::Synced
                };
                g.last_upload = Some(now_iso());
                g.last_cloud_version_id = Some(version_id.clone());
                g.last_error = None;
            }
            Err(e) => {
                g.sync_state = SyncState::Error;
                g.last_error = Some(e.clone());
            }
        }
    }
}

fn set_game_restore_result(
    cfg: &mut AppConfig,
    id: &str,
    result: &Result<(), String>,
    changed_during_sync: bool,
    version_id: &str,
) {
    if let Some(game) = cfg.games.iter_mut().find(|game| game.id == id) {
        match result {
            Ok(()) => {
                game.sync_state = if changed_during_sync {
                    SyncState::Changed
                } else {
                    SyncState::Synced
                };
                game.last_error = None;
                game.last_cloud_version_id = Some(version_id.to_string());
            }
            Err(error) => {
                game.sync_state = SyncState::Error;
                game.last_error = Some(error.clone());
            }
        }
    }
}

/// Keep the verified local ZIP even when the provider is unavailable.
pub async fn upload_game(app: &AppHandle, id: &str) -> Result<(), String> {
    sync_game_save(app, id, GameSaveAction::Upload).await
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GameSaveAction {
    Check,
    Local,
    Upload,
}

pub async fn backup_game_local(app: &AppHandle, id: &str) -> Result<(), String> {
    sync_game_save(app, id, GameSaveAction::Local).await
}

pub async fn check_game_save(app: &AppHandle, id: &str) -> Result<(), String> {
    sync_game_save(app, id, GameSaveAction::Check).await
}

async fn scan_game_save(path: &str) -> Result<crate::game_save::Snapshot, String> {
    let path = PathBuf::from(path);
    tokio::task::spawn_blocking(move || crate::game_save::scan(&path))
        .await
        .map_err(|e| e.to_string())?
}

async fn stable_game_backup(
    game: &crate::config::Game,
    cfg: &AppConfig,
) -> Result<(PathBuf, String), String> {
    for _ in 0..3 {
        let before = scan_game_save(&game.save_path).await?;
        if before.0.is_empty() {
            return Err("Save priečinok neobsahuje žiadne súbory. Predchádzajúce zálohy zostávajú zachované.".into());
        }
        tokio::time::sleep(Duration::from_secs(cfg.settings.debounce_seconds.max(2))).await;
        if scan_game_save(&game.save_path).await? != before {
            continue;
        }
        let root = PathBuf::from(&game.save_path);
        let directory =
            PathBuf::from(&game.backup_path).join(format!("{}-{}", sanitize(&game.name), game.id));
        let fingerprint = before.fingerprint();
        let level = cfg.settings.compression_level;
        let result = tokio::task::spawn_blocking(move || {
            crate::game_save::create_backup(&root, &directory, &before, level)
        })
        .await
        .map_err(|e| e.to_string())?;
        match result {
            Ok(path) => return Ok((path, fingerprint)),
            Err(error) => {
                if scan_game_save(&game.save_path).await?.fingerprint() == fingerprint {
                    return Err(error);
                }
            }
        }
    }
    Err("Hra stále zapisuje save. Kontrola a záloha sa zopakujú automaticky.".into())
}

async fn reusable_game_backup(game: &crate::config::Game, hash: &str) -> Option<PathBuf> {
    if game.local.backup_hash.as_deref() != Some(hash) {
        return None;
    }
    let path = PathBuf::from(game.local.backup_file.as_ref()?);
    if !comparable_path(&path).starts_with(comparable_path(Path::new(&game.backup_path))) {
        return None;
    }
    let archive = path.clone();
    let fingerprint = tokio::task::spawn_blocking(move || {
        crate::game_save::scan_archive(&archive).map(|s| s.fingerprint())
    })
    .await
    .ok()?
    .ok()?;
    (fingerprint == hash).then_some(path)
}

fn record_game_observation(game: &mut crate::config::Game, hash: &str) {
    if game.local.observed_hash.as_deref() != Some(hash) {
        game.last_change = Some(now_iso());
    }
    game.local.observed_hash = Some(hash.to_string());
    game.local.last_check = Some(now_iso());
}

fn should_upload_game(
    game: &crate::config::Game,
    cfg: &AppConfig,
    action: GameSaveAction,
    hash: &str,
) -> bool {
    action == GameSaveAction::Upload
        || (action == GameSaveAction::Check
        && game.auto_upload.unwrap_or(cfg.settings.auto_upload_saves)
        // Legacy versions have no content baseline: require one explicit upload.
        && (game.last_cloud_version_id.is_none() || game.local.uploaded_hash.is_some())
        && game.local.uploaded_hash.as_deref() != Some(hash))
}

async fn sync_game_save(app: &AppHandle, id: &str, action: GameSaveAction) -> Result<(), String> {
    let _guard = match begin_sync(app, "game", id) {
        Ok(guard) => guard,
        Err(_) if action == GameSaveAction::Check => {
            record_pending_change(app, "game", id);
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    let cfg = snapshot(app).await;
    let game = cfg
        .games
        .iter()
        .find(|g| g.id == id)
        .cloned()
        .ok_or("Hra sa nenašla.")?;
    let result = async {
        let initial_hash = scan_game_save(&game.save_path).await?.fingerprint();
        let backup = reusable_game_backup(&game, &initial_hash).await;
        let upload = should_upload_game(&game, &cfg, action, &initial_hash);
        with_config(app, |c| {
            if let Some(g) = c.games.iter_mut().find(|g| g.id == id) {
                record_game_observation(g, &initial_hash);
                g.sync_state = if backup.is_none() || upload {
                    SyncState::Syncing
                } else if g.local.uploaded_hash.as_deref() == Some(&initial_hash) {
                    SyncState::Synced
                } else {
                    SyncState::Changed
                };
                if backup.is_none() || upload || action == GameSaveAction::Local {
                    g.last_error = None;
                }
            }
        })
        .await?;
        if backup.is_some() && !upload {
            return Ok(());
        }
        emit_progress(
            app,
            "game",
            id,
            "uploading",
            Some("Overujem a zálohujem save…".into()),
        );
        let (zip, hash) = match backup {
            Some(path) => (path, initial_hash),
            None => {
                let (path, hash) = stable_game_backup(&game, &cfg).await?;
                with_config(app, |c| {
                    if let Some(g) = c.games.iter_mut().find(|g| g.id == id) {
                        g.local.last_backup = Some(now_iso());
                        g.local.backup_file = Some(path.to_string_lossy().into_owned());
                        g.local.backup_hash = Some(hash.clone());
                    }
                })
                .await?;
                (path, hash)
            }
        };
        let current_cfg = snapshot(app).await;
        let current_game = current_cfg
            .games
            .iter()
            .find(|g| g.id == id)
            .ok_or("Hra sa nenašla.")?;
        let upload = upload && should_upload_game(current_game, &current_cfg, action, &hash);
        let uploaded = if upload {
            emit_progress(
                app,
                "game",
                id,
                "uploading",
                Some("Lokálna záloha je uložená. Nahrávam na cloud…".into()),
            );
            let storage = make_provider(game.provider, &cfg)?;
            let name = format!(
                "{}_{}_{}.zip",
                sanitize(&game.name),
                chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%3f"),
                uuid::Uuid::new_v4()
            );
            Some(
                storage
                    .upload_file(&game_remote_dir_for(&game), &name, &zip)
                    .await?,
            )
        } else {
            None
        };
        // Persist cloud success before the follow-up scan, which can itself fail.
        with_config(app, |c| {
            if let Some(version) = &uploaded {
                set_game_result(c, id, &Ok(version.clone()), true);
            }
            if let Some(g) = c.games.iter_mut().find(|g| g.id == id) {
                if uploaded.is_some() {
                    g.local.uploaded_hash = Some(hash);
                }
            }
        })
        .await?;
        let current = scan_game_save(&game.save_path).await?.fingerprint();
        with_config(app, |c| {
            take_pending_change(app, "game", id);
            if let Some(g) = c.games.iter_mut().find(|g| g.id == id) {
                record_game_observation(g, &current);
                g.sync_state = if g.local.uploaded_hash.as_deref() == Some(&current) {
                    SyncState::Synced
                } else {
                    SyncState::Changed
                };
                g.last_error = None;
            }
        })
        .await?;
        emit_progress(
            app,
            "game",
            id,
            "done",
            Some(
                if upload {
                    "Save zálohovaný lokálne aj na cloud"
                } else {
                    "Save zálohovaný lokálne"
                }
                .into(),
            ),
        );
        Ok::<(), String>(())
    }
    .await;
    if let Err(error) = &result {
        with_config(app, |c| {
            if let Some(g) = c.games.iter_mut().find(|g| g.id == id) {
                g.sync_state = SyncState::Error;
                g.last_error = Some(error.clone());
            }
        })
        .await?;
        if game.last_error.as_ref() != Some(error) || action != GameSaveAction::Check {
            emit_progress(app, "game", id, "error", Some(error.clone()));
        }
    }
    result
}

// Older providers may omit creation dates. Our ZIP names contain local wall time.
fn game_version_date(file: &RemoteFile) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    use chrono::TimeZone;
    if let Some(date) = file
        .created_at
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
    {
        return Some(date);
    }
    let stem = file.name.get(..file.name.len().checked_sub(4)?)?;
    let stem = match stem.rsplit_once('_') {
        Some((prefix, suffix)) if uuid::Uuid::parse_str(suffix).is_ok() => prefix,
        _ => stem,
    };
    for (length, format) in [(23, "%Y-%m-%d_%H-%M-%S-%3f"), (19, "%Y-%m-%d_%H-%M-%S")] {
        let Some(stamp) = stem
            .len()
            .checked_sub(length)
            .and_then(|start| stem.get(start..))
        else {
            continue;
        };
        if let Ok(date) = chrono::NaiveDateTime::parse_from_str(stamp, format) {
            return chrono::Local
                .from_local_datetime(&date)
                .earliest()
                .map(|date| date.fixed_offset());
        }
    }
    None
}

fn sorted_game_versions(files: Vec<RemoteFile>) -> Vec<RemoteFile> {
    let mut files: Vec<_> = files
        .into_iter()
        .filter(|file| file.name.to_lowercase().ends_with(".zip"))
        .map(|mut file| {
            file.created_at = game_version_date(&file).map(|date| date.to_rfc3339());
            file
        })
        .collect();
    files.sort_by_cached_key(|file| (std::cmp::Reverse(game_version_date(file)), file.id.clone()));
    files
}

fn sorted_tree_game_versions(files: Vec<RemoteTreeFile>) -> Vec<RemoteFile> {
    sorted_game_versions(
        files
            .into_iter()
            .map(|file| RemoteFile {
                id: file.id,
                name: file.relative_path,
                created_at: file.modified_at,
                size_bytes: file.size_bytes,
            })
            .collect(),
    )
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
    Ok(sorted_game_versions(
        storage.list_files(&game_remote_dir_for(&game)).await?,
    ))
}

fn remote_game_version_is_newer(
    game: &crate::config::Game,
    latest: &RemoteFile,
    baseline: Option<&RemoteFile>,
) -> bool {
    if game.last_cloud_version_id.as_deref() == Some(latest.id.as_str()) {
        return false;
    }
    let local = baseline.and_then(game_version_date).or_else(|| {
        game.last_upload
            .as_deref()
            .and_then(|date| chrono::DateTime::parse_from_rfc3339(date).ok())
    });
    match (game_version_date(latest), local) {
        (Some(remote), Some(local)) => remote > local + chrono::Duration::seconds(5),
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// Skontroluje, či cloud obsahuje novšiu verziu než posledná známa lokálna verzia hry.
/// Nedostupný cloud neskryje kartu ani jej aktuálny lokálny stav.
pub async fn list_game_cloud_statuses(app: &AppHandle) -> Result<Vec<GameCloudStatus>, String> {
    let cfg = snapshot(app).await;
    let mut statuses = Vec::with_capacity(cfg.games.len());
    for game in &cfg.games {
        let versions = match make_provider(game.provider, &cfg) {
            Ok(storage) => match storage.list_files(&game_remote_dir_for(game)).await {
                Ok(files) => sorted_game_versions(files),
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        };
        let latest = versions.first();
        let baseline = versions
            .iter()
            .find(|version| game.last_cloud_version_id.as_deref() == Some(version.id.as_str()));
        let update_available =
            latest.is_some_and(|latest| remote_game_version_is_newer(game, latest, baseline));
        statuses.push(GameCloudStatus {
            id: game.id.clone(),
            update_available,
            latest_at: latest.and_then(|file| file.created_at.clone()),
        });
    }
    Ok(statuses)
}

/// Refresh the cloud listing at click time; reuse the protected restore path.
pub async fn restore_latest_game_version(app: &AppHandle, id: &str) -> Result<(), String> {
    let versions = list_game_versions(app, id).await?;
    let latest = versions
        .first()
        .ok_or_else(|| "Na cloude nie je žiadna záloha tejto hry.".to_string())?;
    if latest.created_at.is_none() {
        return Err("Dátum najnovšej zálohy sa nedá určiť. Vyber ju cez Verzie.".into());
    }
    let cfg = snapshot(app).await;
    let game = cfg
        .games
        .iter()
        .find(|game| game.id == id)
        .ok_or_else(|| "Hra sa nenašla.".to_string())?;
    let baseline = versions
        .iter()
        .find(|version| game.last_cloud_version_id.as_deref() == Some(version.id.as_str()));
    if !remote_game_version_is_newer(game, latest, baseline) {
        return Err(
            "Cloud už neobsahuje novšiu verziu. Stav hry sa aktualizuje pri ďalšej kontrole."
                .into(),
        );
    }
    restore_game_version(app, id, &latest.id).await
}

/// Stiahne zvolenú verziu a rozbalí ju do save priečinka.
pub async fn restore_game_version(
    app: &AppHandle,
    id: &str,
    version_id: &str,
) -> Result<(), String> {
    let _sync = begin_sync(app, "game", id)?;
    let cfg = with_config(app, |config| {
        if let Some(game) = config.games.iter_mut().find(|game| game.id == id) {
            game.sync_state = SyncState::Syncing;
            game.last_error = None;
        }
    })
    .await?;
    let game = cfg
        .games
        .iter()
        .find(|g| g.id == id)
        .cloned()
        .ok_or_else(|| "Hra sa nenašla.".to_string())?;
    let tmp =
        std::env::temp_dir().join(format!("backup-sync-restore-{}.zip", uuid::Uuid::new_v4()));
    let staging =
        std::env::temp_dir().join(format!("backup-sync-game-restore-{}", uuid::Uuid::new_v4()));
    let backup_destination = PathBuf::from(&game.save_path);
    let backup_path = game.backup_path.clone();
    let backup_game_name = game.name.clone();
    let backup_level = cfg.settings.compression_level;
    emit_progress(
        app,
        "game",
        id,
        "uploading",
        Some("Sťahujem verziu…".into()),
    );
    let result = async {
        tokio::task::spawn_blocking(move || {
            backup_local_save_before_cloud(
                &backup_destination,
                &backup_path,
                &backup_game_name,
                backup_level,
            )
        })
        .await
        .map_err(|e| e.to_string())??;
        let storage = make_provider(game.provider, &cfg)?;
        let selected = storage
            .list_files(&game_remote_dir_for(&game))
            .await?
            .into_iter()
            .any(|file| file.id == version_id && file.name.to_lowercase().ends_with(".zip"));
        if !selected {
            return Err("Vybraná verzia nepatrí k tejto hre alebo už neexistuje.".into());
        }
        storage.download_file(version_id, &tmp).await?;
        std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
        {
            let archive = tmp.clone();
            let staging = staging.clone();
            tokio::task::spawn_blocking(move || crate::archive::unzip_to(&archive, &staging))
                .await
                .map_err(|e| e.to_string())??;
        }
        let cloud_hash = scan_game_save(&staging.to_string_lossy())
            .await?
            .fingerprint();
        suppress_watcher(app, &game.save_path, Duration::from_secs(30));
        let destination = PathBuf::from(&game.save_path);
        let staging_for_copy = staging.clone();
        tokio::task::spawn_blocking(move || copy_staged_tree(&staging_for_copy, &destination))
            .await
            .map_err(|e| e.to_string())??;
        suppress_watcher(app, &game.save_path, Duration::from_secs(15));
        let local_hash = scan_game_save(&game.save_path).await?.fingerprint();
        Ok::<_, String>((cloud_hash, local_hash))
    }
    .await;
    let _ = std::fs::remove_file(&tmp);
    let _ = std::fs::remove_dir_all(&staging);
    with_config(app, |config| {
        let changed = take_pending_change(app, "game", id);
        let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
        set_game_restore_result(config, id, &status, changed, version_id);
        if let (Ok((cloud_hash, local_hash)), Some(game)) =
            (&result, config.games.iter_mut().find(|g| g.id == id))
        {
            game.local.uploaded_hash = Some(cloud_hash.clone());
            record_game_observation(game, local_hash);
            if cloud_hash != local_hash {
                game.sync_state = SyncState::Changed;
            }
        }
    })
    .await?;
    match &result {
        Ok(_) => emit_progress(app, "game", id, "done", Some("Verzia obnovená".into())),
        Err(error) => emit_progress(app, "game", id, "error", Some(error.clone())),
    }
    result.map(|_| ())
}

/// Zoznam všetkých herných záloh na cloude (čitateľne), najnovšie prvé.
pub async fn list_game_backups(
    app: &AppHandle,
    provider: Provider,
) -> Result<Vec<CloudGameBackup>, String> {
    let cfg = snapshot(app).await;
    let storage = make_provider(provider, &cfg)?;
    let folders = storage.list_folders(&games_backups_root()).await?;
    let mut backups = Vec::with_capacity(folders.len());
    for folder in folders {
        let versions = sorted_tree_game_versions(storage.list_files_recursive(&folder.id).await?);
        if versions.is_empty() {
            continue;
        }
        let latest_at = versions.first().and_then(|file| file.created_at.clone());
        let size_bytes = versions
            .iter()
            .fold(0u64, |total, f| total.saturating_add(f.size_bytes));
        let already_connected = cfg.games.iter().any(|g| {
            g.provider == provider
                && if g.cloud_name.trim().is_empty() {
                    game_cloud_name(&g.name) == folder.name
                } else {
                    g.cloud_name == folder.name
                }
        });
        backups.push(CloudGameBackup {
            cloud_id: folder.id,
            name: prettify_cloud_name(&folder.name),
            latest_at,
            version_count: versions.len(),
            size_bytes,
            already_connected,
        });
    }
    backups.sort_by(|a, b| {
        b.latest_at
            .cmp(&a.latest_at)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(backups)
}

/// Napojí hru z cloudu na tento PC: stiahne najnovšiu verziu do zvoleného priečinka
/// a pridá ju medzi sledované hry.
pub async fn connect_game_backup(
    app: &AppHandle,
    provider: Provider,
    cloud_id: &str,
    destination: &str,
    display_name: &str,
    backup_path: &str,
) -> Result<AppConfig, String> {
    if !Path::new(destination).is_dir() {
        return Err(format!("Cieľový priečinok neexistuje: {destination}"));
    }
    let destination_canonical = canonical_path(destination)?;
    let _sync = begin_sync(
        app,
        "connection",
        &comparable_path(&destination_canonical).to_string_lossy(),
    )?;
    let cfg = snapshot(app).await;
    for game in &cfg.games {
        if canonical_path(&game.save_path).ok().as_ref() == Some(&destination_canonical) {
            return Err("Tento priečinok už používa iná hra.".into());
        }
    }

    let storage = make_provider(provider, &cfg)?;
    let folder = storage
        .list_folders(&games_backups_root())
        .await?
        .into_iter()
        .find(|folder| folder.id == cloud_id)
        .ok_or_else(|| "Vybraná herná záloha už na cloude neexistuje.".to_string())?;

    let latest = sorted_tree_game_versions(storage.list_files_recursive(&folder.id).await?)
        .into_iter()
        .next()
        .ok_or_else(|| "Táto herná záloha nemá žiadnu verziu.".to_string())?;
    if latest.created_at.is_none() {
        return Err(
            "Dátum najnovšej zálohy sa nedá určiť. Pridaj hru ručne a vyber zálohu cez Verzie."
                .into(),
        );
    }

    let name = if display_name.trim().is_empty() {
        prettify_cloud_name(&folder.name)
    } else {
        display_name.trim().to_string()
    };

    let tmp = std::env::temp_dir().join(format!(
        "backup-sync-game-connect-{}.zip",
        uuid::Uuid::new_v4()
    ));
    let staging =
        std::env::temp_dir().join(format!("backup-sync-game-connect-{}", uuid::Uuid::new_v4()));
    let destination_path = PathBuf::from(destination);
    let backup_destination = destination_path.clone();
    let backup_game_name = name.clone();
    let backup_level = cfg.settings.compression_level;
    let local_backup_path = backup_path.to_string();
    let _local_backup = tokio::task::spawn_blocking(move || {
        backup_local_save_before_cloud(
            &backup_destination,
            &local_backup_path,
            &backup_game_name,
            backup_level,
        )
    })
    .await
    .map_err(|e| e.to_string())??;

    let result = async {
        storage.download_file(&latest.id, &tmp).await?;
        std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
        {
            let archive = tmp.clone();
            let staging = staging.clone();
            tokio::task::spawn_blocking(move || crate::archive::unzip_to(&archive, &staging))
                .await
                .map_err(|e| e.to_string())??;
        }
        let cloud_hash = scan_game_save(&staging.to_string_lossy())
            .await?
            .fingerprint();
        suppress_watcher(app, destination, Duration::from_secs(30));
        {
            let staging = staging.clone();
            let destination_path = destination_path.clone();
            tokio::task::spawn_blocking(move || copy_staged_tree(&staging, &destination_path))
                .await
                .map_err(|e| e.to_string())??;
        }
        suppress_watcher(app, destination, Duration::from_secs(15));
        let local_hash = scan_game_save(destination).await?.fingerprint();
        Ok::<_, String>((cloud_hash, local_hash))
    }
    .await;
    let _ = std::fs::remove_file(&tmp);
    let _ = std::fs::remove_dir_all(&staging);
    let (cloud_hash, local_hash) = result?;

    with_config(app, |config| {
        config.games.push(crate::config::Game {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            save_path: destination.to_string(),
            cloud_name: folder.name,
            backup_path: backup_path.to_string(),
            artwork: None,
            artwork_checked_name: None,
            provider,
            auto_upload: Some(false),
            sync_state: if cloud_hash == local_hash {
                SyncState::Synced
            } else {
                SyncState::Changed
            },
            last_change: None,
            last_upload: latest.created_at,
            last_cloud_version_id: Some(latest.id),
            last_error: None,
            local: crate::config::GameLocalState {
                observed_hash: Some(local_hash),
                uploaded_hash: Some(cloud_hash),
                last_check: Some(now_iso()),
                ..Default::default()
            },
        });
    })
    .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_update_install_excludes_active_transfers_and_releases_its_lock() {
        let active = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
        active.lock().unwrap().insert("game:save".to_string());
        assert!(super::begin_app_update_install(active.clone()).is_err());
        assert!(!active.lock().unwrap().contains(super::APP_UPDATE_KEY));

        active.lock().unwrap().clear();
        let guard = super::begin_app_update_install(active.clone()).unwrap();
        assert!(active.lock().unwrap().contains(super::APP_UPDATE_KEY));
        assert!(super::begin_app_update_install(active.clone()).is_err());
        drop(guard);
        assert!(active.lock().unwrap().is_empty());
    }

    use super::*;

    fn version(id: &str, name: &str, date: Option<&str>) -> RemoteFile {
        RemoteFile {
            id: id.into(),
            name: name.into(),
            created_at: date.map(str::to_string),
            size_bytes: 1,
        }
    }

    #[test]
    fn cloud_versions_without_dates_use_zip_timestamps_including_uuid_suffix() {
        let files = sorted_game_versions(vec![
            version("old", "Stardew-Valley_2026-09-12_10-01-37-888.zip", None),
            version("unknown", "unknown.zip", None),
            version(
                "new",
                "Stardew-Valley_2026-09-15_12-50-10-658_550e8400-e29b-41d4-a716-446655440000.zip",
                None,
            ),
            version("ignore", "notes.txt", Some("2027-01-01T00:00:00Z")),
            version(
                "middle",
                "Stardew-Valley_2026-09-13_20-14-07-810.zip",
                Some("invalid"),
            ),
        ]);
        assert_eq!(
            files
                .iter()
                .map(|file| file.id.as_str())
                .collect::<Vec<_>>(),
            ["new", "middle", "old", "unknown"]
        );
        assert!(files[0].created_at.is_some());
        assert!(files[3].created_at.is_none());
    }

    #[test]
    fn cloud_versions_compare_instants_not_iso_text_or_file_names() {
        let files = sorted_game_versions(vec![
            version(
                "old",
                "save_2027-01-01_00-00-00-000.zip",
                Some("2026-09-15T12:00:00+02:00"),
            ),
            version("new", "save.zip", Some("2026-09-15T11:00:00Z")),
        ]);
        assert_eq!(files[0].id, "new");
    }

    #[test]
    fn latest_cloud_version_is_not_offered_again_after_restore() {
        let mut game = test_game();
        let latest = version("latest", "save.zip", Some("2026-11-06T12:00:00Z"));
        game.last_upload = Some("2026-11-05T12:00:00Z".into());
        assert!(remote_game_version_is_newer(&game, &latest, None));
        game.last_cloud_version_id = Some(latest.id.clone());
        assert!(!remote_game_version_is_newer(&game, &latest, None));
        assert!(!remote_game_version_is_newer(
            &test_game(),
            &version("unknown", "unknown.zip", None),
            None
        ));
    }

    #[test]
    fn older_restored_version_is_compared_with_its_cloud_date() {
        let mut game = test_game();
        game.last_upload = Some("2026-11-06T14:00:00Z".into());
        game.last_cloud_version_id = Some("restored-old".into());
        let baseline = version("restored-old", "old.zip", Some("2026-11-05T12:00:00Z"));
        let latest = version("latest", "latest.zip", Some("2026-11-06T12:00:00Z"));
        assert!(remote_game_version_is_newer(
            &game,
            &latest,
            Some(&baseline)
        ));
        assert!(!remote_game_version_is_newer(
            &game,
            &baseline,
            Some(&baseline)
        ));
    }

    #[test]
    fn cloud_reconnection_sorts_recursive_versions_by_instants_and_filename_dates() {
        let files = vec![
            RemoteTreeFile {
                id: "older".into(),
                relative_path: "save_2026-09-12_10-00-00-000.zip".into(),
                modified_at: None,
                size_bytes: 3,
            },
            RemoteTreeFile {
                id: "newest".into(),
                relative_path: "save_2026-09-15_10-00-00-000.zip".into(),
                modified_at: None,
                size_bytes: 5,
            },
            RemoteTreeFile {
                id: "metadata".into(),
                relative_path: "notes.txt".into(),
                modified_at: Some("2027-01-01T00:00:00Z".into()),
                size_bytes: 1,
            },
            RemoteTreeFile {
                id: "known".into(),
                relative_path: "save.zip".into(),
                modified_at: Some("2026-09-13T10:00:00+02:00".into()),
                size_bytes: 7,
            },
        ];
        let versions = sorted_tree_game_versions(files);
        assert_eq!(
            versions
                .iter()
                .map(|file| file.id.as_str())
                .collect::<Vec<_>>(),
            ["newest", "known", "older"]
        );
        assert!(versions[0].created_at.is_some());
        assert_eq!(versions[0].size_bytes, 5);
    }

    fn test_game() -> crate::config::Game {
        serde_json::from_value(serde_json::json!({
            "id": "test-game", "name": "Stardew Valley", "savePath": "unused",
            "provider": "gdrive", "autoUpload": true, "syncState": "synced"
        }))
        .unwrap()
    }

    #[test]
    fn legacy_games_need_content_baseline_before_automatic_cloud_upload() {
        let mut game = test_game();
        game.last_cloud_version_id = Some("legacy-version".into());
        assert!(game.local.observed_hash.is_none());
        assert!(!should_upload_game(
            &game,
            &AppConfig::default(),
            GameSaveAction::Check,
            "new"
        ));
        assert!(should_upload_game(
            &game,
            &AppConfig::default(),
            GameSaveAction::Upload,
            "new"
        ));
    }

    #[test]
    fn restart_preserves_baseline_and_detects_offline_changes_without_duplicate_uploads() {
        let mut game = test_game();
        game.local.uploaded_hash = Some("old".into());
        game.local.observed_hash = Some("old".into());
        let json = serde_json::to_string(&game).unwrap();
        let mut restored: crate::config::Game = serde_json::from_str(&json).unwrap();
        let cfg = AppConfig::default();
        assert!(!should_upload_game(
            &restored,
            &cfg,
            GameSaveAction::Check,
            "old"
        ));
        record_game_observation(&mut restored, "changed-while-app-was-off");
        assert!(should_upload_game(
            &restored,
            &cfg,
            GameSaveAction::Check,
            "changed-while-app-was-off"
        ));
        assert!(!should_upload_game(
            &restored,
            &cfg,
            GameSaveAction::Local,
            "changed-while-app-was-off"
        ));
        restored.auto_upload = Some(false);
        assert!(!should_upload_game(
            &restored,
            &cfg,
            GameSaveAction::Check,
            "new"
        ));
    }

    #[tokio::test]
    async fn local_snapshot_survives_cloud_failure_and_corruption_is_not_reused() {
        let root = temp_test_dir("cloud-failure");
        let mut game = test_game();
        game.save_path = root.join("Saves").to_string_lossy().into_owned();
        game.backup_path = root.join("Backups").to_string_lossy().into_owned();
        std::fs::create_dir_all(Path::new(&game.save_path).join("Farm2")).unwrap();
        std::fs::write(Path::new(&game.save_path).join("Farm2/save"), b"farm-data").unwrap();
        let mut cfg = AppConfig::default();
        let (archive, hash) = stable_game_backup(&game, &cfg).await.unwrap();
        game.local.backup_file = Some(archive.to_string_lossy().into_owned());
        game.local.backup_hash = Some(hash.clone());
        game.local.last_backup = Some(now_iso());
        cfg.games.push(game.clone());
        let failure = make_provider(game.provider, &cfg)
            .err()
            .expect("disconnected cloud must fail");
        set_game_result(&mut cfg, &game.id, &Err(failure), false);
        assert_eq!(
            cfg.games[0].local.backup_hash.as_deref(),
            Some(hash.as_str())
        );
        assert!(cfg.games[0].local.uploaded_hash.is_none());
        assert_eq!(
            reusable_game_backup(&cfg.games[0], &hash).await,
            Some(archive.clone())
        );
        std::fs::write(&archive, b"corrupted-zip").unwrap();
        assert!(reusable_game_backup(&cfg.games[0], &hash).await.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn backup_waits_for_nested_save_writes_to_finish() {
        let root = temp_test_dir("busy-save");
        let mut game = test_game();
        game.save_path = root.join("Saves").to_string_lossy().into_owned();
        game.backup_path = root.join("Backups").to_string_lossy().into_owned();
        std::fs::create_dir_all(Path::new(&game.save_path).join("Farm2")).unwrap();
        let file = Path::new(&game.save_path).join("Farm2/save");
        std::fs::write(&file, b"previous-save").unwrap();
        let writer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            std::fs::write(file, b"completed-new-save").unwrap();
        });
        let (archive, hash) = stable_game_backup(&game, &AppConfig::default())
            .await
            .unwrap();
        writer.await.unwrap();
        let source = crate::game_save::scan(Path::new(&game.save_path)).unwrap();
        assert_eq!(source.fingerprint(), hash);
        assert_eq!(crate::game_save::scan_archive(&archive).unwrap(), source);
        assert_eq!(
            std::fs::read_dir(archive.parent().unwrap())
                .unwrap()
                .count(),
            1
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    fn temp_test_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("backup-sync-test-{label}-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn cloud_folder_name_is_stable_and_readable() {
        assert_eq!(
            folder_cloud_name(r"C:\Users\Test\Documents", "12345678-abcd"),
            "Documents-12345678"
        );
    }

    #[test]
    fn cloud_connect_backs_up_existing_save_next_to_destination() {
        let root = temp_test_dir("local-before-cloud");
        let destination = root.join("game-save");
        std::fs::create_dir_all(destination.join("nested")).unwrap();
        std::fs::write(destination.join("profile.dat"), b"old-root-save").unwrap();
        std::fs::write(
            destination.join("nested").join("slot.dat"),
            b"old-nested-save",
        )
        .unwrap();

        let archive = backup_local_save_before_cloud(
            &destination,
            "",
            "Example Game",
            crate::config::CompressionLevel::Fast,
        )
        .unwrap()
        .expect("existing save files must create a ZIP backup");

        let backup_dir = root.join("Backup Sync local saves");
        assert_eq!(archive.parent(), Some(backup_dir.as_path()));
        assert!(!archive.starts_with(&destination));

        let restored = root.join("archive-check");
        crate::archive::unzip_to(&archive, &restored).unwrap();
        assert_eq!(
            std::fs::read(restored.join("profile.dat")).unwrap(),
            b"old-root-save"
        );
        assert_eq!(
            std::fs::read(restored.join("nested").join("slot.dat")).unwrap(),
            b"old-nested-save"
        );
        assert!(!restored.join("game-save").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cloud_restore_uses_configured_game_backup_directory() {
        let root = temp_test_dir("configured-game-backup");
        let destination = root.join("game-save");
        let backup_dir = root.join("chosen-backups");
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(destination.join("profile.dat"), b"old-save").unwrap();
        let backup_path = backup_dir.to_string_lossy().to_string();

        let archive = backup_local_save_before_cloud(
            &destination,
            &backup_path,
            "Example Game",
            crate::config::CompressionLevel::Fast,
        )
        .unwrap()
        .expect("existing save files must create a ZIP backup");

        assert_eq!(archive.parent(), Some(backup_dir.as_path()));
        assert!(!archive.starts_with(&destination));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn restore_rejects_path_traversal() {
        let root = temp_test_dir("paths");
        assert!(safe_relative_path(&root, "../outside.txt").is_err());
        assert!(safe_relative_path(&root, r"..\outside.txt").is_err());
        assert!(safe_relative_path(&root, "/absolute.txt").is_err());
        #[cfg(windows)]
        {
            assert!(safe_relative_path(&root, r"C:\outside.txt").is_err());
            assert!(safe_relative_path(&root, "CON.txt").is_err());
        }
    }

    #[test]
    fn restore_copies_remote_files_and_hashes_only_them() {
        let staging = temp_test_dir("staging");
        let destination = temp_test_dir("destination");
        std::fs::create_dir_all(staging.join("nested")).unwrap();
        std::fs::create_dir_all(destination.join("nested")).unwrap();
        std::fs::write(staging.join("nested").join("save.dat"), b"cloud-new").unwrap();
        std::fs::write(destination.join("nested").join("save.dat"), b"local-old").unwrap();
        std::fs::write(destination.join("local-only.txt"), b"keep-me").unwrap();
        let files = vec![RemoteTreeFile {
            id: "remote-1".into(),
            relative_path: "nested/save.dat".into(),
            modified_at: None,
            size_bytes: 9,
        }];

        let hashes = copy_downloaded_files(&staging, &destination, &files).unwrap();

        assert_eq!(
            std::fs::read(destination.join("nested").join("save.dat")).unwrap(),
            b"cloud-new"
        );
        assert_eq!(
            std::fs::read(destination.join("local-only.txt")).unwrap(),
            b"keep-me"
        );
        assert_eq!(hashes.len(), 1);
        assert!(hashes.contains_key("nested/save.dat"));
        assert!(!hashes.contains_key("local-only.txt"));

        let _ = std::fs::remove_dir_all(staging);
        let _ = std::fs::remove_dir_all(destination);
    }
}
