import type { SyncState } from "../types";

const MAP: Record<SyncState, { label: string; cls: string; dot: string }> = {
  idle: {
    label: "Nečinné",
    cls: "border-zinc-700 bg-zinc-800/80 text-zinc-400",
    dot: "bg-zinc-500",
  },
  changed: {
    label: "Zmenené",
    cls: "border-amber-500/30 bg-amber-500/10 text-amber-400",
    dot: "bg-amber-400",
  },
  syncing: {
    label: "Synchronizuje sa",
    cls: "border-sky-500/30 bg-sky-500/10 text-sky-400",
    dot: "bg-sky-400 animate-pulse",
  },
  synced: {
    label: "Synchronizované",
    cls: "border-emerald-500/30 bg-emerald-500/10 text-emerald-400",
    dot: "bg-emerald-400",
  },
  error: {
    label: "Chyba",
    cls: "border-red-500/30 bg-red-500/10 text-red-400",
    dot: "bg-red-400",
  },
};

export default function StateBadge({ state }: { state: SyncState }) {
  const s = MAP[state];
  return (
    <span
      className={`inline-flex items-center gap-1.5 whitespace-nowrap rounded-full border px-2.5 py-0.5 text-xs font-medium ${s.cls}`}
    >
      <span className={`h-1.5 w-1.5 rounded-full ${s.dot}`} />
      {s.label}
    </span>
  );
}
