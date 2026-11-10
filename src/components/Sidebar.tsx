import type { Page } from "../App";
import appIcon from "../../src-tauri/icons/128x128.png";
import { useApp } from "../context";
import {
  FolderIcon,
  GamepadIcon,
  HomeIcon,
  SettingsIcon,
} from "./icons";

const NAV: { page: Page; label: string; Icon: typeof HomeIcon }[] = [
  { page: "dashboard", label: "Prehľad", Icon: HomeIcon },
  { page: "folders", label: "Priečinky", Icon: FolderIcon },
  { page: "games", label: "Hry", Icon: GamepadIcon },
  { page: "settings", label: "Nastavenia", Icon: SettingsIcon },
];

export default function Sidebar({
  page,
  onNavigate,
}: {
  page: Page;
  onNavigate: (page: Page) => void;
}) {
  const { config } = useApp();

  const pendingByPage: Partial<Record<Page, number>> = {
    folders: config.watchedFolders.filter((f) => f.syncState === "changed")
      .length,
    games: config.games.filter((g) => g.syncState === "changed").length,
  };

  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-zinc-800 bg-zinc-950 px-3 py-4">
      <div className="mb-6 flex items-center gap-2.5 px-2">
        <img src={appIcon} alt="" aria-hidden="true" className="h-9 w-9 shrink-0 object-contain" />
        <div>
          <div className="text-sm font-semibold leading-tight text-zinc-100">
            Backup Sync
          </div>
          <div className="text-[11px] leading-tight text-zinc-500">
            Zálohy a save súbory
          </div>
        </div>
      </div>

      <nav className="flex flex-col gap-1">
        {NAV.map(({ page: p, label, Icon }) => {
          const active = p === page;
          const pending = pendingByPage[p] ?? 0;
          return (
            <button
              key={p}
              onClick={() => onNavigate(p)}
              className={`group flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors
                ${
                  active
                    ? "bg-indigo-600/15 text-indigo-300"
                    : "text-zinc-400 hover:bg-zinc-800/70 hover:text-zinc-100"
                }`}
            >
              <Icon
                className={`h-4.5 w-4.5 shrink-0 ${active ? "text-indigo-400" : "text-zinc-500 group-hover:text-zinc-300"}`}
              />
              <span className="flex-1 text-left">{label}</span>
              {pending > 0 && (
                <span className="rounded-full bg-amber-500/15 px-1.5 py-0.5 text-[11px] font-semibold text-amber-400">
                  {pending}
                </span>
              )}
            </button>
          );
        })}
      </nav>

      <div className="mt-auto space-y-2 border-t border-zinc-800 px-2 pt-4 text-xs text-zinc-500">
        <div className="flex items-center gap-2">
          <span
            className={`h-2 w-2 rounded-full ${config.providers.gdrive.connected ? "bg-emerald-400" : "bg-zinc-700"}`}
          />
          <span>Google Drive</span>
        </div>
        <div className="flex items-center gap-2">
          <span
            className={`h-2 w-2 rounded-full ${config.providers.ulozto.connected ? "bg-emerald-400" : "bg-zinc-700"}`}
          />
          <span>Ulož.to Disk</span>
        </div>
        <div className="flex items-center gap-2">
          <span
            className={`h-2 w-2 rounded-full ${config.providers.customCloud.connected ? "bg-emerald-400" : "bg-zinc-700"}`}
          />
          <span>Vlastný cloud</span>
        </div>
      </div>
    </aside>
  );
}
