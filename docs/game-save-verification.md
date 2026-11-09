# Backup behavior and verification

This guide describes the current backup and restore implementation and the checks used to validate it. Start with the [application README](../README.md) for installation and everyday use.

## Games and folders use different backup formats

| Item | Local behavior | Cloud behavior |
| --- | --- | --- |
| Game | Verified ZIP snapshots in the selected backup directory | Separate ZIP files representing uploaded versions |
| Watched folder | Content hashes track previously uploaded files | Changed files are uploaded individually |

Folder backup skips temporary and lock-file names defined by `is_ignored_file`. Game content scans and game ZIP creation include regular files throughout the save tree and exclude symbolic links.

Implementation references are [sync.rs](../src-tauri/src/sync.rs), [game_save.rs](../src-tauri/src/game_save.rs), and [archive.rs](../src-tauri/src/archive.rs).

## Game monitoring and snapshots

1. A recursive native watcher observes save-directory changes.
2. A startup reconciliation and a check every 30 seconds also inspect game contents, including when native watching is unavailable.
3. A snapshot records relative file paths and SHA-256 content hashes. Additions, renames, deletions, and same-size edits affect its fingerprint.
4. The app reuses the latest local ZIP only if its stored fingerprint, location, and actual archive contents still match.
5. Otherwise, it waits at least `max(debounceSeconds, 2)` seconds and checks that the source has settled.
6. A new archive is written as a `.partial` file. The archive and source tree must both match the expected snapshot before it is renamed to `.zip`.

The stable-backup operation attempts up to three passes. Continued writes produce an error and later monitoring checks can try again. Empty or unavailable save directories prevent a new upload.

A backup destination inside the save directory is rejected. Normal snapshots use this layout.

```text
<selected backup directory>/
  <sanitized game name>-<game ID>/
    save_<local timestamp>_<UUID>.zip
```

The app retains old snapshots until the user removes them.

## Cloud uploads and status

Automatic local snapshots run independently of the game's automatic cloud-upload setting. A manual upload explicitly requests cloud transfer. A manual local backup performs local snapshot work without uploading.

For automatic uploads, the app compares the current fingerprint with the last uploaded or restored cloud fingerprint. A legacy game with an existing cloud version but no content baseline requires one explicit upload first.

The same verified ZIP is used for cloud transfer. A cloud failure leaves that local file available. After upload, another scan determines whether local contents still match the uploaded version. Changes made during transfer remain marked as changed.

Operations are guarded per target. A monitoring check arriving during a game operation records a pending change. Native watcher events and periodic reconciliation provide later checks.

| Field | Meaning |
| --- | --- |
| `lastChange` | Time the app first observed a different content fingerprint |
| `local.lastCheck` | Most recent content observation |
| `local.lastBackup` | Time a new normal local snapshot was recorded |
| `local.backupFile` / `local.backupHash` | Latest normal local ZIP and its content fingerprint |
| `local.uploadedHash` | Fingerprint of the successfully uploaded or restored cloud snapshot |
| `lastUpload` | Last recorded upload or restore time |
| `lastCloudVersionId` | Last known uploaded or restored cloud version ID |
| `local.watchError` | Native monitoring problem reported separately from transfer errors |

## Cloud versions

Cloud history contains uploaded ZIP versions. Local-only ZIPs are accessed through the selected backup directory.

The app sorts ZIPs by parsed provider timestamps, then by recognized timestamps in filenames when needed. It compares actual timestamp values across time zones. Historical filename timestamps have no zone and are interpreted in the current PC's local zone. Versions with unknown dates remain in the history.

The newer-version indicator checks the latest version against the stored version ID and last upload or restore time. For dated versions, the normal time comparison uses a five-second tolerance. A failed provider listing does not produce a newer-version indicator, so its absence alone does not confirm cloud availability.

The latest-version restore command refreshes the listing and rechecks the comparison. If it cannot determine the latest backup's date, it asks the user to select a version manually.

## Restore protection and limits

Close the game before restoring.

Existing regular save files receive a verified protective ZIP before a cloud restore or connection proceeds. This ZIP is written directly to the configured backup directory with a `local-before-cloud` filename. Older configurations with no backup path use a sibling `Backup Sync local saves` directory.

The downloaded ZIP is extracted into a temporary staging directory before files are copied to the save destination. Archive extraction rejects unsafe paths and limits archives to 100,000 entries and 100 GiB of declared uncompressed data.

Restoration overwrites matching files and keeps local-only files. The app compares the restored cloud fingerprint with the resulting local contents. Extra files can leave the game marked as changed. A protective ZIP can be used for recovery, but copying files into the destination is not a transactional whole-directory swap.

Folder restoration also stages downloads and validates relative paths, then replaces matching files and retains other local files. The protective game-save ZIP behavior applies to games.

## Automated checks

Run the current tests and static checks from the repository root.

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
npm run build
```

The backend suite covers nested content changes, stable snapshots, corrupted ZIP reuse, baselines across restarts, legacy migration, restore protection, path validation, and cloud-version ordering. Windows builds also include a recursive filesystem-notification test.

Test totals change as the suite evolves. Use each run's output for the current count. Tests using temporary directories, mock HTTP servers, or a disconnected provider do not establish that an authenticated cloud restore or a live game's write sequence succeeds.

## Manual acceptance with disposable data

1. Create `Saves/Farm1/save` and `Saves/Farm2/nested/save`. Add the game and choose a separate backup directory.
2. Leave cloud upload off, edit Farm2, and confirm that a local ZIP contains both farms without changing the cloud upload time.
3. Exit fully through the tray, edit a save, and restart. Confirm that reconciliation creates a new snapshot. Also test a save directory that becomes available after startup.
4. Upload once to a disposable cloud account, enable automatic uploads, and edit a save. Confirm that a new cloud version appears.
5. Make the cloud unavailable and edit again. Confirm that a readable local ZIP remains and the transfer error is visible. Reconnect and retry with unchanged content.
6. Change the save during a slow upload. Confirm that the newer content remains marked as changed and is backed up on a later pass.
7. Restore a test version with local-only files present. Confirm the protective ZIP location, the restored contents, and the resulting changed state.
8. Upload a newer version from another test PC. Confirm the card's latest-version action, its confirmation, and manual access to an older version.

Record which checks actually ran when preparing a release. Content stability between scans cannot establish game-level consistency when a game pauses between related writes.
