import { useState } from "react";
import {
  addWatchedFolder,
  backupFolderNow,
  pickFolder,
  removeWatchedFolder,
} from "../api";
import { ConfirmDialog } from "../components/Modal";
import StateBadge from "../components/StateBadge";
import {
  FolderIcon,
  PlusIcon,
  TrashIcon,
  UploadIcon,
} from "../components/icons";
import { Button, Card, EmptyState, PageHeader } from "../components/ui";
import { useApp } from "../context";
import { baseName, formatDate, providerLabel } from "../format";
import { useConfigAction } from "../hooks";
import type { WatchedFolder } from "../types";

export default function Folders() {
  const { config } = useApp();
  const { busy, run } = useConfigAction();
  const [toRemove, setToRemove] = useState<WatchedFolder | null>(null);
  const [adding, setAdding] = useState(false);

  const folders = config.watchedFolders;

  async function handleAdd() {
    setAdding(true);
    try {
      const path = await pickFolder();
      if (path) {
        await run(
          "add",
          () => addWatchedFolder(path, "gdrive"),
          "Priečinok pridaný do sledovania.",
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

  return (
    <div>
      <PageHeader
        title="Priečinky"
        subtitle="Sledované priečinky sa automaticky zálohujú na cloud."
        action={
          <Button variant="primary" onClick={handleAdd} loading={adding}>
            <PlusIcon className="h-4 w-4" />
            Pridať priečinok
          </Button>
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
            <Card key={f.id} className="flex items-center gap-4 p-4">
              <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg bg-indigo-600/15 text-indigo-400">
                <FolderIcon className="h-5.5 w-5.5" />
              </div>

              <div className="min-w-0 flex-1">
                <div
                  className="truncate text-sm font-medium text-zinc-100"
                  title={f.path}
                >
                  {baseName(f.path)}
                </div>
                <div className="truncate font-mono text-xs text-zinc-500">
                  {f.path}
                </div>
                <div className="mt-1 text-xs text-zinc-500">
                  {providerLabel(f.provider)}
                  <span className="text-zinc-700"> · </span>
                  Posledná záloha: {formatDate(f.lastSync)}
                </div>
                {f.lastError && (
                  <div className="mt-1 break-words text-xs text-red-400">
                    {f.lastError}
                  </div>
                )}
              </div>

              <StateBadge state={f.syncState} />

              <Button
                variant="secondary"
                loading={busy === `backup:${f.id}`}
                disabled={busy !== null && busy !== `backup:${f.id}`}
                onClick={() =>
                  run(
                    `backup:${f.id}`,
                    () => backupFolderNow(f.id),
                    "Záloha dokončená.",
                  )
                }
              >
                <UploadIcon className="h-4 w-4" />
                Zálohovať teraz
              </Button>

              <Button
                variant="ghost"
                className="!px-2 text-zinc-500 hover:!bg-red-500/10 hover:!text-red-400"
                onClick={() => setToRemove(f)}
                aria-label="Odstrániť priečinok"
              >
                <TrashIcon className="h-4.5 w-4.5" />
              </Button>
            </Card>
          ))}
        </div>
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
