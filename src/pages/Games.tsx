import { useEffect, useState } from "react";
import {
  addGame,
  backupGameLocal,
  connectGameBackup,
  listGameCloudStatuses,
  listGameBackups,
  listGameVersions,
  openLocalFolder,
  pickFolder,
  removeGame,
  restoreGameVersion,
  setGameAutoUpload,
  updateGame,
  uploadGameSave,
} from "../api";
import { ConfirmDialog, Modal } from "../components/Modal";
import ProviderMenu from "../components/ProviderMenu";
import StateBadge from "../components/StateBadge";
import {
  CloudIcon,
  DownloadIcon,
  FolderIcon,
  GamepadIcon,
  GridIcon,
  HistoryIcon,
  LinkIcon,
  ListIcon,
  PlusIcon,
  SpinnerIcon,
  SettingsIcon,
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
import type {
  CloudGameBackup,
  Game,
  GameCloudStatus,
  Provider,
  VersionInfo,
} from "../types";

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
  }, [game.id, game.lastUpload, reloadKey]);

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

function ConnectGameModal({
  provider,
  backup,
  onClose,
  onDone,
}: {
  provider: Provider;
  backup: CloudGameBackup;
  onClose: () => void;
  onDone: () => void;
}) {
  const { toast } = useApp();
  const { busy, run } = useConfigAction();
  const [name, setName] = useState(backup.name);
  const [destination, setDestination] = useState("");
  const [backupPath, setBackupPath] = useState("");
  const [picking, setPicking] = useState(false);

  async function handlePick() {
    setPicking(true);
    try {
      const path = await pickFolder();
      if (path) setDestination(path);
    } finally {
      setPicking(false);
    }
  }

  async function handleConnect() {
    const trimmed = name.trim();
    if (!trimmed) {
      toast("error", "Zadajte názov hry.");
      return;
    }
    if (!destination || !backupPath.trim()) {
      toast("error", "Vyberte save priečinok aj miesto lokálnych záloh.");
      return;
    }
    const ok = await run(
      "connectGame",
      () => connectGameBackup(provider, backup.cloudId, destination, trimmed, backupPath.trim()),
      `Hra „${trimmed}“ bola stiahnutá z cloudu a napojená. Pôvodné save súbory sú, ak existovali, v ZIPe vo vybranom priečinku záloh.`,
    );
    if (ok) onDone();
  }

  return (
    <Modal title={`Napojiť z cloudu — ${backup.name}`} onClose={onClose}>
      <div className="flex flex-col gap-4">
        <div className="rounded-lg border border-zinc-800 bg-zinc-800/40 px-3 py-2 text-xs text-zinc-400">
          Najnovšia verzia z {formatDate(backup.latestAt)}
          <span className="text-zinc-700"> · </span>
          {backup.versionCount}{" "}
          {backup.versionCount === 1 ? "verzia" : "verzií"}
          <span className="text-zinc-700"> · </span>
          {formatBytes(backup.sizeBytes)}
        </div>

        <div>
          <label className={labelCls} htmlFor="connect-game-name">
            Názov hry
          </label>
          <input
            id="connect-game-name"
            className={inputCls}
            value={name}
            onChange={(e) => setName(e.currentTarget.value)}
            autoFocus
          />
        </div>

        <div>
          <span className={labelCls}>
            Kam synchronizovať na tomto PC
          </span>
          <div className="flex items-center gap-2">
            <div
              className={`min-w-0 flex-1 truncate rounded-lg border border-zinc-700 bg-zinc-800/60 px-3 py-2 font-mono text-xs ${destination ? "text-zinc-200" : "text-zinc-500"}`}
              title={destination || undefined}
            >
              {destination || "Žiadny priečinok nevybraný"}
            </div>
            <Button variant="secondary" onClick={handlePick} loading={picking}>
              <FolderIcon className="h-4 w-4" />
              Vybrať
            </Button>
          </div>
          <p className="mt-1.5 text-xs text-zinc-500">
            Cloud save sa rozbalí priamo do tohto priečinka, bez ďalšieho
            vnoreného priečinka. Ak tu už sú save súbory, najprv sa uložia do
            ZIPu do vybraného priečinka lokálnych záloh.
          </p>
        </div>

        <BackupLocationPicker value={backupPath} onChange={setBackupPath} />
        <div className="mt-1 flex justify-end gap-2">
          <Button variant="secondary" onClick={onClose}>
            Zrušiť
          </Button>
          <Button
            variant="primary"
            onClick={handleConnect}
            loading={busy === "connectGame"}
          >
            <DownloadIcon className="h-4 w-4" />
            Stiahnuť a napojiť
          </Button>
        </div>
      </div>
    </Modal>
  );
}

function CloudGamesModal({
  provider,
  onClose,
}: {
  provider: Provider;
  onClose: () => void;
}) {
  const [backups, setBackups] = useState<CloudGameBackup[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [selected, setSelected] = useState<CloudGameBackup | null>(null);

  useEffect(() => {
    let cancelled = false;
    setBackups(null);
    setLoadError(null);
    listGameBackups(provider)
      .then((b) => {
        if (!cancelled) setBackups(b);
      })
      .catch((e) => {
        if (!cancelled) setLoadError(errMsg(e));
      });
    return () => {
      cancelled = true;
    };
  }, [provider, reloadKey]);

  return (
    <>
      <Modal
        title={`Vybrať z cloudu — ${providerLabel(provider)}`}
        onClose={onClose}
        wide
      >
        <p className="mb-4 text-sm text-zinc-400">
          Herné save súbory zálohované na {providerLabel(provider)}. Vyber hru a
          priečinok na tomto PC, kam sa má stiahnuť.
        </p>

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
        ) : backups === null ? (
          <div className="flex items-center justify-center gap-2 py-8 text-sm text-zinc-500">
            <SpinnerIcon className="h-4 w-4" />
            Načítavajú sa zálohy z cloudu…
          </div>
        ) : backups.length === 0 ? (
          <p className="py-4 text-center text-sm text-zinc-500">
            Na cloude zatiaľ nie sú žiadne herné zálohy.
          </p>
        ) : (
          <div className="divide-y divide-zinc-800">
            {backups.map((b) => (
              <div key={b.cloudId} className="flex items-center gap-3 py-3">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-indigo-600/15 text-indigo-400">
                  <GamepadIcon className="h-4.5 w-4.5" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-sm font-medium text-zinc-100">
                      {b.name}
                    </span>
                    {b.alreadyConnected && (
                      <span className="shrink-0 rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-medium text-emerald-400">
                        Už napojené
                      </span>
                    )}
                  </div>
                  <div className="text-xs text-zinc-500">
                    {formatDate(b.latestAt)}
                    <span className="text-zinc-700"> · </span>
                    {b.versionCount}{" "}
                    {b.versionCount === 1 ? "verzia" : "verzií"}
                    <span className="text-zinc-700"> · </span>
                    {formatBytes(b.sizeBytes)}
                  </div>
                </div>
                <Button variant="secondary" onClick={() => setSelected(b)}>
                  <LinkIcon className="h-4 w-4" />
                  Napojiť na PC
                </Button>
              </div>
            ))}
          </div>
        )}
      </Modal>

      {selected && (
        <ConnectGameModal
          provider={provider}
          backup={selected}
          onClose={() => setSelected(null)}
          onDone={() => {
            setSelected(null);
            onClose();
          }}
        />
      )}
    </>
  );
}

function BackupLocationPicker({
  value,
  onChange,
}: {
  value: string;
  onChange: (path: string) => void;
}) {
  const [picking, setPicking] = useState(false);
  const { toast } = useApp();
  return (
    <div>
      <label className={labelCls}>
        Priečinok pre lokálne ZIP zálohy
        <input
          className={inputCls}
          value={value}
          onChange={(e) => onChange(e.currentTarget.value)}
          placeholder="Vyber priečinok mimo save súborov"
        />
      </label>
      <Button
        variant="secondary"
        loading={picking}
        onClick={async () => {
          setPicking(true);
          try {
            const path = await pickFolder();
            if (path) onChange(path);
          } catch (error) {
            toast("error", errMsg(error));
          } finally {
            setPicking(false);
          }
        }}
      >
        <FolderIcon className="h-4 w-4" /> Vybrať miesto záloh
      </Button>
    </div>
  );
}

function AddGameModal({ onClose }: { onClose: () => void }) {
  const { toast } = useApp();
  const { busy, run } = useConfigAction();
  const [name, setName] = useState("");
  const [savePath, setSavePath] = useState("");
  const [backupPath, setBackupPath] = useState("");
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
    if (!savePath || !backupPath.trim()) {
      toast("error", "Vyberte save priečinok aj miesto lokálnych záloh.");
      return;
    }
    const ok = await run(
      "addGame",
      () => addGame(trimmed, savePath, provider, backupPath.trim()),
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
          <BackupLocationPicker value={backupPath} onChange={setBackupPath} />
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
            <option value="ulozto">Ulož.to Disk</option>
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

function EditGameModal({
  game,
  onClose,
}: {
  game: Game;
  onClose: () => void;
}) {
  const { toast } = useApp();
  const { busy, run } = useConfigAction();
  const [name, setName] = useState(game.name);
  const [savePath, setSavePath] = useState(game.savePath);
  const [backupPath, setBackupPath] = useState(game.backupPath);
  const [provider, setProvider] = useState<Provider>(game.provider);
  const [picking, setPicking] = useState<"save" | "backup" | null>(null);

  async function handlePick(target: "save" | "backup") {
    setPicking(target);
    try {
      const path = await pickFolder();
      if (!path) return;
      if (target === "save") setSavePath(path);
      else setBackupPath(path);
    } finally {
      setPicking(null);
    }
  }

  async function handleSubmit() {
    const trimmed = name.trim();
    if (!trimmed || !savePath || !backupPath) {
      toast("error", "Vyplňte názov hry, save cestu aj backup cestu.");
      return;
    }
    const ok = await run(
      `edit:${game.id}`,
      () => updateGame(game.id, trimmed, savePath, backupPath, provider),
      `Hra „${trimmed}“ upravená.`,
    );
    if (ok) onClose();
  }

  return (
    <Modal title={`Upraviť hru: ${game.name}`} onClose={onClose}>
      <form
        className="flex flex-col gap-4"
        onSubmit={(e) => {
          e.preventDefault();
          void handleSubmit();
        }}
      >
        <div>
          <label className={labelCls} htmlFor={`edit-game-name-${game.id}`}>
            Názov hry v aplikácii
          </label>
          <input
            id={`edit-game-name-${game.id}`}
            className={inputCls}
            value={name}
            onChange={(e) => setName(e.currentTarget.value)}
            autoFocus
          />
        </div>

        <div>
          <label className={labelCls} htmlFor={`edit-game-save-${game.id}`}>
            Cesta k save súborom
          </label>
          <div className="flex gap-2">
            <input
              id={`edit-game-save-${game.id}`}
              className={inputCls}
              value={savePath}
              onChange={(e) => setSavePath(e.currentTarget.value)}
            />
            <Button
              type="button"
              variant="secondary"
              loading={picking === "save"}
              onClick={() => void handlePick("save")}
            >
              Vybrať
            </Button>
          </div>
        </div>

        <div>
          <label className={labelCls} htmlFor={`edit-game-backup-${game.id}`}>
            Cesta pre lokálne ZIP zálohy
          </label>
          <div className="flex gap-2">
            <input
              id={`edit-game-backup-${game.id}`}
              className={inputCls}
              value={backupPath}
              onChange={(e) => setBackupPath(e.currentTarget.value)}
            />
            <Button
              type="button"
              variant="secondary"
              loading={picking === "backup"}
              onClick={() => void handlePick("backup")}
            >
              Vybrať
            </Button>
          </div>
          <p className="mt-1.5 text-xs text-zinc-500">
            Sem sa ukladajú automatické ZIP zálohy pri zmene save-u aj ochranné zálohy pred obnovením
            verzie z cloudu.
          </p>
        </div>

        <div>
          <label className={labelCls} htmlFor={`edit-game-provider-${game.id}`}>
            Cloud disk
          </label>
          <select
            id={`edit-game-provider-${game.id}`}
            className={inputCls}
            value={provider}
            onChange={(e) => setProvider(e.currentTarget.value as Provider)}
          >
            <option value="gdrive">Google Drive</option>
            <option value="ulozto">Ulož.to Disk</option>
            <option value="customCloud">Vlastný cloud</option>
          </select>
          <p className="mt-1.5 text-xs text-zinc-500">
            Po zmene cloud disku môžeš obnoviť existujúcu verziu cez „Verzie“
            alebo nahrať nový save na zvolený cloud.
          </p>
        </div>

        <div className="mt-1 flex justify-end gap-2">
          <Button type="button" variant="secondary" onClick={onClose}>
            Zrušiť
          </Button>
          <Button variant="primary" type="submit" loading={busy === `edit:${game.id}`}>
            Uložiť zmeny
          </Button>
        </div>
      </form>
    </Modal>
  );
}

export default function Games() {
  const { config, toast } = useApp();
  const { busy, run } = useConfigAction();
  const [toRemove, setToRemove] = useState<Game | null>(null);
  const [editGame, setEditGame] = useState<Game | null>(null);
  const [versionsGame, setVersionsGame] = useState<Game | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const [cloudProvider, setCloudProvider] = useState<Provider | null>(null);
  const [cloudStatuses, setCloudStatuses] = useState<
    Record<string, GameCloudStatus>
  >({});
  const [viewMode, setViewMode] = useState<"list" | "grid">(() => {
    try {
      const saved = localStorage.getItem("games_view_mode");
      if (saved === "list" || saved === "grid") return saved;
    } catch {
      // ignore
    }
    return "list";
  });

  const handleSetViewMode = (mode: "list" | "grid") => {
    setViewMode(mode);
    try {
      localStorage.setItem("games_view_mode", mode);
    } catch {
      // ignore
    }
  };

  const games = config.games;
  const cloudStatusKey = games
    .map(
      (game) =>
        `${game.id}:${game.provider}:${game.cloudName}:${game.lastUpload}:${game.lastCloudVersionId}`,
    )
    .join("|");

  useEffect(() => {
    let cancelled = false;
    async function refresh() {
      try {
        const statuses = await listGameCloudStatuses();
        if (!cancelled) {
          setCloudStatuses(Object.fromEntries(statuses.map((status) => [status.id, status])));
        }
      } catch {
        // Nedostupný cloud nesmie zablokovať zobrazenie lokálnych hier.
      }
    }
    void refresh();
    const interval = window.setInterval(() => void refresh(), 60_000);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [cloudStatusKey]);

  async function handleOpenFolder(game: Game) {
    try {
      await openLocalFolder(game.savePath);
    } catch (e) {
      toast("error", errMsg(e));
    }
  }

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
          <div className="flex items-center gap-2">
            <div className="flex items-center rounded-lg border border-zinc-800 bg-zinc-900 p-0.5">
              <button
                type="button"
                className={`rounded-md p-1.5 transition-colors ${
                  viewMode === "list"
                    ? "bg-zinc-800 text-zinc-100 shadow-xs"
                    : "text-zinc-500 hover:text-zinc-300"
                }`}
                onClick={() => handleSetViewMode("list")}
                title="Zobrazenie ako zoznam"
                aria-label="Zobrazenie ako zoznam"
              >
                <ListIcon className="h-4 w-4" />
              </button>
              <button
                type="button"
                className={`rounded-md p-1.5 transition-colors ${
                  viewMode === "grid"
                    ? "bg-zinc-800 text-zinc-100 shadow-xs"
                    : "text-zinc-500 hover:text-zinc-300"
                }`}
                onClick={() => handleSetViewMode("grid")}
                title="Zobrazenie ako mriežka"
                aria-label="Zobrazenie ako mriežka"
              >
                <GridIcon className="h-4 w-4" />
              </button>
            </div>
            <ProviderMenu
              label="Vybrať z cloudu"
              icon={<DownloadIcon className="h-4 w-4" />}
              onSelect={setCloudProvider}
            />
            <Button variant="primary" onClick={() => setAddOpen(true)}>
              <PlusIcon className="h-4 w-4" />
              Pridať hru
            </Button>
          </div>
        }
      />

      {games.length === 0 ? (
        <EmptyState
          icon={<GamepadIcon className="h-10 w-10" />}
          message="Žiadne hry"
          hint="Pridajte hru a vyberte priečinok so save súbormi, alebo stiahnite existujúcu zálohu cez „Vybrať z cloudu“."
        />
      ) : viewMode === "list" ? (
        <div className="flex flex-col gap-3">
          {games.map((g) => {
            const changed = g.syncState === "changed";
            const cloudStatus = cloudStatuses[g.id];
            const hasWaitingBackup = Boolean(
              g.local.backupHash &&
                g.local.observedHash &&
                g.local.backupHash !== g.local.observedHash,
            );
            const isUnconfirmedCloud = Boolean(
              g.local.observedHash &&
                g.local.uploadedHash !== g.local.observedHash,
            );

            return (
              <Card
                key={g.id}
                className={`overflow-hidden transition-shadow ${
                  changed
                    ? "!border-amber-500/50 ring-2 ring-amber-500/20"
                    : ""
                }`}
              >
                <div className="flex items-start gap-4 px-5 py-4">
                  <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl border border-indigo-500/10 bg-indigo-600/15 text-indigo-400">
                    <GamepadIcon className="h-6 w-6" />
                  </div>

                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2.5">
                      <div
                        className="min-w-0 truncate text-sm font-semibold text-zinc-100"
                        title={g.name}
                      >
                        {g.name}
                      </div>
                      <StateBadge state={g.syncState} />
                      {cloudStatus?.updateAvailable && (
                        <span
                          className="rounded-md border border-amber-500/40 bg-amber-500/10 px-2 py-0.5 text-xs font-medium text-amber-300"
                          title={`Cloud má novšiu verziu z ${formatDate(cloudStatus.latestAt)}. Otvor Verzie a obnov ju.`}
                        >
                          Aktualizácia!
                        </span>
                      )}
                    </div>

                    <div
                      className="mt-1 truncate font-mono text-xs text-zinc-500"
                      title={`Save priečinok: ${g.savePath}\nLokálne ZIP zálohy: ${g.local.backupFile ?? g.backupPath}`}
                    >
                      {g.savePath}
                    </div>

                    <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-zinc-500">
                      <span className="inline-flex items-center gap-1">
                        <CloudIcon className="h-3.5 w-3.5" />
                        {providerLabel(g.provider)}
                      </span>
                      <span className="hidden h-1 w-1 rounded-full bg-zinc-700 sm:block" />
                      <span>Nahratie: {formatDate(g.lastUpload)}</span>
                      <span className="hidden h-1 w-1 rounded-full bg-zinc-700 sm:block" />
                      <span>Lokálna záloha: {formatDate(g.local.lastBackup)}</span>
                      {g.lastChange && (
                        <>
                          <span className="hidden h-1 w-1 rounded-full bg-zinc-700 sm:block" />
                          <span>Zmena: {formatDate(g.lastChange)}</span>
                        </>
                      )}
                      {hasWaitingBackup && (
                        <span className="text-amber-400 font-medium">· čaká na novú zálohu</span>
                      )}
                      {isUnconfirmedCloud && !hasWaitingBackup && (
                        <span className="text-amber-400 font-medium">· čaká na upload</span>
                      )}
                    </div>

                    {g.local.watchError && (
                      <p className="mt-2 text-xs text-amber-400">{g.local.watchError}</p>
                    )}
                    {g.lastCloudVersionId && !g.local.uploadedHash && (
                      <p className="mt-2 text-xs text-amber-400">
                        Obsah starej cloudovej verzie ešte nie je overený. Skontroluj save a použi „Nahrať na cloud“.
                      </p>
                    )}
                    {g.lastError && (
                      <div className="mt-2 break-words rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-xs text-red-400">
                        {g.lastError}
                      </div>
                    )}
                  </div>

                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      className="!p-2 text-zinc-500 hover:!bg-zinc-700/50 hover:!text-zinc-200"
                      onClick={() => handleOpenFolder(g)}
                      aria-label={`Otvoriť priečinok hry ${g.name}`}
                      title="Otvoriť priečinok so save súbormi"
                    >
                      <FolderIcon className="h-4 w-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      className="!p-2 text-zinc-500 hover:!bg-zinc-700/50 hover:!text-zinc-200"
                      onClick={() => setEditGame(g)}
                      aria-label={`Upraviť hru ${g.name}`}
                      title="Upraviť hru"
                    >
                      <SettingsIcon className="h-4 w-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      className="!p-2 text-zinc-500 hover:!bg-red-500/10 hover:!text-red-400"
                      onClick={() => setToRemove(g)}
                      aria-label={`Odstrániť hru ${g.name}`}
                      title="Odstrániť hru"
                    >
                      <TrashIcon className="h-4 w-4" />
                    </Button>
                  </div>
                </div>

                <div className="flex flex-wrap items-center justify-between gap-3 border-t border-zinc-800 bg-zinc-950/25 px-5 py-3">
                  <div className="flex items-center gap-2">
                    <label
                      className="text-xs text-zinc-400 whitespace-nowrap"
                      htmlFor={`auto-upload-${g.id}`}
                    >
                      Auto-upload:
                    </label>
                    <select
                      id={`auto-upload-${g.id}`}
                      className={`${inputCls} !w-36 !py-1 text-xs`}
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
                  </div>

                  <div className="flex flex-wrap items-center gap-2">
                    <Button
                      variant="secondary"
                      className="!py-1.5 text-xs whitespace-nowrap"
                      onClick={() => setVersionsGame(g)}
                    >
                      <HistoryIcon className="h-4 w-4" />
                      Verzie
                    </Button>
                    <Button
                      variant="secondary"
                      className="!py-1.5 text-xs whitespace-nowrap"
                      disabled={busy !== null || g.syncState === "syncing"}
                      onClick={() =>
                        run(
                          `local:${g.id}`,
                          () => backupGameLocal(g.id),
                          "Lokálna záloha save je overená.",
                        )
                      }
                      title="Vytvoriť okamžitú lokálnu ZIP zálohu"
                    >
                      <FolderIcon className="h-4 w-4" />
                      Zálohovať lokálne
                    </Button>
                    <Button
                      variant={g.syncState === "synced" ? "secondary" : "primary"}
                      className="!py-1.5 text-xs whitespace-nowrap"
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
                  </div>
                </div>
              </Card>
            );
          })}
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2 2xl:grid-cols-3">
          {games.map((g) => {
            const changed = g.syncState === "changed";
            const cloudStatus = cloudStatuses[g.id];
            const hasWaitingBackup = Boolean(
              g.local.backupHash &&
                g.local.observedHash &&
                g.local.backupHash !== g.local.observedHash,
            );
            const isUnconfirmedCloud = Boolean(
              g.local.observedHash &&
                g.local.uploadedHash !== g.local.observedHash,
            );

            return (
              <Card
                key={g.id}
                className={`flex flex-col justify-between overflow-hidden transition-shadow ${
                  changed
                    ? "!border-amber-500/50 ring-2 ring-amber-500/20"
                    : ""
                }`}
              >
                <div className="p-4">
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
                        title={`Save: ${g.savePath}\nLokálne ZIP zálohy: ${g.local.backupFile ?? g.backupPath}`}
                      >
                        {g.savePath}
                      </div>
                    </div>
                    <Button
                      variant="ghost"
                      className="!p-1.5 text-zinc-500 hover:!bg-zinc-700/50 hover:!text-zinc-200"
                      onClick={() => handleOpenFolder(g)}
                      aria-label={`Otvoriť priečinok hry ${g.name}`}
                      title="Otvoriť priečinok so save súbormi"
                    >
                      <FolderIcon className="h-4 w-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      className="!p-1.5 text-zinc-500 hover:!bg-zinc-700/50 hover:!text-zinc-200"
                      onClick={() => setEditGame(g)}
                      aria-label={`Upraviť hru ${g.name}`}
                      title="Upraviť hru"
                    >
                      <SettingsIcon className="h-4 w-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      className="!p-1.5 text-zinc-500 hover:!bg-red-500/10 hover:!text-red-400"
                      onClick={() => setToRemove(g)}
                      aria-label={`Odstrániť hru ${g.name}`}
                      title="Odstrániť hru"
                    >
                      <TrashIcon className="h-4 w-4" />
                    </Button>
                  </div>

                  <div className="mt-3 flex flex-wrap items-center gap-2">
                    <StateBadge state={g.syncState} />
                    {cloudStatus?.updateAvailable && (
                      <span
                        className="rounded-md border border-amber-500/40 bg-amber-500/10 px-2 py-0.5 text-xs font-medium text-amber-300"
                        title={`Cloud má novšiu verziu z ${formatDate(cloudStatus.latestAt)}. Otvor Verzie a obnov ju.`}
                      >
                        Aktualizácia!
                      </span>
                    )}
                    <span className="inline-flex items-center gap-1 text-xs text-zinc-500">
                      <CloudIcon className="h-3.5 w-3.5" />
                      {providerLabel(g.provider)}
                    </span>
                  </div>

                  <div className="mt-2.5 space-y-0.5 text-xs text-zinc-500">
                    <div className="truncate">
                      Nahratie: {formatDate(g.lastUpload)}
                    </div>
                    <div className="truncate">
                      Záloha: {formatDate(g.local.lastBackup)}
                      {hasWaitingBackup && (
                        <span className="text-amber-400 font-medium"> · čaká na novú</span>
                      )}
                      {isUnconfirmedCloud && !hasWaitingBackup && (
                        <span className="text-amber-400 font-medium"> · neoverené na cloude</span>
                      )}
                    </div>
                  </div>

                  {g.local.watchError && (
                    <p className="mt-2 text-xs text-amber-400">{g.local.watchError}</p>
                  )}
                  {g.lastCloudVersionId && !g.local.uploadedHash && (
                    <p className="mt-2 text-xs text-amber-400">
                      Obsah starej cloudovej verzie ešte nie je overený. Skontroluj save a použi „Nahrať na cloud“.
                    </p>
                  )}
                  {g.lastError && (
                    <div className="mt-2 break-words rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-xs text-red-400">
                      {g.lastError}
                    </div>
                  )}
                </div>

                <div className="border-t border-zinc-800 bg-zinc-950/25 p-3">
                  <div className="mb-2 flex items-center gap-2">
                    <label
                      className="sr-only"
                      htmlFor={`auto-upload-grid-${g.id}`}
                    >
                      Automatické nahrávanie pre {g.name}
                    </label>
                    <select
                      id={`auto-upload-grid-${g.id}`}
                      className={`${inputCls} !py-1 text-xs`}
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
                      className="!py-1 text-xs whitespace-nowrap"
                      onClick={() => setVersionsGame(g)}
                      title="História verzií"
                    >
                      <HistoryIcon className="h-3.5 w-3.5" />
                      Verzie
                    </Button>
                  </div>

                  <div className="grid grid-cols-2 gap-2">
                    <Button
                      variant="secondary"
                      className="!py-1 text-xs whitespace-nowrap"
                      disabled={busy !== null || g.syncState === "syncing"}
                      onClick={() =>
                        run(
                          `local:${g.id}`,
                          () => backupGameLocal(g.id),
                          "Lokálna záloha save je overená.",
                        )
                      }
                      title="Vytvoriť lokálnu ZIP zálohu"
                    >
                      <FolderIcon className="h-3.5 w-3.5" />
                      Zálohovať
                    </Button>
                    <Button
                      variant={g.syncState === "synced" ? "secondary" : "primary"}
                      className="!py-1 text-xs whitespace-nowrap"
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
                      <UploadIcon className="h-3.5 w-3.5" />
                      {g.syncState === "error"
                        ? "Znova"
                        : g.syncState === "syncing"
                          ? "Prebieha…"
                          : g.syncState === "synced"
                            ? "Znova"
                            : "Nahrať"}
                    </Button>
                  </div>
                </div>
              </Card>
            );
          })}
        </div>
      )}

      {addOpen && <AddGameModal onClose={() => setAddOpen(false)} />}

      {editGame && <EditGameModal game={editGame} onClose={() => setEditGame(null)} />}

      {cloudProvider && (
        <CloudGamesModal
          provider={cloudProvider}
          onClose={() => setCloudProvider(null)}
        />
      )}

      {versionsGame && (
        <VersionsModal
          game={config.games.find((game) => game.id === versionsGame.id) ?? versionsGame}
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
