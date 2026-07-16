import type { ReactNode } from "react";
import type { ActivityItem } from "../App";
import StateBadge from "../components/StateBadge";
import {
  AlertIcon,
  CheckIcon,
  FolderIcon,
  GamepadIcon,
  HistoryIcon,
  UploadIcon,
} from "../components/icons";
import { Card, PageHeader } from "../components/ui";
import { useApp } from "../context";
import { baseName, formatDate, formatTime } from "../format";
import type { SyncProgress } from "../types";

function StatCard({
  label,
  value,
  icon,
  accent = false,
}: {
  label: string;
  value: string;
  icon: ReactNode;
  accent?: boolean;
}) {
  return (
    <Card className="flex items-center gap-4 p-4">
      <div
        className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-lg
          ${accent ? "bg-amber-500/15 text-amber-400" : "bg-indigo-600/15 text-indigo-400"}`}
      >
        {icon}
      </div>
      <div className="min-w-0">
        <div className="truncate text-xs text-zinc-500">{label}</div>
        <div
          className={`truncate text-xl font-semibold ${accent ? "text-amber-400" : "text-zinc-100"}`}
        >
          {value}
        </div>
      </div>
    </Card>
  );
}

const STATUS_ICON: Record<
  SyncProgress["status"],
  { Icon: typeof CheckIcon; cls: string; label: string }
> = {
  uploading: { Icon: UploadIcon, cls: "text-sky-400", label: "Nahráva sa" },
  done: { Icon: CheckIcon, cls: "text-emerald-400", label: "Dokončené" },
  error: { Icon: AlertIcon, cls: "text-red-400", label: "Chyba" },
};

export default function Dashboard({ activity }: { activity: ActivityItem[] }) {
  const { config } = useApp();
  const { watchedFolders, games } = config;

  const pendingFolders = watchedFolders.filter(
    (f) => f.syncState === "changed",
  );
  const pendingGames = games.filter((g) => g.syncState === "changed");
  const pendingCount = pendingFolders.length + pendingGames.length;

  const lastSyncMs = Math.max(
    0,
    ...watchedFolders
      .map((f) => (f.lastSync ? new Date(f.lastSync).getTime() : 0))
      .filter((n) => Number.isFinite(n)),
    ...games
      .map((g) => (g.lastUpload ? new Date(g.lastUpload).getTime() : 0))
      .filter((n) => Number.isFinite(n)),
  );

  const resolveName = (p: SyncProgress): string => {
    if (p.targetType === "folder") {
      const f = watchedFolders.find((x) => x.id === p.id);
      return f ? baseName(f.path) : p.id;
    }
    const g = games.find((x) => x.id === p.id);
    return g ? g.name : p.id;
  };

  return (
    <div>
      <PageHeader
        title="Prehľad"
        subtitle="Stav záloh a synchronizácie na jednom mieste."
      />

      <div className="grid grid-cols-2 gap-4 xl:grid-cols-4">
        <StatCard
          label="Sledované priečinky"
          value={String(watchedFolders.length)}
          icon={<FolderIcon className="h-5 w-5" />}
        />
        <StatCard
          label="Hry"
          value={String(games.length)}
          icon={<GamepadIcon className="h-5 w-5" />}
        />
        <StatCard
          label="Čakajúce zmeny"
          value={String(pendingCount)}
          icon={<UploadIcon className="h-5 w-5" />}
          accent={pendingCount > 0}
        />
        <StatCard
          label="Posledná synchronizácia"
          value={lastSyncMs > 0 ? formatDate(new Date(lastSyncMs).toISOString()) : "—"}
          icon={<HistoryIcon className="h-5 w-5" />}
        />
      </div>

      <div className="mt-6 grid grid-cols-1 gap-6 xl:grid-cols-2">
        <section>
          <h2 className="mb-3 text-sm font-semibold text-zinc-300">
            Čakajúce zmeny
          </h2>
          {pendingCount === 0 ? (
            <Card className="p-5 text-sm text-zinc-500">
              Žiadne čakajúce zmeny — všetko je zálohované.
            </Card>
          ) : (
            <Card className="divide-y divide-zinc-800">
              {pendingFolders.map((f) => (
                <div key={f.id} className="flex items-center gap-3 px-4 py-3">
                  <FolderIcon className="h-4.5 w-4.5 shrink-0 text-zinc-500" />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium text-zinc-200">
                      {baseName(f.path)}
                    </div>
                    <div className="truncate font-mono text-xs text-zinc-600">
                      {f.path}
                    </div>
                  </div>
                  <StateBadge state={f.syncState} />
                </div>
              ))}
              {pendingGames.map((g) => (
                <div key={g.id} className="flex items-center gap-3 px-4 py-3">
                  <GamepadIcon className="h-4.5 w-4.5 shrink-0 text-zinc-500" />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium text-zinc-200">
                      {g.name}
                    </div>
                    <div className="truncate text-xs text-zinc-600">
                      Posledná zmena: {formatDate(g.lastChange)}
                    </div>
                  </div>
                  <StateBadge state={g.syncState} />
                </div>
              ))}
            </Card>
          )}
        </section>

        <section>
          <h2 className="mb-3 text-sm font-semibold text-zinc-300">
            Nedávna aktivita
          </h2>
          {activity.length === 0 ? (
            <Card className="p-5 text-sm text-zinc-500">
              Zatiaľ žiadna aktivita v tejto relácii.
            </Card>
          ) : (
            <Card className="divide-y divide-zinc-800">
              {activity.map((item) => {
                const s = STATUS_ICON[item.progress.status];
                return (
                  <div key={item.id} className="flex items-start gap-3 px-4 py-2.5">
                    <s.Icon className={`mt-0.5 h-4 w-4 shrink-0 ${s.cls}`} />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm text-zinc-200">
                        {resolveName(item.progress)}
                        <span className="text-zinc-500">
                          {" · "}
                          {item.progress.targetType === "folder"
                            ? "priečinok"
                            : "hra"}
                        </span>
                      </div>
                      <div className="truncate text-xs text-zinc-500">
                        {item.progress.message ?? s.label}
                      </div>
                    </div>
                    <span className="shrink-0 font-mono text-xs text-zinc-600">
                      {formatTime(item.at)}
                    </span>
                  </div>
                );
              })}
            </Card>
          )}
        </section>
      </div>
    </div>
  );
}
