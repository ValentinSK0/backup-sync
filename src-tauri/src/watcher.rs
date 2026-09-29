use crate::config::{AppConfig, SyncState};
use crate::sync;
use crate::AppState;
use notify::{RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Target {
    Folder(String),
    Game(String),
}

struct WatchRoot {
    path: PathBuf,
    comparable_path: PathBuf,
    target: Target,
}

/// Zastaví starý watcher a spustí nový podľa aktuálneho configu.
/// Volať po každej zmene sledovaných ciest alebo debounce nastavenia.
pub fn rebuild(app: &AppHandle, cfg: &AppConfig) {
    let state = app.state::<AppState>();

    // zastav starý beh
    if let Some(old) = state.watcher_stop.lock().unwrap().take() {
        old.store(true, Ordering::SeqCst);
    }

    let stop = Arc::new(AtomicBool::new(false));
    *state.watcher_stop.lock().unwrap() = Some(stop.clone());

    let mut roots: Vec<WatchRoot> = Vec::new();
    for f in &cfg.watched_folders {
        let path = PathBuf::from(&f.path);
        roots.push(WatchRoot {
            comparable_path: sync::comparable_path(&path),
            path,
            target: Target::Folder(f.id.clone()),
        });
    }
    for g in &cfg.games {
        let path = PathBuf::from(&g.save_path);
        roots.push(WatchRoot {
            comparable_path: sync::comparable_path(&path),
            path,
            target: Target::Game(g.id.clone()),
        });
    }

    let debounce = Duration::from_secs(cfg.settings.debounce_seconds.max(1));
    let suppress = state.suppress.clone();
    let app = app.clone();

    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(tx.clone()).ok();
        let mut watched = HashSet::new();
        let mut last_scan = Instant::now() - Duration::from_secs(30);

        let mut pending: HashMap<Target, Instant> = HashMap::new();
        loop {
            if stop.load(Ordering::SeqCst) {
                return; // watcher sa dropne, sledovanie končí
            }
            match rx.recv_timeout(Duration::from_millis(400)) {
                Ok(Ok(event)) if matters(&event.kind) => {
                    let now = Instant::now();
                    for path in &event.paths {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            if sync::is_ignored_file(name) {
                                continue;
                            }
                        }
                        if is_suppressed(&suppress, path) {
                            continue;
                        }
                        let comparable_path = sync::comparable_path(path);
                        for root in &roots {
                            if comparable_path.starts_with(&root.comparable_path) {
                                pending.insert(root.target.clone(), now);
                            }
                        }
                    }
                }
                Ok(Ok(_)) => {}
                Ok(Err(error)) => {
                    watched.clear();
                    for root in &roots {
                        report_watch_error(&app, &root.target, Some(error.to_string()));
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // Continue content checks even when the native watcher cannot start.
                    std::thread::sleep(Duration::from_millis(400));
                }
            }

            if last_scan.elapsed() >= Duration::from_secs(30) {
                last_scan = Instant::now();
                if watcher.is_none() {
                    watcher = notify::recommended_watcher(tx.clone()).ok();
                }
                for root in &roots {
                    if !root.path.is_dir() {
                        watched.remove(&root.path);
                    }
                    if !watched.contains(&root.path) {
                        let result = match watcher.as_mut() {
                            Some(watcher) => watcher
                                .watch(&root.path, RecursiveMode::Recursive)
                                .map_err(|e| e.to_string()),
                            None => Err("Systémové sledovanie nie je dostupné.".into()),
                        };
                        match result {
                            Ok(()) => {
                                watched.insert(root.path.clone());
                                report_watch_error(&app, &root.target, None);
                            }
                            Err(error) => report_watch_error(&app, &root.target, Some(error)),
                        }
                    }
                    if let Target::Game(id) = &root.target {
                        if !is_suppressed(&suppress, &root.path) {
                            let app = app.clone();
                            let id = id.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = sync::check_game_save(&app, &id).await;
                            });
                        }
                    }
                }
            }

            // debounce tik: spracuj ciele, ktoré sa už `debounce` neozvali
            let now = Instant::now();
            let ready: Vec<Target> = pending
                .iter()
                .filter(|(_, last)| now.duration_since(**last) >= debounce)
                .map(|(t, _)| t.clone())
                .collect();
            for target in ready {
                pending.remove(&target);
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    handle_change(app, target).await;
                });
            }
        }
    });
}

fn report_watch_error(app: &AppHandle, target: &Target, error: Option<String>) {
    let app = app.clone();
    let target = target.clone();
    tauri::async_runtime::spawn(async move {
        match target {
            Target::Game(id) => {
                let message = error.map(|e| {
                    format!("Sledovanie zmien zlyhalo; obsah kontrolujem každých 30 sekúnd. {e}")
                });
                let cfg = sync::snapshot(&app).await;
                if cfg
                    .games
                    .iter()
                    .find(|g| g.id == id)
                    .is_some_and(|g| g.local.watch_error != message)
                {
                    let _ = sync::with_config(&app, |c| {
                        if let Some(g) = c.games.iter_mut().find(|g| g.id == id) {
                            g.local.watch_error = message;
                        }
                    })
                    .await;
                }
            }
            Target::Folder(id) => {
                if let Some(error) = error {
                    sync::emit_progress(
                        &app,
                        "folder",
                        &id,
                        "error",
                        Some(format!("Sledovanie priečinka zlyhalo: {error}")),
                    );
                }
            }
        }
    });
}

fn matters(kind: &notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Create(_) | notify::EventKind::Modify(_) | notify::EventKind::Remove(_)
    )
}

fn is_suppressed(suppress: &Arc<std::sync::Mutex<HashMap<PathBuf, Instant>>>, path: &Path) -> bool {
    let mut map = suppress.lock().unwrap();
    let now = Instant::now();
    map.retain(|_, until| *until > now);
    let comparable_path = sync::comparable_path(path);
    map.keys().any(|root| comparable_path.starts_with(root))
}

async fn handle_change(app: AppHandle, target: Target) {
    match target {
        Target::Folder(id) => {
            let mut retry_after_sync = false;
            let cfg = match sync::with_config(&app, |c| {
                if let Some(f) = c.watched_folders.iter_mut().find(|f| f.id == id) {
                    if f.sync_state == SyncState::Syncing {
                        retry_after_sync = sync::record_pending_change(&app, "folder", &id);
                    } else {
                        f.sync_state = SyncState::Changed;
                    }
                }
            })
            .await
            {
                Ok(config) => config,
                Err(error) => {
                    sync::emit_progress(&app, "folder", &id, "error", Some(error));
                    return;
                }
            };
            let auto = cfg
                .watched_folders
                .iter()
                .find(|f| f.id == id)
                .map(|f| {
                    (f.sync_state != SyncState::Syncing || retry_after_sync)
                        && f.auto_backup.unwrap_or(cfg.settings.auto_backup_folders)
                })
                .unwrap_or(false);
            if auto {
                if retry_after_sync {
                    wait_until_idle(&app, "folder", &id).await;
                    let current = sync::snapshot(&app).await;
                    let still_auto = current
                        .watched_folders
                        .iter()
                        .find(|folder| folder.id == id)
                        .is_some_and(|folder| {
                            folder
                                .auto_backup
                                .unwrap_or(current.settings.auto_backup_folders)
                        });
                    if !still_auto {
                        return;
                    }
                }
                let _ = sync::backup_folder(&app, &id).await;
            }
        }
        Target::Game(id) => {
            let _ = sync::check_game_save(&app, &id).await;
        }
    }
}

async fn wait_until_idle(app: &AppHandle, kind: &str, id: &str) {
    while sync::is_sync_active(app, kind, id) {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_watcher_receives_changes_inside_a_farm() {
        let root = std::env::temp_dir().join(format!("backup-sync-watch-{}", uuid::Uuid::new_v4()));
        let farm = root.join("Saves/Farm2/nested");
        std::fs::create_dir_all(&farm).unwrap();
        let file = farm.join("save");
        std::fs::write(&file, b"previous").unwrap();
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx).unwrap();
        watcher
            .watch(&root.join("Saves"), RecursiveMode::Recursive)
            .unwrap();
        std::fs::write(&file, b"new-save").unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        let mut detected = false;
        while let Some(remaining) = until.checked_duration_since(Instant::now()) {
            match rx.recv_timeout(remaining) {
                Ok(Ok(event)) if matters(&event.kind) => {
                    if event
                        .paths
                        .iter()
                        .any(|path| sync::comparable_path(path) == sync::comparable_path(&file))
                    {
                        detected = true;
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        drop(watcher);
        std::fs::remove_dir_all(&root).unwrap();
        assert!(
            detected,
            "recursive watcher must deliver a nested save change"
        );
    }
}
