# Documentation

These guides describe the current `main` branch. For features in a downloaded installer, use the notes attached to that release. The application interface currently uses Slovak labels.

## Find the right guide

| Guide | Purpose |
| --- | --- |
| [App overview and setup](../README.md) | Installation, first backup, provider setup, and everyday use |
| [Backup behavior and verification](game-save-verification.md) | Local snapshots, cloud versions, restore behavior, regression coverage, and manual acceptance checks |
| [Artwork providers](artwork.md) | Automatic lookup, manual image selection, caching, and provider limits |
| [Custom cloud API](custom-cloud-api.md) | REST endpoints and storage semantics required by a compatible server |
| [Frontend and backend IPC](ipc-contract.md) | Registered Tauri commands, JSON payloads, events, and client responsibilities |
| [Release checklist](release-checklist.md) | Validation, Windows packaging, checksums, and GitHub publication |
| [Application updates](app-updates.md) | In-app GitHub updates, signing keys, release assets, and installation checks |

## How the application fits together

The React frontend calls registered Tauri commands. Rust owns persisted configuration, file monitoring, backup creation, restore operations, and provider access. Configuration changes and progress events update the interface.

- Folder backups upload changed files individually.
- Game backups produce verified local ZIP snapshots and can upload them as separate cloud versions.
- Each game or folder selects one provider, with per-item overrides for automatic cloud operations.
- The dashboard summarizes existing state and the latest 20 session events. Its activity feed resets when the app restarts.
- Closing the main window keeps the process running in the system tray. Monitoring stops when the process exits.

## Source map

| Area | Implementation |
| --- | --- |
| Frontend calls and types | [api.ts](../src/api.ts), [types.ts](../src/types.ts) |
| Startup and command registration | [lib.rs](../src-tauri/src/lib.rs) |
| Configuration and migrations | [config.rs](../src-tauri/src/config.rs) |
| Command validation | [commands.rs](../src-tauri/src/commands.rs) |
| Monitoring | [watcher.rs](../src-tauri/src/watcher.rs) |
| Backup and restore coordination | [sync.rs](../src-tauri/src/sync.rs) |
| Content fingerprints and ZIP validation | [game_save.rs](../src-tauri/src/game_save.rs), [archive.rs](../src-tauri/src/archive.rs) |
| Storage implementations | [storage](../src-tauri/src/storage) |
| Artwork services | [artwork.rs](../src-tauri/src/artwork.rs), [Wikipedia adapter](../src-tauri/src/artwork/wikipedia.rs) |
| Application updates | [updater.rs](../src-tauri/src/updater.rs), [frontend state](../src/useAppUpdater.ts), [release builder](../scripts/build-release.mjs) |

## State and credentials

Tauri resolves the application configuration directory using the identifier `com.valentin.backup-sync`. On Windows this is normally `%APPDATA%\com.valentin.backup-sync`.

The directory contains `config.json`, its backup, and a `state` directory for hashes and cached artwork. Game ZIP backups use the separate locations chosen by the user.

Provider tokens and the Google OAuth Client Secret use Windows Credential Manager under the `backup-sync` service. Configuration includes paths, provider status, and the public Google Client ID. Treat configuration exports and diagnostic output as user data.

Historical release information is maintained in [CHANGELOG.md](../CHANGELOG.md) and [GitHub Releases](https://github.com/ValentinSK0/backup-sync/releases).
