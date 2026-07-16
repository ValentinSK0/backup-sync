import { useEffect, useState } from "react";
import {
  addGame,
  listGameVersions,
  pickFolder,
  removeGame,
  restoreGameVersion,
  setGameAutoUpload,
  uploadGameSave,
} from "../api";
import { ConfirmDialog, Modal } from "../components/Modal";
import StateBadge from "../components/StateBadge";
import {
  CloudIcon,
  FolderIcon,
  GamepadIcon,
  HistoryIcon,
  PlusIcon,
  SpinnerIcon,
  TrashIcon,
  UploadIcon,
} from "../components/icons";
import {
  Button,
  Card,
  EmptyState,
  PageHeader,
  inputCls,
  labelCls,
} from "../components/ui";
import { useApp } from "../context";
import { errMsg, formatBytes, formatDate, providerLabel } from "../format";
import { useConfigAction } from "../hooks";
import type { Game, Provider, VersionInfo } from "../types";

type AutoUploadOption = "inherit" | "on" | "off";

function autoUploadToOption(value: boolean | null): AutoUploadOption {
  if (value === null) return "inherit";
  return value ? "on" : "off";
}

function optionToAutoUpload(option: AutoUploadOption): boolean | null {
  if (option === "inherit") return null;
  return option === "on";
}

function VersionsModal({ game, onClose }: { game: Game; onClose: () => void }) {
  const { toast } = useApp();
  const [versions, setVersions] = useState<VersionInfo[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [toRestore, setToRestore] = useState<VersionInfo | null>(null);
  const [restoring, setRestoring] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setVersions(null);
    setLoadError(null);
    listGameVersions(game.id)
      .then((v) => {
        if (!cancelled) setVersions(v);
      })
      .catch((e) => {
        if (!cancelled) setLoadError(errMsg(e));
      });
    return () => {
      cancelled = true;
    };
  }, [game.id, reloadKey]);

  async function handleRestore() {
    if (!toRestore) return;
    setRestoring(true);
    try {
      await restoreGameVersion(game.id, toRestore.id);
      toast("success", `Verzia „${toRestore.name}“ bola obnovená.`);
      setToRestore(null);
    } catch (e) {
      toast("error", errMsg(e));
    } finally {
      setRestoring(false);
    }
  }

  return (
    <>
      <Modal title={`Verzie — ${game.name}`} onClose={onClose} wide>
        {loadError ? (
          <div className="rounded-lg border border-red-500/20 bg-red-500/10 p-3">
            <p className="break-words text-sm text-red-400">{loadError}</p>
            <Button
              className="mt-3"
              variant="secondary"
              onClick={() => setReloadKey((value) => value + 1)}
            >
              Skúsiť znova
            </Button>
          </div>
        ) : versions === null ? (
          <div className="flex items-center justify-center gap-2 py-8 text-sm text-zinc-500">
            <SpinnerIcon className="h-4 w-4" />
            Načítavajú sa verzie…
          </div>
        ) : versions.length === 0 ? (
          <p className="py-4 text-center text-sm text-zinc-500">
            Na cloude zatiaľ nie sú žiadne verzie.
          </p>
        ) : (
          <div className="divide-y divide-zinc-800">
            {versions.map((v) => (
              <div key={v.id} className="flex items-center gap-3 py-3">
                <HistoryIcon className="h-4.5 w-4.5 shrink-0 text-zinc-500" />
                <div className="min-w-0 flex-1">
                  <div
                    className="truncate font-mono text-sm text-zinc-200"
                    title={v.name}
                  >
                    {v.name}
                  </div>
                  <div className="text-xs text-zinc-500">
                    {formatDate(v.createdAt)}
                    <span className="text-zinc-700"> · </span>
                    {formatBytes(v.sizeBytes)}
                  </div>
                </div>
                <Button
                  variant="secondary"
                  onClick={() => setToRestore(v)}
                  disabled={restoring}
                >
                  Obnoviť
                </Button>
              </div>
            ))}
          </div>
        )}
      </Modal>

      {toRestore && (
        <ConfirmDialog
          title="Obnoviť verziu"
          message={`Naozaj obnoviť „${toRestore.name}“ z ${formatDate(toRestore.createdAt)}? Aktuálne save súbory hry budú prepísané.`}
          confirmLabel="Obnoviť"
          loading={restoring}
          onConfirm={handleRestore}
          onCancel={() => setToRestore(null)}
        />
      )}
    </>
  );
}

function AddGameModal({ onClose }: { onClose: () => void }) {
  const { toast } = useApp();
  const { busy, run } = useConfigAction();
  const [name, setName] = useState("");
  const [savePath, setSavePath] = useState("");
  const [provider, setProvider] = useState<Provider>("gdrive");
  const [picking, setPicking] = useState(false);

  async function handlePick() {
    setPicking(true);
    try {
      const path = await pickFolder();
      if (path) setSavePath(path);
    } finally {
      setPicking(false);
    }
  }

  async function handleSubmit() {
    const trimmed = name.trim();
    if (!trimmed) {
      toast("error", "Zadajte názov hry.");
      return;
    }
    if (!savePath) {
      toast("error", "Vyberte priečinok so save súbormi.");
      return;
    }
    const ok = await run(
      "addGame",
      () => addGame(trimmed, savePath, provider),
      `Hra „${trimmed}“ pridaná.`,
    );
    if (ok) onClose();
  }

  return (
    <Modal title="Pridať hru" onClose={onClose}>
      <form
        className="flex flex-col gap-4"
        onSubmit={(e) => {
          e.preventDefault();
          void handleSubmit();
        }}
      >
        <div>
          <label className={labelCls} htmlFor="game-name">
            Názov hry
          </label>
          <input
            id="game-name"
            className={inputCls}
            value={name}
            onChange={(e) => setName(e.currentTarget.value)}
            placeholder="napr. Elden Ring"
            autoFocus
          />
        </div>

        <div>
          <span className={labelCls}>Priečinok so save súbormi</span>
          <div className="flex items-center gap-2">
            <div
              className={`min-w-0 flex-1 truncate rounded-lg border border-zinc-700 bg-zinc-800/60 px-3 py-2 font-mono text-xs ${savePath ? "text-zinc-200" : "text-zinc-500"}`}
              title={savePath || undefined}
            >
              {savePath || "Žiadny priečinok nevybraný"}
            </div>
            <Button variant="secondary" onClick={handlePick} loading={picking}>
              <FolderIcon className="h-4 w-4" />
              Vybrať
            </Button>
          </div>
        </div>

        <div>
          <label className={labelCls} htmlFor="game-provider">
            Cloud
          </label>
          <select
            id="game-provider"
            className={inputCls}
            value={provider}
            onChange={(e) => setProvider(e.currentTarget.value as Provider)}
          >
            <option value="gdrive">Google Drive</option>
            <option value="customCloud">Vlastný cloud</option>
          </select>
        </div>

        <div className="mt-1 flex justify-end gap-2">
          <Button variant="secondary" onClick={onClose}>
            Zrušiť
          </Button>
          <Button variant="primary" type="submit" loading={busy === "addGame"}>
            <PlusIcon className="h-4 w-4" />
            Pridať hru
          </Button>
        </div>
      </form>
    </Modal>
  );
}

export default function Games() {
  const { config } = useApp();
  const { busy, run } = useConfigAction();
  const [toRemove, setToRemove] = useState<Game | null>(null);
  const [versionsGame, setVersionsGame] = useState<Game | null>(null);
  const [addOpen, setAddOpen] = useState(false);

  const games = config.games;

  async function handleRemove() {
    if (!toRemove) return;
    const ok = await run(
      `remove:${toRemove.id}`,
      () => removeGame(toRemove.id),
      `Hra „${toRemove.name}“ odstránená.`,
    );
    if (ok) setToRemove(null);
  }

  return (
    <div>
      <PageHeader
        title="Hry"
        subtitle="Automatické zálohovanie save súborov hier na cloud."
        action={
          <Button variant="primary" onClick={() => setAddOpen(true)}>
            <PlusIcon className="h-4 w-4" />
            Pridať hru
          </Button>
        }
      />

      {games.length === 0 ? (
        <EmptyState
          icon={<GamepadIcon className="h-10 w-10" />}
          message="Žiadne hry"
          hint="Pridajte hru a vyberte priečinok, kde ukladá svoje save súbory."
        />
      ) : (
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2 2xl:grid-cols-3">
          {games.map((g) => {
            const changed = g.syncState === "changed";
            return (
              <Card
                key={g.id}
                className={`flex flex-col gap-3 p-4 transition-shadow ${
                  changed
                    ? "!border-amber-500/50 ring-2 ring-amber-500/20"
                    : ""
                }`}
              >
                <div className="flex items-start gap-3">
                  <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-indigo-600/15 text-indigo-400">
                    <GamepadIcon className="h-5 w-5" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-semibold text-zinc-100">
                      {g.name}
                    </div>
                    <div
                      className="truncate font-mono text-xs text-zinc-500"
                      title={g.savePath}
                    >
                      {g.savePath}
                    </div>
                  </div>
                  <Button
                    variant="ghost"
                    className="!px-1.5 !py-1.5 text-zinc-500 hover:!bg-red-500/10 hover:!text-red-400"
                    onClick={() => setToRemove(g)}
                    aria-label={`Odstrániť hru ${g.name}`}
                  >
                    <TrashIcon className="h-4 w-4" />
                  </Button>
                </div>

                <div className="flex flex-wrap items-center gap-2">
                  <StateBadge state={g.syncState} />
                  <span className="inline-flex items-center gap-1 text-xs text-zinc-500">
                    <CloudIcon className="h-3.5 w-3.5" />
                    {providerLabel(g.provider)}
                  </span>
                </div>

                <div className="text-xs text-zinc-500">
                  Posledné nahratie: {formatDate(g.lastUpload)}
                  {g.lastChange && (
                    <>
                      <span className="text-zinc-700"> · </span>
                      Zmena: {formatDate(g.lastChange)}
                    </>
                  )}
                </div>

                {g.lastError && (
                  <div className="break-words rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-xs text-red-400">
                    {g.lastError}
                  </div>
                )}

                <Button
                  variant={g.syncState === "synced" ? "secondary" : "primary"}
                  className="w-full"
                  loading={
                    busy === `upload:${g.id}` || g.syncState === "syncing"
                  }
                  disabled={
                    (busy !== null && busy !== `upload:${g.id}`) ||
                    g.syncState === "syncing"
                  }
                  onClick={() =>
                    run(
                      `upload:${g.id}`,
                      () => uploadGameSave(g.id),
                      `Save hry „${g.name}“ nahraný na cloud.`,
                    )
                  }
                >
                  <UploadIcon className="h-4 w-4" />
                  {g.syncState === "error"
                    ? "Skúsiť znova"
                    : g.syncState === "syncing"
                      ? "Prebieha…"
                      : g.syncState === "synced"
                        ? "Nahrať znova"
                        : "Nahrať na cloud"}
                </Button>

                <div className="mt-auto flex items-center gap-2 border-t border-zinc-800 pt-3">
                  <label
                    className="sr-only"
                    htmlFor={`auto-upload-${g.id}`}
                  >
                    Automatické nahrávanie pre {g.name}
                  </label>
                  <select
                    id={`auto-upload-${g.id}`}
                    className={`${inputCls} flex-1 !py-1.5 text-xs`}
                    value={autoUploadToOption(g.autoUpload)}
                    disabled={busy === `auto:${g.id}`}
                    onChange={(e) => {
                      const value = optionToAutoUpload(
                        e.currentTarget.value as AutoUploadOption,
                      );
                      void run(`auto:${g.id}`, () =>
                        setGameAutoUpload(g.id, value),
                      );
                    }}
                    title="Automatické nahrávanie save súborov"
                  >
                    <option value="inherit">Dediť globálne</option>
                    <option value="on">Zapnuté</option>
                    <option value="off">Vypnuté</option>
                  </select>
                  <Button
                    variant="secondary"
                    className="!py-1.5 text-xs"
                    onClick={() => setVersionsGame(g)}
                  >
                    <HistoryIcon className="h-4 w-4" />
                    Verzie
                  </Button>
                </div>
              </Card>
            );
          })}
        </div>
      )}

      {addOpen && <AddGameModal onClose={() => setAddOpen(false)} />}

      {versionsGame && (
        <VersionsModal
          game={versionsGame}
          onClose={() => setVersionsGame(null)}
        />
      )}

      {toRemove && (
        <ConfirmDialog
          title="Odstrániť hru"
          message={`Naozaj odstrániť hru „${toRemove.name}“? Sledovanie jej save súborov sa zastaví.`}
          loading={busy === `remove:${toRemove.id}`}
          onConfirm={handleRemove}
          onCancel={() => setToRemove(null)}
        />
      )}
    </div>
  );
}
