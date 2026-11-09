# Frontend and backend IPC

The React frontend uses Tauri `invoke()` for commands and `listen()` for events. Command names use snake_case. Argument names and serialized model fields use camelCase.

The registered command list is in [lib.rs](../src-tauri/src/lib.rs). Frontend wrappers are in [api.ts](../src/api.ts), and frontend models are in [types.ts](../src/types.ts). Rust models and serializers define the actual wire payload.

## Shared values and state

```ts
type Provider = "gdrive" | "customCloud" | "ulozto";
type SyncState = "idle" | "changed" | "syncing" | "synced" | "error";
type CompressionLevel = "fast" | "best";
type ArtworkSource = "steam" | "wikipedia";
```

`AppConfig` contains `watchedFolders`, `games`, provider statuses for all three providers, and `settings`.

| Model or field | Meaning |
| --- | --- |
| `WatchedFolder` | Local path, provider, stable cloud ID/name, automation override, synchronization state, last sync, and last error/action |
| `Game` | Display name, save path, local backup path, stable cloud name, provider, upload override, state, last change/upload/cloud version, local snapshot state, and artwork |
| `Game.local` | Observed, backup, and uploaded fingerprints plus last backup/check times, backup filename, and watcher error |
| `Settings` | Theme, debounce seconds, global folder/game automation, compression, public Google Client ID, and Client Secret configured flag |
| `ProviderStatus` | Connected flag, account label, endpoint where applicable, and provider error |
| `GameCloudStatus` | Game ID, newer-version flag, and latest known cloud timestamp |
| `VersionInfo` | Cloud file ID, filename, nullable `createdAt`, and `sizeBytes` |
| `CloudFolderBackup` | Cloud ID, name, nullable modified date, file count, total bytes, and already-connected flag |
| `CloudGameBackup` | Cloud ID, name, nullable latest date, version count, total bytes, and already-connected flag |

`autoBackup` and `autoUpload` are `boolean | null`. A null value inherits the global setting. For games the override affects cloud uploads, while local snapshot checks continue.

The default global automation flags are false. `debounceSeconds` accepts integers from 1 through 3600. Normal settings updates preserve the stored Google credentials. Change OAuth configuration through `save_gdrive_credentials`.

Secrets are not returned in `AppConfig`. The Client Secret configured flag reports availability, and provider status can include an account label.

## Commands

Arguments below are JSON objects. Empty arguments mean the command has no frontend-supplied arguments. All returns are the resolved value of the invocation promise.

### Configuration and provider connections

| Command | Arguments | Return |
| --- | --- | --- |
| `get_config` | None | `AppConfig` |
| `update_settings` | `{ settings: Settings }` | `AppConfig` |
| `save_gdrive_credentials` | `{ clientId: string, clientSecret: string }` | `AppConfig` |
| `gdrive_connect` | None | `AppConfig` |
| `gdrive_disconnect` | None | `AppConfig` |
| `ulozto_login` | `{ login: string, authToken: string, appToken: string }` | `AppConfig` |
| `ulozto_logout` | None | `AppConfig` |
| `custom_cloud_login` | `{ endpoint: string, username: string, password: string }` | `AppConfig` |
| `custom_cloud_logout` | None | `AppConfig` |

An empty Google `clientSecret` retains the stored secret only when the Client ID is unchanged and a secret already exists. Changing credentials clears the Google connection and refresh token. Credential changes and provider disconnection can be rejected while transfers are active.

An empty Ulož.to `appToken` selects the source's public test application key. The user's `authToken` is the application login token.

### Watched folders

| Command | Arguments | Return |
| --- | --- | --- |
| `add_watched_folder` | `{ path: string, provider: Provider }` | `AppConfig` |
| `remove_watched_folder` | `{ id: string }` | `AppConfig` |
| `set_folder_auto_backup` | `{ id: string, value: boolean \| null }` | `AppConfig` |
| `backup_folder_now` | `{ id: string }` | `AppConfig` |
| `list_folder_backups` | `{ provider: Provider }` | `CloudFolderBackup[]` |
| `connect_folder_backup` | `{ provider: Provider, cloudId: string, path: string }` | `AppConfig` |
| `restore_folder_from_cloud` | `{ id: string }` | `AppConfig` |

Connecting a cloud folder downloads it and records its stable cloud ID. The new item starts with automatic backup disabled.

### Games and versions

| Command | Arguments | Return |
| --- | --- | --- |
| `add_game` | `{ name: string, savePath: string, provider: Provider, backupPath: string }` | `AppConfig` |
| `update_game` | `{ id: string, name: string, savePath: string, backupPath: string, provider: Provider }` | `AppConfig` |
| `remove_game` | `{ id: string }` | `AppConfig` |
| `set_game_auto_upload` | `{ id: string, value: boolean \| null }` | `AppConfig` |
| `backup_game_local` | `{ id: string }` | `AppConfig` |
| `upload_game_save` | `{ id: string }` | `AppConfig` |
| `list_game_versions` | `{ id: string }` | `VersionInfo[]` |
| `list_game_cloud_statuses` | None | `GameCloudStatus[]` |
| `restore_game_version` | `{ id: string, versionId: string }` | `null` |
| `restore_latest_game_version` | `{ id: string }` | `AppConfig` |
| `list_game_backups` | `{ provider: Provider }` | `CloudGameBackup[]` |
| `connect_game_backup` | `{ provider: Provider, cloudId: string, path: string, name: string, backupPath: string }` | `AppConfig` |

`backupPath` is the directory for local ZIP backups. It must be outside the save directory. Connecting an existing cloud game restores it and starts with automatic upload disabled.

Version listing filters ZIPs and sorts the newest dated versions first. Date values may be derived from recognized filenames. A failed cloud-status lookup produces no newer-version flag for that game.

Restore confirmation belongs to the caller. The UI asks the user to close the game and confirm replacement before invoking restore. Both restore commands use the backend's protective local-backup path. The latest-version command also refreshes and validates the cloud selection.

### Artwork

| Command | Arguments | Return |
| --- | --- | --- |
| `search_artwork_games` | `{ query: string }` | `ArtworkSearchResult` |
| `list_artwork_images` | `{ gameId: number, kind: "grid" \| "hero", page: number, source?: ArtworkSource }` | `ArtworkImage[]` |
| `load_game_artwork` | `{ id: string, automatic: boolean }` | Data URL string or `null` |
| `select_game_artwork` | `{ id: string, gameId: number, kind: "grid" \| "hero", page: number, imageId: number, source?: ArtworkSource }` | `AppConfig` |
| `clear_game_artwork` | `{ id: string }` | `AppConfig` |

Search accepts a nonempty title of at most 200 characters. The backend defaults an omitted or null artwork `source` to `steam`. Frontend wrappers send the source explicitly. Catalogue IDs must always be paired with their source.

The current artwork response shapes are shown below.

```ts
interface ArtworkGame {
  id: number;
  name: string;
  source: ArtworkSource;
}

interface ArtworkSearchResult {
  games: ArtworkGame[];
  warnings: string[];
}

interface ArtworkImage {
  id: number;
  url: string;
  thumb: string;
  width: number;
  height: number;
  author: { name: string };
}
```

The Rust image payload does not include a score, although the current frontend interface still declares one. Consumers must use the fields actually supplied by the backend. Image dimensions can be zero when the catalogue does not provide them.

Persisted `GameArtwork` also records source, catalogue game ID/name, image ID, category, URL, credit, and `selectedManually`. Legacy saved selections can have source `steamgriddb`. New catalogue search accepts the two current providers.

The full lookup and cache behavior is documented in the [artwork guide](artwork.md).

### Local folder access

| Command | Arguments | Return |
| --- | --- | --- |
| `open_local_folder` | `{ path: string }` | `null` |

The backend opens existing directories that match a configured watched folder, game save path, or game backup path. It rejects untracked locations.

The frontend's `pickFolder()` helper uses the Tauri dialog plugin directly. It is not an application command in the registered Rust handler list.

## Events

| Event | Payload | Behavior |
| --- | --- | --- |
| `config-changed` | `AppConfig` | Complete saved configuration snapshot |
| `sync-progress` | `SyncProgress` | Transfer or local-backup progress, completion, or failure |

```ts
interface SyncProgress {
  targetType: "folder" | "game";
  id: string;
  status: "uploading" | "done" | "error";
  message: string | null;
}
```

The `uploading` value is a general in-progress signal. It is also used for downloads and local snapshot work. The message describes the active step.

`config-changed` is emitted after the new configuration is saved and installed as the in-memory snapshot. The UI also applies returned configurations from successful commands. The frontend creates activity timestamps when progress arrives and retains up to 20 entries for the session.

## Error handling and persistence

Commands reject with user-facing strings, currently in Slovak. Use the shared `errMsg` formatter when displaying failures.

A failed operation may have already created a local ZIP, updated progress, or persisted successful upload metadata before a later step failed. Callers should use the updated configuration and events instead of assuming every failure leaves state unchanged.

Configuration has migration defaults for older files. Startup fills missing cloud names and backup locations, converts interrupted `syncing` states to `changed`, and migrates legacy Google secrets into credential storage.
