# IPC kontrakt (Rust ⇄ React)

Payloady používajú camelCase JSON. Frontend volá Rust cez `invoke()` z `@tauri-apps/api/core`; Rust posiela eventy cez Tauri event systém.

## Dátové typy

```ts
type Provider = "gdrive" | "customCloud";
type SyncState = "idle" | "changed" | "syncing" | "synced" | "error";
type CompressionLevel = "fast" | "best";

interface WatchedFolder {
  id: string;
  path: string;
  provider: Provider;
  cloudId: string | null;
  cloudName: string;
  autoBackup: boolean | null;
  syncState: SyncState;
  lastSync: string | null;
  lastError: string | null;
  lastErrorAction: "upload" | "download" | null;
}

interface Game {
  id: string;
  name: string;
  savePath: string;
  provider: Provider;
  autoUpload: boolean | null;
  syncState: SyncState;
  lastChange: string | null;
  lastUpload: string | null;
  lastError: string | null;
}

interface Settings {
  theme: "dark" | "light";
  debounceSeconds: number;
  autoBackupFolders: boolean;
  autoUploadSaves: boolean;
  compressionLevel: CompressionLevel;
  gdriveClientId: string;
}

interface CloudFolderBackup {
  cloudId: string;
  name: string;
  modifiedAt: string | null;
  fileCount: number;
  sizeBytes: number;
  alreadyConnected: boolean;
}
```

`autoBackup` a `autoUpload`: `null` dedí globálne nastavenie, `true` ho zapína a `false` vypína pre konkrétnu položku.

## Commands

| Command | Args | Návrat |
|---|---|---|
| `get_config` | — | `AppConfig` |
| `update_settings` | `{ settings }` | `AppConfig` |
| `add_watched_folder` | `{ path, provider }` | `AppConfig` |
| `remove_watched_folder` | `{ id }` | `AppConfig` |
| `set_folder_auto_backup` | `{ id, value }` | `AppConfig` |
| `backup_folder_now` | `{ id }` | `AppConfig` |
| `list_folder_backups` | — | `CloudFolderBackup[]` |
| `connect_folder_backup` | `{ cloudId, path }` | `AppConfig` |
| `restore_folder_from_cloud` | `{ id }` | `AppConfig` |
| `add_game` | `{ name, savePath, provider }` | `AppConfig` |
| `remove_game` | `{ id }` | `AppConfig` |
| `set_game_auto_upload` | `{ id, value }` | `AppConfig` |
| `upload_game_save` | `{ id }` | `AppConfig` |
| `list_game_versions` | `{ id }` | `VersionInfo[]` |
| `restore_game_version` | `{ id, versionId }` | `null` |
| `gdrive_connect` | — | `AppConfig` |
| `gdrive_disconnect` | — | `AppConfig` |
| `custom_cloud_login` | `{ endpoint, username, password }` | `AppConfig` |
| `custom_cloud_logout` | — | `AppConfig` |

Command chyby sú reťazce určené na priame zobrazenie v UI.

## Eventy

| Event | Payload | Význam |
|---|---|---|
| `config-changed` | `AppConfig` | Čerstvý uložený stav konfigurácie |
| `sync-progress` | `SyncProgress` | Priebeh, dokončenie alebo chyba operácie |

`SyncProgress.status` je `uploading`, `done` alebo `error`. Hodnota `uploading` znamená všeobecne prebiehajúcu cloudovú operáciu a používa sa aj pri downloade.
