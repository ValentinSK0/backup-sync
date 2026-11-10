# Backup Sync v0.5.4

Published on November 10, 2026. This release adds optional background operation, redesigned folder cards and a focused code cleanup with connection reuse improvements.

## Changes

- Added a tray and background-mode switch in Settings. It is off by default, including when an older configuration has no preference. With the switch off, closing the window exits the app. With it on, closing hides the window and monitoring continues. The choice is saved locally on each device and applies immediately.
- Redesigned folder cards to match the game library, using folder icons, backup states, last-backup dates, automation controls and cloud actions. Clicking a folder path opens it in Explorer.
- Reused the purple application icon in the sidebar.
- Reused HTTP connection pools for cloud operations and artwork requests. Game cloud-status refreshes reuse one provider instance per cloud, retaining its access-token and session caches for that refresh.
- Kept file watchers running when settings unrelated to their timing change.
- Fixed unsaved Settings drafts being reset by background configuration updates or a tray change.
- Removed unused starter images, an empty stylesheet, the obsolete sidebar logo and the unused JavaScript opener package. Folder opening and Google sign-in continue through the Rust opener plugin.

## Installation and updates

Download `Backup.Sync_0.5.4_x64-setup.exe` for Windows x64, or use the update panel in an updater-capable installation such as v0.5.3. Versions up to v0.5.2 require a manual installation to gain the updater.

The installer reuses WebView2 when available. If WebView2 is missing, its bootstrapper downloads the runtime from Microsoft and needs internet access. The installer and updater feed are signed with the existing updater key. The installer has no Windows Authenticode signature, so SmartScreen may display a warning.

This release includes the installer, its `.sig` and `.sha256` sidecars, and `latest.json`.

## Verification

- Reviewed the frontend, Rust IPC handlers, configuration, synchronization, monitoring, archive protection, storage adapters, artwork and updater paths. A fresh structural graph indexed 52 code files. Unused-code reports were cross-checked against IPC registration and maintenance entry points.
- Clean npm installation and the TypeScript/Vite production build passed. Runtime dependency and export checks found no unused frontend dependencies or exports. The standalone icon-rendering maintenance script remains available.
- The default Rust suite passed 44 tests with two live-network tests excluded. Both live Steam and Wikipedia tests were also run successfully. Tests include archive integrity, configuration recovery, legacy configuration, tray preference persistence, cloud-version ordering, restore protection and the custom-cloud protocol against a local test server.
- A regression test verified reuse of one HTTP connection with distinct authorization headers on successive requests.
- Clippy passed with warnings denied. npm audit found zero vulnerabilities. Cargo audit found zero vulnerabilities and 10 upstream warnings.
- An isolated native Windows build verified the new sidebar icon and folder layout, local snapshots for two games, exact ZIP contents after a same-size edit with its timestamp preserved, unsaved Settings drafts during background events, tray hiding, single-instance reopening and normal exit with tray disabled.
- Release artifacts were checked for matching checksums, updater signatures, signed version and manifest URLs before publication.

Authenticated production-cloud transfers, a new full installer upgrade cycle and first-time setup on a clean Windows machine without WebView2 were outside this release's completed validation.

[Download v0.5.4](https://github.com/ValentinSK0/backup-sync/releases/tag/v0.5.4)
