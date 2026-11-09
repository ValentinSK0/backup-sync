# Application updates

The Windows app checks the public GitHub release feed 1.5 seconds after the interface mounts and every six hours while the process is running. **Settings** (`Nastavenia`) also provides a manual check. A banner on other pages links to the update panel when a newer version is available.

## User flow

1. The app compares its version with `latest.json` on the latest published GitHub release.
2. The user selects **Download update** (`Stiahnuť aktualizáciu`). Progress appears in the update panel. Backups can continue during this download.
3. The Rust updater verifies the downloaded installer against the embedded public signing key. The signed version must match the version announced in the feed.
4. The user selects **Install and restart** (`Nainštalovať a reštartovať`). If a backup, restore, cloud-backup connection, or guarded item change is active, installation is rejected and the verified download remains ready for another attempt.
5. Installation reserves the same synchronization lock used by backups and restores, preventing a new transfer from starting before exit. The updater starts NSIS in passive mode, closes the current process and asks the installer to relaunch the app afterward.

Downloaded installers stay in memory until installation or app exit. A restart before installation requires downloading the update again. The app installs versions newer than its current version. Checks and downloads require internet access and public release assets, with no GitHub login or user token.

The endpoint is `https://github.com/ValentinSK0/backup-sync/releases/latest/download/latest.json`. Releases made before updater support have no feed asset. The interface reports that update packages are unavailable instead of claiming the app is up to date. Network and verification errors remain visible with retry controls.

## Signing key

The public key is configured in `src-tauri/tauri.conf.json`. The private key used for this project is stored outside the checkout at `%LOCALAPPDATA%\BackupSync\signing\updater.key`. Keep a secure backup of that key. Future releases must use the matching key so installed clients can verify them.

The release script uses that path by default. `TAURI_SIGNING_PRIVATE_KEY` can supply another key file or key value. `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` supplies its password when required. Keep both values out of logs, commits and release attachments. The current local key has an empty password and depends on Windows account access protection.

On a new maintainer machine, restore the existing key. Generate a new key only when deliberately bootstrapping a new signing identity. Replacing the configured public key alone cannot update clients that still trust the old key.

Updater signing is separate from Windows Authenticode signing. A valid updater signature does not remove Windows SmartScreen warnings from an unsigned executable.

## Build and publish

Synchronize the application version in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`, then run the checks in the [release checklist](release-checklist.md).

```powershell
npm run build:release -- --notes-file path/to/release-notes.md
```

Without `--notes-file`, the manifest uses a short generic release description. The script runs the Tauri Windows NSIS build with signing enabled and creates these files under `release-artifacts/v<version>/`.

| Asset | Purpose |
| --- | --- |
| `Backup.Sync_<version>_x64-setup.exe` | Installer used for manual installation and in-app updates |
| `Backup.Sync_<version>_x64-setup.exe.sig` | Tauri updater signature |
| `Backup.Sync_<version>_x64-setup.exe.sha256` | Checksum for download verification |
| `latest.json` | Version, release notes, publication date, Windows x64 URL and signature |

Upload all four files to the matching `v<version>` GitHub release. Verify the installer, signature, checksum and manifest before marking that release latest. The manifest URL must resolve to the exact signed installer bytes. Include `latest.json` on every later stable release, because the app follows GitHub's latest-release redirect.

Clients predating updater support need one manual installation of an updater-capable version. Subsequent releases can be installed through Settings.

## Verification

- Test an equal or older remote version, a newer version, a missing feed, an offline check and a failed download.
- Verify that altered installer bytes and a mismatched signed version are rejected before installation.
- Check that navigation preserves download progress and a downloaded update remains ready after a rejected install attempt.
- During a backup or restore, attempt installation and verify that the operation continues while installation is rejected.
- In a disposable Windows account or VM, update an installed older updater-capable build. Confirm app exit, setup completion, relaunch and persistence of app configuration and backup files.

The implementation is in `src-tauri/src/updater.rs`, the synchronization gate in `src-tauri/src/sync.rs`, frontend state in `src/useAppUpdater.ts`, and the panel in `src/components/AppUpdateCard.tsx`.
