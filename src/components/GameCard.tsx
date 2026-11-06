import { useState, type CSSProperties } from "react";
import { useGameArtwork } from "../artwork";
import type { Game, GameCloudStatus } from "../types";
import { formatDate, providerLabel } from "../format";
import { Button } from "./ui";
import StateBadge from "./StateBadge";
import {
  CloudIcon,
  DownloadIcon,
  FolderIcon,
  GamepadIcon,
  HistoryIcon,
  SettingsIcon,
  TrashIcon,
  UploadIcon,
} from "./icons";
import "./GameCard.css";

interface GameCardProps {
  game: Game;
  cloudStatus?: GameCloudStatus;
  busy: string | null;
  globalAutoUpload: boolean;
  onOpenFolder: () => void;
  onEdit: () => void;
  onRemove: () => void;
  onVersions: () => void;
  onBackup: () => void;
  onUpload: () => void;
  onAutoUpload: (value: boolean | null) => void;
}

// A stable visual identity, including for games without cover artwork.
function gameHue(id: string) {
  let hash = 0;
  for (const character of id) hash = (hash * 31 + character.charCodeAt(0)) | 0;
  return [252, 174, 29, 211, 324, 85][Math.abs(hash) % 6];
}

export default function GameCard({
  game,
  cloudStatus,
  busy,
  globalAutoUpload,
  onOpenFolder,
  onEdit,
  onRemove,
  onVersions,
  onBackup,
  onUpload,
  onAutoUpload,
}: GameCardProps) {
  const { src } = useGameArtwork(game, true);
  const [failedImage, setFailedImage] = useState<string | null>(null);
  const showImage = src && src !== failedImage;
  const imageSource = game.artwork?.source === "steam" ? "Steam" : "SteamGridDB";
  const syncing = game.syncState === "syncing";
  const uploading = busy === `upload:${game.id}` || syncing;
  const waitingBackup = Boolean(
    game.local.backupHash &&
      game.local.observedHash &&
      game.local.backupHash !== game.local.observedHash,
  );
  const unconfirmedCloud = Boolean(
    game.local.observedHash &&
      game.local.uploadedHash !== game.local.observedHash,
  );
  const autoUpload = game.autoUpload ?? globalAutoUpload;
  const titleId = `game-title-${game.id}`;

  return (
    <article
      className="game-card"
      data-state={game.syncState}
      style={{ "--game-hue": gameHue(game.id) } as CSSProperties}
      aria-labelledby={titleId}
    >
      <div className="game-card__cover">
        {showImage ? (
          <div
            className="game-card__art game-card__art--photo"
            aria-hidden="true"
          >
            <img src={src} alt="" onError={() => setFailedImage(src)} />
          </div>
        ) : (
          <div className="game-card__art" aria-hidden="true">
            <div className="game-card__orbit" />
            <GamepadIcon className="game-card__art-icon" />
          </div>
        )}
        <div className="game-card__topline">
          <span className="game-card__provider">
            <CloudIcon className="h-3.5 w-3.5" />
            {providerLabel(game.provider)}
          </span>
          <div className="game-card__tools">
            <Button
              variant="ghost"
              className="game-card__icon-button"
              onClick={onEdit}
              aria-label={`Upraviť hru ${game.name}`}
              title="Upraviť hru"
            >
              <SettingsIcon className="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              className="game-card__icon-button game-card__remove"
              onClick={onRemove}
              aria-label={`Odstrániť hru ${game.name}`}
              title="Odstrániť hru"
            >
              <TrashIcon className="h-4 w-4" />
            </Button>
          </div>
        </div>
        <div className="game-card__identity">
          <span
            className="game-card__eyebrow"
            title={
              showImage
                ? `Obrázok zo ${imageSource} · ${game.artwork?.author || imageSource}`
                : undefined
            }
          >
            {showImage
              ? `${imageSource}${game.artwork?.author ? ` · ${game.artwork.author}` : ""}`
              : "HERNÉ SAVE SÚBORY"}
          </span>
          <h2 id={titleId} className="game-card__title" title={game.name}>
            {game.name}
          </h2>
          <StateBadge state={game.syncState} />
        </div>
      </div>

      <div className="game-card__content">
        {cloudStatus?.updateAvailable && (
          <button
            className="game-card__update"
            type="button"
            onClick={onVersions}
            title={`Cloud má novšiu verziu z ${formatDate(cloudStatus.latestAt)}. Otvor Verzie a obnov ju.`}
          >
            <DownloadIcon className="h-4 w-4 shrink-0" />
            <span>Novšia verzia na cloude</span>
            <span aria-hidden="true">↗</span>
          </button>
        )}

        <dl className="game-card__backups">
          <div>
            <dt>
              <FolderIcon className="h-3.5 w-3.5" /> Lokálna záloha
            </dt>
            <dd>
              {game.local.lastBackup
                ? formatDate(game.local.lastBackup)
                : "Zatiaľ bez zálohy"}
            </dd>
            <span
              className={
                waitingBackup ? "game-card__warning" : "game-card__detail"
              }
            >
              {waitingBackup ? "Čaká na novú zálohu" : "ZIP na tomto počítači"}
            </span>
          </div>
          <div>
            <dt>
              <CloudIcon className="h-3.5 w-3.5" /> Cloudová záloha
            </dt>
            <dd>
              {game.lastUpload
                ? formatDate(game.lastUpload)
                : "Zatiaľ nenahrané"}
            </dd>
            <span
              className={
                unconfirmedCloud ? "game-card__warning" : "game-card__detail"
              }
            >
              {unconfirmedCloud
                ? "Aktuálny save neoverený"
                : providerLabel(game.provider)}
            </span>
          </div>
        </dl>

        <button
          type="button"
          className="game-card__path"
          onClick={onOpenFolder}
          aria-label={`Otvoriť priečinok hry ${game.name}`}
          title={`Save: ${game.savePath}\nLokálne ZIP zálohy: ${game.local.backupFile ?? game.backupPath}`}
        >
          <FolderIcon className="h-4 w-4 shrink-0" />
          <span>{game.savePath}</span>
          <span className="game-card__path-arrow" aria-hidden="true">
            ↗
          </span>
        </button>

        {game.local.watchError && (
          <p className="game-card__notice">{game.local.watchError}</p>
        )}
        {game.lastCloudVersionId && !game.local.uploadedHash && (
          <p className="game-card__notice">
            Obsah starej cloudovej verzie ešte nie je overený. Skontroluj save a
            použi „Nahrať na cloud“.
          </p>
        )}
        {game.lastError && <p className="game-card__error">{game.lastError}</p>}

        <div className="game-card__automation">
          <label htmlFor={`auto-upload-${game.id}`}>
            <span
              className={`game-card__auto-dot ${autoUpload ? "is-on" : ""}`}
              aria-hidden="true"
            />
            Auto nahrávanie
          </label>
          <select
            id={`auto-upload-${game.id}`}
            aria-label={`Automatické nahrávanie pre ${game.name}`}
            value={
              game.autoUpload === null
                ? "inherit"
                : game.autoUpload
                  ? "on"
                  : "off"
            }
            disabled={busy !== null}
            onChange={(event) =>
              onAutoUpload(
                event.currentTarget.value === "inherit"
                  ? null
                  : event.currentTarget.value === "on",
              )
            }
          >
            <option value="inherit">
              Globálne · {globalAutoUpload ? "zapnuté" : "vypnuté"}
            </option>
            <option value="on">Zapnuté</option>
            <option value="off">Vypnuté</option>
          </select>
        </div>

        <div className="game-card__actions">
          <Button
            variant="primary"
            className="game-card__upload"
            loading={uploading}
            disabled={busy !== null || syncing}
            onClick={onUpload}
          >
            {!uploading && <UploadIcon className="h-4 w-4" />}
            {game.syncState === "error"
              ? "Skúsiť znova"
              : uploading
                ? "Nahráva sa…"
                : game.syncState === "synced"
                  ? "Nahrať znova"
                  : "Nahrať na cloud"}
          </Button>
          <div className="game-card__secondary-actions">
            <Button
              variant="ghost"
              onClick={onBackup}
              loading={busy === `local:${game.id}`}
              disabled={busy !== null || syncing}
              title="Vytvoriť okamžitú lokálnu ZIP zálohu"
            >
              {busy !== `local:${game.id}` && (
                <FolderIcon className="h-4 w-4" />
              )}
              Zálohovať lokálne
            </Button>
            <Button
              variant="ghost"
              onClick={onVersions}
              title="História verzií"
            >
              <HistoryIcon className="h-4 w-4" /> Verzie
            </Button>
          </div>
        </div>
      </div>
    </article>
  );
}
