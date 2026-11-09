# Backup Sync v0.5.3

Published on November 9, 2026. This release brings a smaller Windows installer, signed in-app updates, artwork for games outside Steam, and backup reliability fixes.

## Changes

- Reduced the Windows installer from about 222 MB to about 8.4 MB by embedding a small WebView2 bootstrapper and using zlib compression.
- Added automatic update checks at startup and every six hours. Settings provides download progress, release notes, signature verification, and an **Install and restart** action.
- Required updater signatures to cover the announced version. Installation is blocked while backups, restores, cloud-backup connections, or guarded item changes are active.
- Added public Wikipedia artwork lookup alongside Steam, with automatic fallback, manual selection, and local image caching. Artwork lookup needs no login or API key.
- Used consistent chronological ordering and filename-date fallback when reconnecting cloud games. After restoring an older save, the card can offer the newer cloud version again.
- Prevented game and folder changes from racing with synchronization. Cloud catalogue reads use existing directories without creating missing ones.
- Streamed Ulož.to downloads to disk and corrected custom-cloud direct listings and IPv6 loopback handling.
- Updated the English documentation and preserved the animated README banner.

## Installation and updates

Download `Backup.Sync_0.5.3_x64-setup.exe` and its `.sha256` checksum. The installer supports current-user Windows x64 installations and existing installation upgrades.

Versions up to v0.5.2 need one manual installation to gain the in-app updater. Later updater-capable releases can be downloaded and installed through Settings.

If WebView2 is already installed, setup reuses it. If it is missing, setup requires internet access to download the runtime from Microsoft. That additional download can be around 200 MB, depending on the runtime version.

The updater uses a cryptographic signature. Windows Authenticode signing is separate and the installer remains unsigned, so Windows SmartScreen may show a warning. Verify the downloaded installer with the published SHA-256 sidecar.

## Verification

- Clean dependency installation and the TypeScript/Vite production build passed.
- The default Rust suite passed 42 tests, with two live-network tests excluded from the default run. Both live Steam and Wikipedia tests were also run successfully.
- Clippy passed with warnings denied. npm audit reported zero vulnerabilities. Cargo audit reported zero vulnerabilities and 10 upstream warnings.
- Native Windows checks covered game creation, verified local ZIP contents, same-size edits with preserved timestamps, changes while the app was stopped, configuration persistence, window hiding, and single-instance reopening.
- Authenticated Ulož.to checks covered backup discovery, download into an isolated directory, older-version restoration, protective ZIP contents, the newer-version card action, and restoration of the latest save.
- An actual signed update between two isolated test builds completed through download, verification, installation, exit, and relaunch using a local test feed. Test configuration, saves, and ZIP backups were preserved.
- A silent test installation completed in 1.56 seconds on the test PC with WebView2 already installed. Installation times depend on the machine and runtime availability.
- Installer checksums, the public signing key, signed version, and update manifest were checked. Modified installer bytes and modified signed-version metadata were rejected.

Authenticated Google Drive upload and restore, cloud uploads to production accounts, and first-time installation on a clean Windows machine without WebView2 were outside this release's completed validation. The custom-cloud protocol was exercised against a local HTTP test server.

The release includes the installer, `.sig`, `.sha256`, and `latest.json` assets. [Download v0.5.3](https://github.com/ValentinSK0/backup-sync/releases/tag/v0.5.3)
