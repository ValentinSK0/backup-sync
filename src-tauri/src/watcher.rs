use crate::config::{now_iso, AppConfig, SyncState};
use crate::sync;
use crate::AppState;
use notify::{RecursiveMode, Watcher};
use std::collections::HashMap;
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
        let mut watcher = match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(_) => return,
        };
        for root in &roots {
            if root.path.is_dir() {
                let _ = watcher.watch(&root.path, RecursiveMode::Recursive);
            }
        }

        let mut pending: HashMap<Target, Instant> = HashMap::new();
        loop {
            if stop.load(Ordering::SeqCst) {
                return; // watcher sa dropne, sledovanie končí
            }
            match rx.recv_timeout(Duration::from_millis(400)) {
                Ok(Ok(event)) => {
                    if !matters(&event.kind) {
                        continue;
                    }
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
                Ok(Err(_)) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
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
            let mut retry_after_sync = false;
            let cfg = match sync::with_config(&app, |c| {
                if let Some(g) = c.games.iter_mut().find(|g| g.id == id) {
                    g.last_change = Some(now_iso());
                    if g.sync_state == SyncState::Syncing {
                        retry_after_sync = sync::record_pending_change(&app, "game", &id);
                    } else {
                        g.sync_state = SyncState::Changed;
                    }
                }
            })
            .await
            {
                Ok(config) => config,
                Err(error) => {
                    sync::emit_progress(&app, "game", &id, "error", Some(error));
                    return;
                }
            };
            let auto = cfg
                .games
                .iter()
                .find(|g| g.id == id)
                .map(|g| g.auto_upload.unwrap_or(cfg.settings.auto_upload_saves))
                .unwrap_or(false);
            if auto {
                if retry_after_sync {
                    wait_until_idle(&app, "game", &id).await;
                    let current = sync::snapshot(&app).await;
                    let still_auto =
                        current
                            .games
                            .iter()
                            .find(|game| game.id == id)
                            .is_some_and(|game| {
                                game.auto_upload
                                    .unwrap_or(current.settings.auto_upload_saves)
                            });
                    if !still_auto {
                        return;
                    }
                }
                let _ = sync::upload_game(&app, &id).await;
            }
        }
    }
}

async fn wait_until_idle(app: &AppHandle, kind: &str, id: &str) {
    while sync::is_sync_active(app, kind, id) {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}
