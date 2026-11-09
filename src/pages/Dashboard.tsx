import type { ActivityItem, Page } from "../App";
import {
  AlertIcon,
  CheckIcon,
  CloudIcon,
  FolderIcon,
  GamepadIcon,
  HistoryIcon,
  PlusIcon,
  SpinnerIcon,
} from "../components/icons";
import { useApp } from "../context";
import { baseName, formatDate, formatTime, providerLabel } from "../format";
import type { Provider, SyncState } from "../types";
import "./Dashboard.css";

const PROVIDERS: Provider[] = ["gdrive", "ulozto", "customCloud"];
const STATE_LABELS: Record<SyncState, string> = {
  synced: "Synchronizované",
  changed: "Čakajúce",
  syncing: "Prebieha",
  error: "Chyba",
  idle: "Nečinné",
};
const STATE_ORDER: SyncState[] = ["synced", "changed", "error", "syncing", "idle"];
const ACTIVITY_STATUS = {
  done: { Icon: CheckIcon, label: "Dokončené" },
  error: { Icon: AlertIcon, label: "Chyba pri synchronizácii" },
  uploading: { Icon: SpinnerIcon, label: "Prebieha prenos" },
};

function ArrowIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M5 12h14m-5-5 5 5-5 5" />
    </svg>
  );
}

function getSummary(total: number, errors: number, pending: number, syncing: number, synced: number) {
  if (total === 0) return {
    tone: "neutral", label: "PRIPRAVENÉ NA ŠTART", title: "Každá záloha sa počíta.",
    description: "Pridaj svoju prvú hru alebo priečinok. Stav záloh a všetky prenosy potom nájdeš tu.",
  };
  if (errors > 0) return {
    tone: "error", label: "VYŽADUJE POZORNOSŤ", title: "Daj zálohy opäť do poriadku.",
    description: "Pri niektorých položkách sa objavil problém. Podrobnosti nájdeš v zozname nižšie.",
  };
  if (syncing > 0) return {
    tone: "syncing", label: "PREBIEHA SYNCHRONIZÁCIA", title: "Tvoje zálohy sú v pohybe.",
    description: "Aplikácia práve spracúva zálohy. Priebeh môžeš sledovať v nedávnej aktivite.",
  };
  if (pending > 0) return {
    tone: "pending", label: "NOVÉ ZMENY", title: "Nový postup. Čas na zálohu.",
    description: "Niektoré položky obsahujú zmeny čakajúce na synchronizáciu s cloudom.",
  };
  if (synced === total) return {
    tone: "success", label: "SYNCHRONIZOVANÉ", title: "Všetko na svojom mieste.",
    description: "Všetky sledované položky majú stav synchronizované. Prehľad posledných prenosov máš vždy poruke.",
  };
  return {
    tone: "neutral", label: "TVOJE ZÁLOHY", title: "Priestor pre tvoje dáta.",
    description: "Hry aj priečinky máš na jednom mieste. Časť položiek zatiaľ čaká na synchronizáciu.",
  };
}

export default function Dashboard({
  activity,
  onNavigate,
}: {
  activity: ActivityItem[];
  onNavigate: (page: Page) => void;
}) {
  const { config } = useApp();
  const { watchedFolders, games, providers } = config;

  const entries = [
    ...games.map((game) => ({
      id: `game:${game.id}`,
      name: game.name,
      page: "games" as const,
      provider: game.provider,
      state: game.local.watchError ? "error" as const : game.syncState,
      error: game.local.watchError || game.lastError,
      lastSync: game.lastUpload,
    })),
    ...watchedFolders.map((folder) => ({
      id: `folder:${folder.id}`,
      name: baseName(folder.path),
      page: "folders" as const,
      provider: folder.provider,
      state: folder.syncState,
      error: folder.lastError,
      lastSync: folder.lastSync,
    })),
  ];
  const counts: Record<SyncState, number> = { idle: 0, changed: 0, syncing: 0, synced: 0, error: 0 };
  for (const entry of entries) counts[entry.state] += 1;

  const attention = entries
    .filter((entry) => entry.state === "error" || entry.state === "changed" || entry.state === "idle")
    .sort((a, b) => {
      const priority = { error: 0, changed: 1, idle: 2, syncing: 3, synced: 4 };
      return priority[a.state] - priority[b.state];
    });
  const lastSyncMs = Math.max(0, ...entries
    .map((entry) => entry.lastSync ? Date.parse(entry.lastSync) : 0)
    .filter(Number.isFinite));
  const connectedCount = PROVIDERS.filter((provider) => providers[provider].connected && !providers[provider].error).length;
  const summary = getSummary(entries.length, counts.error, counts.changed, counts.syncing, counts.synced);
  const summaryPage = attention[0]?.page ?? (games.length || !watchedFolders.length ? "games" : "folders");
  const today = new Date().toLocaleDateString("sk-SK", { weekday: "long", day: "numeric", month: "long" });

  function activityName(item: ActivityItem) {
    if (item.progress.targetType === "game") {
      return games.find((game) => game.id === item.progress.id)?.name ?? "Odstránená hra";
    }
    const folder = watchedFolders.find((folder) => folder.id === item.progress.id);
    return folder ? baseName(folder.path) : "Odstránený priečinok";
  }

  return (
    <div className="dashboard">
      <header className="dashboard__header">
        <div>
          <span className="dashboard__eyebrow">BACKUP SYNC / TVOJ PRIESTOR</span>
          <h1>Prehľad</h1>
          <p>Všetko dôležité o tvojich zálohách na jednom mieste.</p>
        </div>
        <div className="dashboard__date"><HistoryIcon />{today}</div>
      </header>

      <section className="dashboard__hero" data-tone={summary.tone} aria-labelledby="backup-summary-title">
        <div className="dashboard__hero-copy">
          <span className="dashboard__status">
            {counts.error > 0 ? <AlertIcon /> : counts.syncing > 0 ? <SpinnerIcon /> : <CloudIcon />}
            {summary.label}
          </span>
          <h2 id="backup-summary-title">{summary.title}</h2>
          <p>{summary.description}</p>
          <button className="dashboard__primary" onClick={() => onNavigate(summaryPage)}>
            {summaryPage === "games" ? "Otvoriť hernú knižnicu" : "Otvoriť priečinky"}
            <ArrowIcon />
          </button>
        </div>
        <div className="dashboard__ring-wrap">
          <div className="dashboard__ring" role="img" aria-label={`${counts.synced} z ${entries.length} položiek má stav synchronizované`}>
            <svg viewBox="0 0 160 160" aria-hidden="true">
              <circle className="dashboard__ring-track" cx="80" cy="80" r="72" fill="none" strokeWidth="5" />
              <circle className="dashboard__ring-value" cx="80" cy="80" r="72" fill="none" strokeWidth="5" strokeLinecap="round" pathLength="100" strokeDasharray={`${entries.length ? counts.synced / entries.length * 100 : 0} 100`} />
            </svg>
            <div className="dashboard__ring-core" aria-hidden="true">
              <CloudIcon />
              <strong>{counts.synced}<span> / {entries.length}</span></strong>
              <small>Synchronizované</small>
            </div>
          </div>
        </div>
        <div className="dashboard__hero-footer">
          <div className="dashboard__legend" aria-label="Stavy sledovaných položiek">
            {STATE_ORDER.filter((state) => counts[state] > 0).map((state) => (
              <span key={state}><i className={`dashboard__dot dashboard__dot--${state}`} /><b>{counts[state]}</b>{STATE_LABELS[state]}</span>
            ))}
            {entries.length === 0 && <span>Zatiaľ žiadne sledované položky</span>}
          </div>
          <span className="dashboard__last-sync"><HistoryIcon />Posledná synchronizácia · {lastSyncMs ? formatDate(new Date(lastSyncMs).toISOString()) : "zatiaľ žiadna"}</span>
        </div>
      </section>

      <div className="dashboard__shortcuts">
        <button className="dashboard__shortcut" onClick={() => onNavigate("games")}>
          <span className="dashboard__shortcut-label">Herná knižnica</span>
          <span className="dashboard__shortcut-icon"><GamepadIcon /></span>
          <strong>{games.length}</strong>
          <span className="dashboard__shortcut-bottom">Save súbory a ich zálohy<ArrowIcon /></span>
        </button>
        <button className="dashboard__shortcut" onClick={() => onNavigate("folders")}>
          <span className="dashboard__shortcut-label">Priečinky</span>
          <span className="dashboard__shortcut-icon"><FolderIcon /></span>
          <strong>{watchedFolders.length}</strong>
          <span className="dashboard__shortcut-bottom">Tvoje sledované priečinky<ArrowIcon /></span>
        </button>
        <button className="dashboard__shortcut" onClick={() => onNavigate("settings")}>
          <span className="dashboard__shortcut-label">Pripojené cloudy</span>
          <span className="dashboard__shortcut-icon"><CloudIcon /></span>
          <strong>{connectedCount}<small> / {PROVIDERS.length}</small></strong>
          <span className="dashboard__shortcut-bottom">Spravovať pripojenia<ArrowIcon /></span>
        </button>
      </div>

      <div className="dashboard__columns">
        <section className="dashboard__panel" aria-labelledby="dashboard-attention-title">
          <div className="dashboard__panel-header">
            <div>
              <h2 id="dashboard-attention-title"><AlertIcon />Vyžaduje pozornosť</h2>
              <p>Chyby a položky čakajúce na synchronizáciu</p>
            </div>
            <span className="dashboard__count">{attention.length}</span>
          </div>
          {attention.length > 0 ? (
            <div className="dashboard__list">
              {attention.map((entry) => (
                <button className="dashboard__task" data-state={entry.state} key={entry.id} onClick={() => onNavigate(entry.page)} title={`Otvoriť ${entry.page === "games" ? "hry" : "priečinky"}`}>
                  <span className="dashboard__task-icon">{entry.page === "games" ? <GamepadIcon /> : <FolderIcon />}</span>
                  <span className="dashboard__task-copy">
                    <strong>{entry.name}</strong>
                    <small>{entry.error || (entry.state === "changed" ? "Zmeny čakajú na cloud" : entry.state === "error" ? "Synchronizácia zlyhala" : "Synchronizácia zatiaľ nepotvrdená")}</small>
                    <span className="dashboard__task-meta">{providerLabel(entry.provider)}</span>
                  </span>
                  <ArrowIcon />
                </button>
              ))}
            </div>
          ) : (
            <div className="dashboard__empty">
              <span className="dashboard__empty-icon">{entries.length ? <CheckIcon /> : <PlusIcon />}</span>
              <strong>{entries.length ? "Žiadne položky na vybavenie" : "Tvoj prehľad čaká na prvú zálohu"}</strong>
              <p>{entries.length ? "Keď sa objaví zmena alebo problém, nájdeš ho tu." : "Začni hrou alebo vyber priečinok, ktorý chceš zálohovať."}</p>
              {!entries.length && <button className="dashboard__text-button" onClick={() => onNavigate("folders")}>Otvoriť priečinky<ArrowIcon /></button>}
            </div>
          )}
        </section>

        <section className="dashboard__panel" aria-labelledby="dashboard-activity-title">
          <div className="dashboard__panel-header">
            <div>
              <h2 id="dashboard-activity-title"><HistoryIcon />Nedávna aktivita</h2>
              <p>Udalosti od spustenia aplikácie</p>
            </div>
          </div>
          {activity.length > 0 ? (
            <ol className="dashboard__timeline">
              {activity.map((item) => {
                const status = ACTIVITY_STATUS[item.progress.status];
                return (
                  <li className="dashboard__event" data-status={item.progress.status} key={item.id}>
                    <span className="dashboard__event-icon"><status.Icon /></span>
                    <div>
                      <strong>{activityName(item)}</strong>
                      <p>{item.progress.message || status.label}</p>
                    </div>
                    <time dateTime={new Date(item.at).toISOString()} title={formatDate(new Date(item.at).toISOString())}>{formatTime(item.at)}</time>
                  </li>
                );
              })}
            </ol>
          ) : (
            <div className="dashboard__empty">
              <span className="dashboard__empty-icon"><HistoryIcon /></span>
              <strong>Pripravené na ďalší prenos</strong>
              <p>Tu uvidíš priebeh zálohovania, dokončené operácie aj prípadné chyby.</p>
            </div>
          )}
        </section>
      </div>

      <section className="dashboard__panel dashboard__clouds" aria-labelledby="dashboard-clouds-title">
        <div className="dashboard__panel-header">
          <h2 id="dashboard-clouds-title"><CloudIcon />Cloudové úložiská</h2>
          <button className="dashboard__text-button" onClick={() => onNavigate("settings")}>Nastavenia<ArrowIcon /></button>
        </div>
        <div className="dashboard__providers">
          {PROVIDERS.map((provider) => {
            const status = providers[provider];
            const state = status.error ? "error" : status.connected ? "connected" : "idle";
            return (
              <div className="dashboard__provider" key={provider}>
                <CloudIcon />
                <div className="dashboard__provider-copy">
                  <strong>{providerLabel(provider)}</strong>
                  <p title={status.error || status.account || undefined}>{status.error ? "Chyba pripojenia" : status.connected ? "Pripojené" : "Nepripojené"}</p>
                </div>
                <span className={`dashboard__dot dashboard__dot--${state}`} aria-hidden="true" />
              </div>
            );
          })}
        </div>
      </section>
    </div>
  );
}
