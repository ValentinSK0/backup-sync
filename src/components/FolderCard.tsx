import type { CSSProperties } from "react";
import type { WatchedFolder } from "../types";
import { baseName, formatDate, providerLabel } from "../format";
import { Button } from "./ui";
import StateBadge from "./StateBadge";
import { CloudIcon, DownloadIcon, FolderIcon, HistoryIcon, TrashIcon, UploadIcon } from "./icons";
import "./GameCard.css";
import "./FolderCard.css";

interface FolderCardProps {
  folder: WatchedFolder;
  busy: string | null;
  globalAutoBackup: boolean;
  onOpenFolder: () => void;
  onRemove: () => void;
  onRestore: () => void;
  onBackup: () => void;
  onAutoBackup: (value: boolean | null) => void;
}

export default function FolderCard({
  folder,
  busy,
  globalAutoBackup,
  onOpenFolder,
  onRemove,
  onRestore,
  onBackup,
  onAutoBackup,
}: FolderCardProps) {
  const name = baseName(folder.path);
  const syncing = folder.syncState === "syncing";
  const downloading = busy === `download:${folder.id}`;
  const uploading = busy === `backup:${folder.id}` || (syncing && !downloading);
  const autoBackup = folder.autoBackup ?? globalAutoBackup;
  const downloadFailed = folder.syncState === "error" && folder.lastErrorAction === "download";
  const uploadFailed = folder.syncState === "error" && folder.lastErrorAction === "upload";
  const titleId = `folder-title-${folder.id}`;

  return (
    <article
      className="game-card folder-card"
      data-state={folder.syncState}
      style={{ "--game-hue": 252 } as CSSProperties}
      aria-labelledby={titleId}
    >
      <div className="folder-card__header">
        <div className="game-card__topline">
          <span className="game-card__provider">
            <CloudIcon className="h-3.5 w-3.5" />
            {providerLabel(folder.provider)}
          </span>
          <Button
            variant="ghost"
            className="game-card__icon-button game-card__remove"
            onClick={onRemove}
            disabled={busy !== null || syncing}
            aria-label={`Odstrániť priečinok ${name}`}
            title="Odstrániť zo sledovania"
          >
            <TrashIcon className="h-4 w-4" />
          </Button>
        </div>
        <div className="folder-card__identity">
          <div className="folder-card__icon" aria-hidden="true">
            <FolderIcon className="h-7 w-7" />
          </div>
          <div className="min-w-0 flex-1">
            <span className="game-card__eyebrow">SLEDOVANÝ PRIEČINOK</span>
            <h2 id={titleId} className="game-card__title" title={name}>{name}</h2>
            <StateBadge state={folder.syncState} />
          </div>
        </div>
      </div>

      <div className="game-card__content">
        <dl className="game-card__backups">
          <div>
            <dt><HistoryIcon className="h-3.5 w-3.5" /> Posledná záloha</dt>
            <dd>{folder.lastSync ? formatDate(folder.lastSync) : "Zatiaľ bez zálohy"}</dd>
            <span className={folder.syncState === "changed" ? "game-card__warning" : "game-card__detail"}>
              {folder.syncState === "changed"
                ? "Čaká na nahratie zmien"
                : folder.lastSync ? "Záloha na cloude" : "Čaká na prvé zálohovanie"}
            </span>
          </div>
          <div>
            <dt><CloudIcon className="h-3.5 w-3.5" /> Režim zálohovania</dt>
            <dd>{autoBackup ? "Automatický" : "Manuálny"}</dd>
            <span className="game-card__detail">
              {folder.autoBackup === null ? "Podľa globálnych nastavení" : "Vlastné nastavenie"}
            </span>
          </div>
        </dl>

        <button
          type="button"
          className="game-card__path"
          onClick={onOpenFolder}
          aria-label={`Otvoriť priečinok ${name}`}
          title={folder.path}
        >
          <FolderIcon className="h-4 w-4 shrink-0" />
          <span>{folder.path}</span>
          <span className="game-card__path-arrow" aria-hidden="true">↗</span>
        </button>

        {folder.lastError && <p className="game-card__error">{folder.lastError}</p>}

        <div className="game-card__automation">
          <label htmlFor={`auto-backup-${folder.id}`}>
            <span className={`game-card__auto-dot ${autoBackup ? "is-on" : ""}`} aria-hidden="true" />
            Auto zálohovanie
          </label>
          <select
            id={`auto-backup-${folder.id}`}
            aria-label={`Automatické zálohovanie pre ${name}`}
            value={folder.autoBackup === null ? "inherit" : folder.autoBackup ? "on" : "off"}
            disabled={busy !== null}
            onChange={(event) => onAutoBackup(
              event.currentTarget.value === "inherit" ? null : event.currentTarget.value === "on",
            )}
          >
            <option value="inherit">Globálne · {globalAutoBackup ? "zapnuté" : "vypnuté"}</option>
            <option value="on">Zapnuté</option>
            <option value="off">Vypnuté</option>
          </select>
        </div>

        <div className="game-card__actions">
          <Button
            className="game-card__upload"
            variant="primary"
            loading={uploading}
            disabled={busy !== null || syncing}
            onClick={onBackup}
          >
            {!uploading && <UploadIcon className="h-4 w-4" />}
            {uploadFailed ? "Zálohovať znova" : uploading ? "Prebieha zálohovanie…" : "Zálohovať teraz"}
          </Button>
          <div className="game-card__secondary-actions">
            <Button
              variant={downloadFailed ? "primary" : "ghost"}
              loading={downloading}
              disabled={busy !== null || syncing}
              onClick={onRestore}
              title="Prepísať lokálne súbory verziou z cloudu"
            >
              {!downloading && <DownloadIcon className="h-4 w-4" />}
              {downloading ? "Sťahuje sa…" : downloadFailed ? "Stiahnuť znova" : "Stiahnuť z cloudu"}
            </Button>
          </div>
        </div>
      </div>
    </article>
  );
}
