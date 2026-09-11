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

struct SyncGuard {
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

fn begin_sync(app: &AppHandle, kind: &str, id: &str) -> Result<SyncGuard, String> {
    let active = app.state::<AppState>().active_syncs.clone();
    let key = sync_key(kind, id);
    {
        let mut set = active
            .lock()
            .map_err(|_| "Interný zámok synchronizácie je poškodený.".to_string())?;
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
    game_name: &str,
    level: crate::config::CompressionLevel,
) -> Result<Option<PathBuf>, String> {
    if !contains_local_save_files(destination)? {
        return Ok(None);
    }
    let parent = destination.parent().ok_or_else(|| {
        format!(
            "Pôvodný save sa nedá zazálohovať: priečinok nemá nadradenú cestu: {}",
            destination.display()
        )
    })?;
    let backup_dir = parent.join("Backup Sync local saves");
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%3f");
    let archive = backup_dir.join(format!(
        "{}_local-before-cloud_{}-{}.zip",
        sanitize(game_name),
        stamp,
        uuid::Uuid::new_v4()
    ));
    crate::archive::zip_dir_to(destination, &archive, level)
        .map_err(|e| format!("Pôvodný save sa nepodarilo zazálohovať: {e}"))?;
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

fn game_remote_dir(name: &str) -> Vec<String> {
    let mut dir = games_backups_root();
    dir.push(sanitize(name));
    dir
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
    result: &Result<(), String>,
    changed_during_sync: bool,
) {
    if let Some(g) = cfg.games.iter_mut().find(|g| g.id == id) {
        match result {
            Ok(()) => {
                g.sync_state = if changed_during_sync {
                    SyncState::Changed
                } else {
                    SyncState::Synced
                };
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

fn set_game_restore_result(
    cfg: &mut AppConfig,
    id: &str,
    result: &Result<(), String>,
    changed_during_sync: bool,
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
            }
            Err(error) => {
                game.sync_state = SyncState::Error;
                game.last_error = Some(error.clone());
            }
        }
    }
}

/// Zabalí save priečinok do zipu a nahrá ako novú verziu na cloud.
pub async fn upload_game(app: &AppHandle, id: &str) -> Result<(), String> {
    let _sync = begin_sync(app, "game", id)?;
    let cfg = with_config(app, |c| {
        if let Some(g) = c.games.iter_mut().find(|g| g.id == id) {
            g.sync_state = SyncState::Syncing;
            g.last_error = None;
        }
    })
    .await?;
    let game = cfg
        .games
        .iter()
        .find(|g| g.id == id)
        .cloned()
        .ok_or_else(|| "Hra sa nenašla.".to_string())?;

    emit_progress(app, "game", id, "uploading", Some("Balím save…".into()));
    let result = upload_game_inner(app, &game, &cfg).await;

    with_config(app, |c| {
        let changed = take_pending_change(app, "game", id);
        set_game_result(c, id, &result, changed);
    })
    .await?;
    match &result {
        Ok(()) => emit_progress(app, "game", id, "done", Some("Save nahraný".into())),
        Err(e) => emit_progress(app, "game", id, "error", Some(e.clone())),
    }
    result
}

async fn upload_game_inner(
    app: &AppHandle,
    game: &crate::config::Game,
    cfg: &AppConfig,
) -> Result<(), String> {
    let save_path = PathBuf::from(&game.save_path);
    let level = cfg.settings.compression_level;
    let zip_path = {
        let save_path = save_path.clone();
        tokio::task::spawn_blocking(move || crate::archive::zip_dir(&save_path, level))
            .await
            .map_err(|e| e.to_string())??
    };

    let storage = make_provider(game.provider, cfg)?;
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%3f");
    let name = format!("{}_{}.zip", sanitize(&game.name), stamp);
    emit_progress(
        app,
        "game",
        &game.id,
        "uploading",
        Some(format!("Nahrávam {name}…")),
    );
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
    emit_progress(
        app,
        "game",
        id,
        "uploading",
        Some("Sťahujem verziu…".into()),
    );
    let result = async {
        let storage = make_provider(game.provider, &cfg)?;
        let selected = storage
            .list_files(&game_remote_dir(&game.name))
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
        suppress_watcher(app, &game.save_path, Duration::from_secs(30));
        let destination = PathBuf::from(&game.save_path);
        let staging_for_copy = staging.clone();
        tokio::task::spawn_blocking(move || copy_staged_tree(&staging_for_copy, &destination))
            .await
            .map_err(|e| e.to_string())??;
        suppress_watcher(app, &game.save_path, Duration::from_secs(15));
        Ok::<(), String>(())
    }
    .await;
    let _ = std::fs::remove_file(&tmp);
    let _ = std::fs::remove_dir_all(&staging);
    with_config(app, |config| {
        let changed = take_pending_change(app, "game", id);
        set_game_restore_result(config, id, &result, changed);
    })
    .await?;
    match &result {
        Ok(()) => emit_progress(app, "game", id, "done", Some("Verzia obnovená".into())),
        Err(error) => emit_progress(app, "game", id, "error", Some(error.clone())),
    }
    result
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
        let versions: Vec<_> = storage
            .list_files_recursive(&folder.id)
            .await?
            .into_iter()
            .filter(|f| f.relative_path.to_lowercase().ends_with(".zip"))
            .collect();
        if versions.is_empty() {
            continue;
        }
        let latest_at = versions.iter().filter_map(|f| f.modified_at.clone()).max();
        let size_bytes = versions
            .iter()
            .fold(0u64, |total, f| total.saturating_add(f.size_bytes));
        let already_connected = cfg
            .games
            .iter()
            .any(|g| g.provider == provider && sanitize(&g.name) == folder.name);
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
) -> Result<AppConfig, String> {
    if !Path::new(destination).is_dir() {
        return Err(format!("Cieľový priečinok neexistuje: {destination}"));
    }
    let destination_canonical = canonical_path(destination)?;
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

    let latest = storage
        .list_files_recursive(&folder.id)
        .await?
        .into_iter()
        .filter(|f| f.relative_path.to_lowercase().ends_with(".zip"))
        .max_by(|a, b| a.modified_at.cmp(&b.modified_at))
        .ok_or_else(|| "Táto herná záloha nemá žiadnu verziu.".to_string())?;

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
    let _local_backup = tokio::task::spawn_blocking(move || {
        backup_local_save_before_cloud(&backup_destination, &backup_game_name, backup_level)
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
        suppress_watcher(app, destination, Duration::from_secs(30));
        {
            let staging = staging.clone();
            let destination_path = destination_path.clone();
            tokio::task::spawn_blocking(move || copy_staged_tree(&staging, &destination_path))
                .await
                .map_err(|e| e.to_string())??;
        }
        suppress_watcher(app, destination, Duration::from_secs(15));
        Ok::<(), String>(())
    }
    .await;
    let _ = std::fs::remove_file(&tmp);
    let _ = std::fs::remove_dir_all(&staging);
    result?;

    with_config(app, |config| {
        config.games.push(crate::config::Game {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            save_path: destination.to_string(),
            provider,
            auto_upload: Some(false),
            sync_state: SyncState::Synced,
            last_change: None,
            last_upload: Some(now_iso()),
            last_error: None,
        });
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

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
