import { useCallback, useEffect, useState } from "react";
import {
  addWatchedFolder,
  backupFolderNow,
  connectFolderBackup,
  listFolderBackups,
  pickFolder,
  removeWatchedFolder,
  restoreFolderFromCloud,
  setFolderAutoBackup,
} from "../api";
import { ConfirmDialog, Modal } from "../components/Modal";
import ProviderMenu from "../components/ProviderMenu";
import StateBadge from "../components/StateBadge";
import {
  DownloadIcon,
  FolderIcon,
  LinkIcon,
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
} from "../components/ui";
import { useApp } from "../context";
import {
  baseName,
  errMsg,
  formatBytes,
  formatDate,
  providerLabel,
} from "../format";
import { useConfigAction } from "../hooks";
import type { CloudFolderBackup, Provider, WatchedFolder } from "../types";

type AutoBackupOption = "inherit" | "on" | "off";

function autoBackupToOption(value: boolean | null): AutoBackupOption {
  if (value === null) return "inherit";
  return value ? "on" : "off";
}

function optionToAutoBackup(option: AutoBackupOption): boolean | null {
  if (option === "inherit") return null;
  return option === "on";
}

function ConnectCloudBackupModal({
  provider,
  onClose,
}: {
  provider: Provider;
  onClose: () => void;
}) {
  const { setConfig, toast } = useApp();
  const [backups, setBackups] = useState<CloudFolderBackup[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [destination, setDestination] = useState("");
  const [picking, setPicking] = useState(false);
  const [connecting, setConnecting] = useState(false);

  const load = useCallback(() => {
    setBackups(null);
    setLoadError(null);
    listFolderBackups(provider)
      .then((items) => {
        setBackups(items);
        const available = items.find((item) => !item.alreadyConnected);
        setSelectedId(available?.cloudId ?? null);
      })
      .catch((error) => setLoadError(errMsg(error)));
  }, [provider]);

  useEffect(() => {
    load();
  }, [load]);

  async function handlePick() {
    setPicking(true);
    try {
      const path = await pickFolder();
      if (path) {
        setDestination(path);
        setActionError(null);
      }
    } finally {
      setPicking(false);
    }
  }

  async function handleConnect() {
    if (!selectedId || !destination) return;
    setConnecting(true);
    setActionError(null);
    try {
      const config = await connectFolderBackup(
        provider,
        selectedId,
        destination,
      );
      setConfig(config);
      toast("success", "Záloha stiahnutá a priečinok pripojený.");
      onClose();
    } catch (error) {
      const message = errMsg(error);
      setActionError(message);
      toast("error", message);
    } finally {
      setConnecting(false);
    }
  }

  const selected = backups?.find((item) => item.cloudId === selectedId);

  return (
    <Modal
      title="Pripojiť existujúcu zálohu"
      onClose={connecting ? () => {} : onClose}
      wide
    >
      <div className="flex flex-col gap-4">
        <div>
          <div className="mb-2 text-xs font-medium text-zinc-400">
            1. Vyber zálohu z {providerLabel(provider)}
          </div>
          {loadError ? (
            <div className="rounded-lg border border-red-500/20 bg-red-500/10 p-3">
              <p className="break-words text-xs text-red-400">{loadError}</p>
              <Button className="mt-3" variant="secondary" onClick={load}>
                Skúsiť znova
              </Button>
            </div>
          ) : backups === null ? (
            <div className="flex items-center justify-center gap-2 py-8 text-sm text-zinc-500">
              <SpinnerIcon className="h-4 w-4" />
              Hľadám zálohy…
            </div>
          ) : backups.length === 0 ? (
            <div className="rounded-lg border border-zinc-800 bg-zinc-950/60 p-4 text-center text-sm text-zinc-500">
              Na {providerLabel(provider)} nie sú žiadne zálohy priečinkov.
            </div>
          ) : (
            <div className="flex max-h-64 flex-col gap-2 overflow-y-auto pr-1">
              {backups.map((backup) => (
                <button
                  key={backup.cloudId}
                  type="button"
                  disabled={backup.alreadyConnected || connecting}
                  onClick={() => {
                    setSelectedId(backup.cloudId);
                    setActionError(null);
                  }}
                  className={`rounded-lg border p-3 text-left transition-colors ${
                    selectedId === backup.cloudId
                      ? "border-indigo-500 bg-indigo-500/10"
                      : "border-zinc-800 bg-zinc-950/60 hover:border-zinc-700"
                  } disabled:cursor-not-allowed disabled:opacity-50`}
                >
                  <div className="flex items-start gap-3">
                    <FolderIcon className="mt-0.5 h-4.5 w-4.5 shrink-0 text-indigo-400" />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm font-medium text-zinc-200">
                        {backup.name}
                      </div>
                      <div className="mt-1 text-xs text-zinc-500">
                        {backup.fileCount} súborov
                        <span className="text-zinc-700"> · </span>
                        {formatBytes(backup.sizeBytes)}
                        <span className="text-zinc-700"> · </span>
                        {formatDate(backup.modifiedAt)}
                      </div>
                    </div>
                    {backup.alreadyConnected && (
                      <span className="shrink-0 text-xs text-emerald-400">
                        Už pripojené
                      </span>
                    )}
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>

        <div>
          <div className="mb-2 text-xs font-medium text-zinc-400">
            2. Vyber lokálny cieľový priečinok
          </div>
          <div className="flex items-center gap-2">
            <div
              className={`min-w-0 flex-1 truncate rounded-lg border border-zinc-700 bg-zinc-800/60 px-3 py-2 font-mono text-xs ${
                destination ? "text-zinc-200" : "text-zinc-500"
              }`}
              title={destination || undefined}
            >
              {destination || "Žiadny priečinok nevybraný"}
            </div>
            <Button
              variant="secondary"
              onClick={handlePick}
              loading={picking}
              disabled={connecting}
            >
              <FolderIcon className="h-4 w-4" />
              Vybrať
            </Button>
          </div>
        </div>

        {selected && destination && (
          <div className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-300">
            Súbory s rovnakým názvom v cieľovom priečinku budú prepísané.
            Ostatné lokálne súbory zostanú. Automatické zálohovanie bude po
            pripojení vypnuté.
          </div>
        )}

        {actionError && (
          <div className="break-words rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-xs text-red-400">
            {actionError}
          </div>
        )}

        <div className="flex justify-end gap-2">
          <Button variant="secondary" onClick={onClose} disabled={connecting}>
            Zrušiť
          </Button>
          <Button
            variant="primary"
            onClick={handleConnect}
            loading={connecting}
            disabled={!selectedId || !destination || selected?.alreadyConnected}
          >
            <DownloadIcon className="h-4 w-4" />
            Stiahnuť a pripojiť
          </Button>
        </div>
      </div>
    </Modal>
  );
}

export default function Folders() {
  const { config } = useApp();
  const { busy, run } = useConfigAction();
  const [toRemove, setToRemove] = useState<WatchedFolder | null>(null);
  const [toRestore, setToRestore] = useState<WatchedFolder | null>(null);
  const [adding, setAdding] = useState(false);
  const [connectProvider, setConnectProvider] = useState<Provider | null>(null);

  const folders = config.watchedFolders;

  async function handleAdd(provider: Provider) {
    setAdding(true);
    try {
      const path = await pickFolder();
      if (path) {
        await run(
          "add",
          () => addWatchedFolder(path, provider),
          `Priečinok pridaný do sledovania (${providerLabel(provider)}).`,
        );
      }
    } finally {
      setAdding(false);
    }
  }

  async function handleRemove() {
    if (!toRemove) return;
    const ok = await run(
      `remove:${toRemove.id}`,
      () => removeWatchedFolder(toRemove.id),
      "Priečinok odstránený zo sledovania.",
    );
    if (ok) setToRemove(null);
  }

  async function handleRestore() {
    if (!toRestore) return;
    const ok = await run(`download:${toRestore.id}`, () =>
      restoreFolderFromCloud(toRestore.id),
    );
    if (ok) setToRestore(null);
  }

  return (
    <div>
      <PageHeader
        title="Priečinky"
        subtitle="Priečinky môžeš zálohovať ručne, automaticky alebo pripojiť z cloudu."
        action={
          <div className="flex items-center gap-2">
            <ProviderMenu
              label="Pripojiť z cloudu"
              icon={<LinkIcon className="h-4 w-4" />}
              onSelect={setConnectProvider}
            />
            <ProviderMenu
              label="Pridať nový"
              variant="primary"
              icon={<PlusIcon className="h-4 w-4" />}
              disabled={adding}
              onSelect={(provider) => void handleAdd(provider)}
            />
          </div>
        }
      />

      {folders.length === 0 ? (
        <EmptyState
          icon={<FolderIcon className="h-10 w-10" />}
          message="Žiadne sledované priečinky"
          hint="Kliknite na „Pridať priečinok“ a vyberte, čo chcete zálohovať."
        />
      ) : (
        <div className="flex flex-col gap-3">
          {folders.map((f) => (
            <Card key={f.id} className="overflow-hidden">
              <div className="flex items-start gap-4 px-5 py-4">
                <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl border border-indigo-500/10 bg-indigo-600/15 text-indigo-400">
                  <FolderIcon className="h-6 w-6" />
                </div>

                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2.5">
                    <div
                      className="min-w-0 truncate text-sm font-semibold text-zinc-100"
                      title={f.path}
                    >
                      {baseName(f.path)}
                    </div>
                    <StateBadge state={f.syncState} />
                  </div>
                  <div
                    className="mt-1 truncate font-mono text-xs text-zinc-500"
                    title={f.path}
                  >
                    {f.path}
                  </div>
                  <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-zinc-500">
                    <span>{providerLabel(f.provider)}</span>
                    <span className="hidden h-1 w-1 rounded-full bg-zinc-700 sm:block" />
                    <span>Posledná záloha: {formatDate(f.lastSync)}</span>
                  </div>
                  {f.lastError && (
                    <div className="mt-3 break-words rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-xs text-red-400">
                      {f.lastError}
                    </div>
                  )}
                </div>

                <Button
                  variant="ghost"
                  className="-mr-2 !p-2 text-zinc-500 hover:!bg-red-500/10 hover:!text-red-400"
                  onClick={() => setToRemove(f)}
                  aria-label="Odstrániť priečinok"
                  title="Odstrániť priečinok"
                >
                  <TrashIcon className="h-4.5 w-4.5" />
                </Button>
              </div>

              <div className="grid gap-3 border-t border-zinc-800 bg-zinc-950/25 px-5 py-3.5 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center">
                <div className="flex min-w-0 flex-wrap items-center gap-3">
                  <div className="min-w-36">
                    <label
                      className="block text-xs font-medium text-zinc-300"
                      htmlFor={`auto-backup-${f.id}`}
                    >
                      Automatické zálohovanie
                    </label>
                    <div className="mt-0.5 text-[11px] text-zinc-600">
                      Nahrá zmeny bez klikania
                    </div>
                  </div>
                  <select
                    id={`auto-backup-${f.id}`}
                    className={`${inputCls} !w-44 !py-1.5 text-xs`}
                    value={autoBackupToOption(f.autoBackup)}
                    disabled={busy !== null}
                    onChange={(e) => {
                      const value = optionToAutoBackup(
                        e.currentTarget.value as AutoBackupOption,
                      );
                      void run(`auto:${f.id}`, () =>
                        setFolderAutoBackup(f.id, value),
                      );
                    }}
                    title="Automatické zálohovanie priečinka"
                  >
                    <option value="inherit">Dediť globálne</option>
                    <option value="on">Zapnuté</option>
                    <option value="off">Vypnuté</option>
                  </select>
                </div>

                <div className="grid grid-cols-2 gap-2 sm:flex sm:justify-end">
                  <Button
                    className="whitespace-nowrap"
                    variant={
                      f.syncState === "error" &&
                      f.lastErrorAction === "download"
                        ? "primary"
                        : "secondary"
                    }
                    loading={busy === `download:${f.id}`}
                    disabled={
                      (busy !== null && busy !== `download:${f.id}`) ||
                      (f.syncState === "syncing" &&
                        busy !== `download:${f.id}`)
                    }
                    onClick={() => setToRestore(f)}
                    title="Prepísať lokálne súbory verziou z cloudu"
                  >
                    <DownloadIcon className="h-4 w-4" />
                    {f.syncState === "error" &&
                    f.lastErrorAction === "download"
                      ? "Stiahnuť znova"
                      : "Stiahnuť"}
                  </Button>

                  <Button
                    className="whitespace-nowrap"
                    variant={
                      f.syncState === "changed" ||
                      (f.syncState === "error" &&
                        f.lastErrorAction === "upload")
                        ? "primary"
                        : "secondary"
                    }
                    loading={
                      busy === `backup:${f.id}` ||
                      (f.syncState === "syncing" &&
                        busy !== `download:${f.id}`)
                    }
                    disabled={
                      (busy !== null && busy !== `backup:${f.id}`) ||
                      f.syncState === "syncing"
                    }
                    onClick={() =>
                      run(
                        `backup:${f.id}`,
                        () => backupFolderNow(f.id),
                        "Záloha dokončená.",
                      )
                    }
                  >
                    <UploadIcon className="h-4 w-4" />
                    {f.syncState === "error" &&
                    f.lastErrorAction === "upload"
                      ? "Zálohovať znova"
                      : f.syncState === "syncing"
                        ? "Prebieha…"
                        : "Zálohovať teraz"}
                  </Button>
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}

      {connectProvider && (
        <ConnectCloudBackupModal
          provider={connectProvider}
          onClose={() => setConnectProvider(null)}
        />
      )}

      {toRestore && (
        <ConfirmDialog
          title="Stiahnuť priečinok z cloudu"
          message={`Stiahnuť zálohu „${toRestore.cloudName}“ do „${toRestore.path}“? Lokálne súbory s rovnakým názvom budú prepísané.`}
          confirmLabel="Stiahnuť"
          loading={busy === `download:${toRestore.id}`}
          onConfirm={handleRestore}
          onCancel={() => setToRestore(null)}
        />
      )}

      {toRemove && (
        <ConfirmDialog
          title="Odstrániť priečinok"
          message={`Naozaj odstrániť „${toRemove.path}“ zo sledovania? Zálohovanie tohto priečinka sa zastaví.`}
          loading={busy === `remove:${toRemove.id}`}
          onConfirm={handleRemove}
          onCancel={() => setToRemove(null)}
        />
      )}
    </div>
  );
}
