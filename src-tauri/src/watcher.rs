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
        roots.push(WatchRoot {
            path: PathBuf::from(&f.path),
            target: Target::Folder(f.id.clone()),
        });
    }
    for g in &cfg.games {
        roots.push(WatchRoot {
            path: PathBuf::from(&g.save_path),
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
                        for root in &roots {
                            if path.starts_with(&root.path) {
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

fn is_suppressed(suppress: &Arc<std::sync::Mutex<HashMap<String, Instant>>>, path: &Path) -> bool {
    let mut map = suppress.lock().unwrap();
    let now = Instant::now();
    map.retain(|_, until| *until > now);
    map.keys().any(|root| path.starts_with(root))
}

async fn handle_change(app: AppHandle, target: Target) {
    match target {
        Target::Folder(id) => {
            sync::with_config(&app, |c| {
                if let Some(f) = c.watched_folders.iter_mut().find(|f| f.id == id) {
                    if f.sync_state != SyncState::Syncing {
                        f.sync_state = SyncState::Changed;
                    }
                }
            })
            .await;
            // priečinky sa zálohujú automaticky (real-time sync)
            let _ = sync::backup_folder(&app, &id).await;
        }
        Target::Game(id) => {
            let cfg = sync::with_config(&app, |c| {
                if let Some(g) = c.games.iter_mut().find(|g| g.id == id) {
                    if g.sync_state != SyncState::Syncing {
                        g.sync_state = SyncState::Changed;
                        g.last_change = Some(now_iso());
                    }
                }
            })
            .await;
            let auto = cfg
                .games
                .iter()
                .find(|g| g.id == id)
                .map(|g| g.auto_upload.unwrap_or(cfg.settings.auto_upload_saves))
                .unwrap_or(false);
            if auto {
                let _ = sync::upload_game(&app, &id).await;
            }
        }
    }
}
