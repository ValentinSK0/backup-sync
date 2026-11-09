# Changelog

Changes are grouped by release. The Unreleased section describes work on `main` after the latest packaged release. For current behavior and setup, see the [README](README.md) and [documentation](docs/README.md).

## Unreleased

## 0.5.3 — 2026-11-09

- Kept Google Drive and Ulož.to catalogue reads from creating missing cloud directories, and streamed Ulož.to downloads to disk.

- Applied filename date fallback and chronological ordering when reconnecting cloud games, and compared newer saves with the restored cloud version when available.
- Guarded cloud-backup connections and game or folder changes against concurrent transfers and app-update installation.
- Accepted IPv6 loopback custom-cloud endpoints and aligned artwork image types with the backend payload.

- Added automatic GitHub release checks at startup and every six hours, plus a Settings panel for checking, downloading, and installing app updates.
- Added signed Windows updater artifacts, signed-version verification, release manifest generation, and installation exclusion while backups or restores are active.

- Reduced Windows installer payload by embedding the small WebView2 bootstrapper and using zlib compression. Installing a missing WebView2 runtime requires internet access.

- Added Wikipedia artwork lookup for games outside Steam, with automatic fallback and manual selection alongside Steam results.
- Added source-specific catalogue IDs, separate provider caches and retry deadlines, and local image caching for the new artwork source.
- Added an animated SVG banner to the README.
- Reworked the README and technical documentation in English, including the current IPC contract, backup behavior, artwork providers, custom cloud API, and release workflow.
- Published the repository with a public documentation entry point.

## 0.5.2 — 2026-11-09

- Redesigned the dashboard with a synchronization summary and an indicator based on actual item counts.
- Added navigation cards for games, folders, and cloud connections.
- Added an attention panel for errors and pending items, a session activity timeline, and provider connection status.
- Added responsive layouts and distinct content for empty, changed, syncing, synchronized, and error states.

## 0.5.1 — 2026-11-06

- Added a direct latest-version download action on game cards when a newer cloud save is detected.
- Refreshed the cloud listing before restoration and reused the protective local-save backup flow.
- Added readable version dates, newest-first ordering, and a latest-version label.
- Added date fallback from recognized ZIP filenames, including historical names and UUID suffixes.
- Compared timestamps as actual instants across time zones.
- Updated development dependencies, including Vite fixes.

## 0.5.0 — 2026-11-06

- Redesigned game cards in grid and list views with separate local and cloud backup information.
- Added automatic artwork from the public Steam catalogue without an artwork-service login or API key.
- Added manual selection of covers, backgrounds, and screenshots through the game settings.
- Loaded the image gallery after the user opens the picker and persisted the selected image across restarts.
- Added local image caching and checks against ambiguous automatic game matches.
- Updated networking, TLS, and ZIP dependencies.

## 0.4.3 — 2026-09-29

- Added content-based game-save monitoring at startup, after changes, and on a 30-second reconciliation interval.
- Added verified local ZIP snapshots and cloud uploads using the same verified archive.
- Preserved local snapshots after cloud failures and validated cached ZIP contents before reuse.
- Added configurable local backup locations when adding or connecting games.
- Separated local-change, local-backup, and cloud-upload information in the Games page.
- Added a required manual upload for legacy cloud versions without a content baseline.
- Updated rustls to address RUSTSEC-2026-0285.

## 0.4.2 — 2026-09-14

- Added game editing and a configurable local backup directory.
- Used the configured location for protective backups before game-save restoration.
- Added stable cloud mapping and detection of newer cloud save versions.

## 0.4.1 — 2026-09-11

- Backed up existing local saves to a ZIP before connecting a game from the cloud.
- Stored protective ZIPs beside the selected save directory in a local backup folder.
- Extracted cloud save contents directly into the selected destination without an extra parent directory.

## 0.4.0 — 2026-07-24

- Added Ulož.to Disk as a third storage provider.
- Added application-token login, chunked uploads, and reuse of provider sessions.
- Added provider selection when adding folders or browsing existing game and folder backups.

## 0.3.1 — 2026-07-23

- Refreshed the Windows shell icon cache after installation so shortcuts use the updated application icon.

## 0.3.0 — 2026-07-23

- Added the cloud-upload application logo.
- Added system-tray operation, including reopening the window and exiting through the tray menu.
- Added single-instance behavior so a second launch opens the running instance.

## 0.2.0 — 2026-07-21

- Added a browser for existing cloud game backups, including version count, latest date, and total size.
- Added connection of an existing cloud game backup to a local save directory on another PC.
- Added a game-card action for opening the save directory in File Explorer.
- Started newly connected cloud games with automatic uploads disabled.

## 0.1.1 — 2026-07-16

- Restored Google OAuth Client Secret entry for desktop clients that require it.
- Stored the Client Secret in Windows Credential Manager.
- Sent the Client Secret during initial token exchange and refresh.

## 0.1.0 — 2026-07-16

- Released the initial Windows application.
- Added incremental folder backups, versioned game-save ZIPs, and restoration.
- Added Google Drive and custom cloud storage adapters.
- Added connection of existing cloud folder backups using stable cloud IDs.
- Added per-item automation overrides and retry actions after failed transfers.
- Added OAuth PKCE, state validation, credential storage, streamed transfers, and restore path checks.
- Added atomic configuration saves with a backup copy.
- Packaged the app with an offline WebView2 installer.

Published installers and their original release records are available on [GitHub Releases](https://github.com/ValentinSK0/backup-sync/releases).
