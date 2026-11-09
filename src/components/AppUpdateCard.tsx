import { useApp } from "../context";
import { Button, Card } from "./ui";

export default function AppUpdateCard() {
  const { updater } = useApp();
  const { currentVersion, info, phase, progress, error, check, download, install } = updater;
  const update = info?.update;
  const busy = phase !== "idle";
  const percentage = progress.total
    ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
    : null;

  let status = "Kontrola prebieha pri spustení a potom každých 6 hodín.";
  if (phase === "checking") status = "Kontrolujem GitHub Releases…";
  else if (phase === "downloading") status = "Sťahujem aktualizáciu a overujem podpis…";
  else if (phase === "installing") status = "Spúšťam inštaláciu. Aplikácia sa zatvorí a znovu spustí.";
  else if (update?.downloaded) status = "Aktualizácia je stiahnutá a jej podpis je overený.";
  else if (update) status = `Nová verzia ${update.version} je pripravená na stiahnutie.`;
  else if (info?.feedAvailable) status = "Máte najnovšiu verziu aplikácie.";
  else if (info) status = "GitHub zatiaľ neposkytuje aktualizačný balík. Skúste kontrolu neskôr.";

  return (
    <Card className="overflow-hidden border-indigo-500/25">
      <div className="bg-gradient-to-br from-indigo-500/10 via-zinc-900 to-zinc-900 p-5">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h2 className="text-sm font-semibold text-zinc-100">Aktualizácie aplikácie</h2>
            <p className="mt-1 text-xs text-zinc-400">
              Backup Sync {currentVersion || "…"} · GitHub Releases
            </p>
          </div>
          {update && (
            <span className="rounded-full border border-indigo-400/30 bg-indigo-400/10 px-2.5 py-1 text-xs font-medium text-indigo-200">
              Verzia {update.version}
            </span>
          )}
        </div>

        <p role="status" aria-live="polite" className="mt-4 text-sm text-zinc-300">{status}</p>
        {phase === "downloading" && (
          <div className="mt-3">
            <div
              role="progressbar"
              aria-label="Sťahovanie aktualizácie"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={percentage ?? undefined}
              className="h-1.5 overflow-hidden rounded-full bg-zinc-800"
            >
              <div
                className={`h-full rounded-full bg-indigo-400 transition-all ${percentage === null ? "animate-pulse" : ""}`}
                style={{ width: percentage === null ? "35%" : `${percentage}%` }}
              />
            </div>
            <p className="mt-2 text-xs tabular-nums text-zinc-400">
              {(progress.downloaded / 1_000_000).toFixed(1)} MB
              {progress.total ? ` / ${(progress.total / 1_000_000).toFixed(1)} MB (${percentage} %)` : ""}
            </p>
          </div>
        )}
        {error && <p role="alert" className="mt-3 break-words text-xs text-red-400">{error}</p>}

        {update?.notes && (
          <details className="mt-4 rounded-lg border border-zinc-800 bg-zinc-950/40 p-3">
            <summary className="cursor-pointer text-xs font-medium text-zinc-300">Čo je nové</summary>
            <p className="mt-3 max-h-56 overflow-y-auto whitespace-pre-wrap break-words text-xs leading-relaxed text-zinc-400">
              {update.notes}
            </p>
          </details>
        )}

        <div className="mt-4 flex flex-wrap gap-2">
          <Button onClick={() => void check()} loading={phase === "checking"} disabled={busy || update?.downloaded}>
            Skontrolovať aktualizácie
          </Button>
          {update && !update.downloaded && (
            <Button variant="primary" onClick={() => void download()} loading={phase === "downloading"} disabled={busy}>
              Stiahnuť aktualizáciu
            </Button>
          )}
          {update?.downloaded && (
            <Button variant="primary" onClick={() => void install()} loading={phase === "installing"} disabled={busy}>
              Nainštalovať a reštartovať
            </Button>
          )}
        </div>
        <p className="mt-3 text-xs leading-relaxed text-zinc-500">
          Inštaláciu spustíte tlačidlom po dokončení zálohovania. Pri inštalácii sa aplikácia reštartuje.
        </p>
      </div>
    </Card>
  );
}
