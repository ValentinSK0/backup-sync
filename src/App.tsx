import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getConfig } from "./api";
import Sidebar from "./components/Sidebar";
import ToastStack, { type ToastItem } from "./components/Toast";
import { AlertIcon, SpinnerIcon } from "./components/icons";
import { Button } from "./components/ui";
import { AppContext, type ToastKind } from "./context";
import { baseName, errMsg } from "./format";
import Dashboard from "./pages/Dashboard";
import Folders from "./pages/Folders";
import Games from "./pages/Games";
import Settings from "./pages/Settings";
import type { AppConfig, SyncProgress } from "./types";
import { useAppUpdater } from "./useAppUpdater";

export type Page = "dashboard" | "folders" | "games" | "settings";

export interface ActivityItem {
  id: number;
  at: number; // epoch ms
  progress: SyncProgress;
}

const MAX_ACTIVITY = 20;
const TOAST_TTL_MS = 5000;

let nextId = 0;

export default function App() {
  const updater = useAppUpdater();
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [page, setPage] = useState<Page>("dashboard");
  const [activity, setActivity] = useState<ActivityItem[]>([]);
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const configRef = useRef<AppConfig | null>(null);
  useEffect(() => {
    configRef.current = config;
  }, [config]);

  const dismissToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const toast = useCallback(
    (kind: ToastKind, message: string) => {
      const id = ++nextId;
      setToasts((prev) => [...prev, { id, kind, message }]);
      window.setTimeout(() => dismissToast(id), TOAST_TTL_MS);
    },
    [dismissToast],
  );

  const loadConfig = useCallback(() => {
    setLoadError(null);
    getConfig()
      .then(setConfig)
      .catch((e) => setLoadError(errMsg(e)));
  }, []);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  const handleProgress = useCallback(
    (p: SyncProgress) => {
      setActivity((prev) =>
        [{ id: ++nextId, at: Date.now(), progress: p }, ...prev].slice(
          0,
          MAX_ACTIVITY,
        ),
      );

      const cfg = configRef.current;
      let name = p.id;
      if (cfg) {
        if (p.targetType === "folder") {
          const f = cfg.watchedFolders.find((x) => x.id === p.id);
          if (f) name = baseName(f.path);
        } else {
          const g = cfg.games.find((x) => x.id === p.id);
          if (g) name = g.name;
        }
      }

      // Priebežné udalosti nechávame len vo feede, inak by toasty spamovali.
      if (p.status === "done") {
        toast("success", p.message ?? `Hotovo: ${name}`);
      } else if (p.status === "error") {
        toast("error", p.message ? `${name}: ${p.message}` : `Chyba: ${name}`);
      }
    },
    [toast],
  );

  useEffect(() => {
    let disposed = false;
    const unlisteners: UnlistenFn[] = [];

    (async () => {
      const un1 = await listen<AppConfig>("config-changed", (e) => {
        setConfig(e.payload);
      });
      const un2 = await listen<SyncProgress>("sync-progress", (e) => {
        handleProgress(e.payload);
      });
      if (disposed) {
        un1();
        un2();
      } else {
        unlisteners.push(un1, un2);
      }
    })();

    return () => {
      disposed = true;
      for (const un of unlisteners) un();
    };
  }, [handleProgress]);

  if (loadError) {
    return (
      <div className="flex h-screen flex-col items-center justify-center gap-4 bg-zinc-950 p-8 text-center">
        <AlertIcon className="h-10 w-10 text-red-400" />
        <div>
          <p className="text-sm font-medium text-zinc-200">
            Nepodarilo sa načítať konfiguráciu
          </p>
          <p className="mt-1 max-w-md break-words text-xs text-zinc-500">
            {loadError}
          </p>
        </div>
        <Button variant="primary" onClick={loadConfig}>
          Skúsiť znova
        </Button>
      </div>
    );
  }

  if (!config) {
    return (
      <div className="flex h-screen items-center justify-center gap-3 bg-zinc-950 text-sm text-zinc-500">
        <SpinnerIcon className="h-5 w-5 text-indigo-400" />
        Načítava sa…
      </div>
    );
  }

  return (
    <AppContext.Provider value={{ config, setConfig, toast, updater }}>
      <div className="flex h-screen overflow-hidden bg-zinc-950 text-zinc-100">
        <Sidebar page={page} onNavigate={setPage} />
        <main className="min-w-0 flex-1 overflow-y-auto px-8 py-6">
          {updater.info?.update && page !== "settings" && (
            <div className="mb-5 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-indigo-500/30 bg-indigo-500/10 px-4 py-3">
              <p className="text-sm text-indigo-100">
                Backup Sync {updater.info.update.version} je {updater.info.update.downloaded ? "pripravený na inštaláciu" : "dostupný"}.
              </p>
              <Button onClick={() => setPage("settings")}>Zobraziť aktualizáciu</Button>
            </div>
          )}
          {page === "dashboard" && <Dashboard activity={activity} onNavigate={setPage} />}
          {page === "folders" && <Folders />}
          {page === "games" && <Games />}
          {page === "settings" && <Settings />}
        </main>
      </div>
      <ToastStack toasts={toasts} onDismiss={dismissToast} />
    </AppContext.Provider>
  );
}
