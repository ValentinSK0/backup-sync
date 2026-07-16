# IPC kontrakt (Rust ⇄ React)

Všetky payloady sú camelCase JSON. Volanie: `invoke("command_name", { args })` z `@tauri-apps/api/core`.
Eventy: `listen("event-name", handler)` z `@tauri-apps/api/event`.

## Typy (TypeScript)

```ts
export type Provider = "gdrive" | "customCloud";
export type SyncState = "idle" | "changed" | "syncing" | "synced" | "error";
export type CompressionLevel = "fast" | "best";

export interface WatchedFolder {
  id: string;
  path: string;
  provider: Provider;
  syncState: SyncState;
  lastSync: string | null;   // ISO-8601
  lastError: string | null;
}

export interface Game {
  id: string;
  name: string;
  savePath: string;
  provider: Provider;
  autoUpload: boolean | null; // null = dedí globálne nastavenie
  syncState: SyncState;
  lastChange: string | null;
  lastUpload: string | null;
  lastError: string | null;
}

export interface ProviderStatus {
  connected: boolean;
  account: string | null;   // e-mail / užívateľ
  endpoint: string | null;  // len customCloud
  error: string | null;
}

export interface Settings {
  theme: "dark" | "light";
  debounceSeconds: number;
  autoUploadSaves: boolean;      // globálny slider zap/vyp
  compressionLevel: CompressionLevel;
  gdriveClientId: string;        // OAuth client ID (Desktop app) z Google Cloud Console
  gdriveClientSecret: string;    // OAuth client secret (Google ho pri Desktop app vyžaduje aj s PKCE)
}

export interface AppConfig {
  watchedFolders: WatchedFolder[];
  games: Game[];
  providers: { gdrive: ProviderStatus; customCloud: ProviderStatus };
  settings: Settings;
}

export interface VersionInfo {
  id: string;        // ID súboru na cloude
  name: string;      // napr. Elden-Ring_2026-07-16_14-30-00.zip
  createdAt: string; // ISO-8601
  sizeBytes: number;
}

export interface SyncProgress {
  targetType: "folder" | "game";
  id: string;
  status: "uploading" | "done" | "error";
  message: string | null;
}
```

## Commands

| Command | Args | Návrat |
|---|---|---|
| `get_config` | — | `AppConfig` |
| `update_settings` | `{ settings: Settings }` | `AppConfig` |
| `add_watched_folder` | `{ path: string, provider: Provider }` | `AppConfig` |
| `remove_watched_folder` | `{ id: string }` | `AppConfig` |
| `add_game` | `{ name: string, savePath: string, provider: Provider }` | `AppConfig` |
| `remove_game` | `{ id: string }` | `AppConfig` |
| `set_game_auto_upload` | `{ id: string, value: boolean \| null }` | `AppConfig` |
| `backup_folder_now` | `{ id: string }` | `AppConfig` (stav po dobehnutí) |
| `upload_game_save` | `{ id: string }` | `AppConfig` |
| `list_game_versions` | `{ id: string }` | `VersionInfo[]` |
| `restore_game_version` | `{ id: string, versionId: string }` | `null` |
| `gdrive_connect` | — | `AppConfig` (otvorí prehliadač, čaká na OAuth) |
| `gdrive_disconnect` | — | `AppConfig` |
| `custom_cloud_login` | `{ endpoint: string, username: string, password: string }` | `AppConfig` |
| `custom_cloud_logout` | — | `AppConfig` |

Chyby: command vráti reject s reťazcom chyby (zobraz v UI, napr. toast/inline).

Výber priečinka: použiť `open({ directory: true })` z `@tauri-apps/plugin-dialog`.

## Eventy (Rust → React)

| Event | Payload | Kedy |
|---|---|---|
| `config-changed` | `AppConfig` | watcher zmenil syncState, dokončený upload, zmena configu na pozadí |
| `sync-progress` | `SyncProgress` | priebeh uploadu/chyba |

UI drží config v state; pri `config-changed` ho nahradí. Po každom command-e s návratom `AppConfig` tiež.
