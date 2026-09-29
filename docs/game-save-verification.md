# Game save regression checks

Version 0.4.3 adds persistent content fingerprints, periodic reconciliation and verified local snapshots before cloud upload.

Automated coverage (`cargo test --manifest-path src-tauri/Cargo.toml`):

- Nested file changes with identical size and preserved modification time.
- New farms, renamed files and deleted files.
- Recursive Windows filesystem notifications for an existing nested farm.
- ZIP contents matching the complete source tree, including multiple farms.
- Rejection and cleanup when the source changes before a snapshot is published.
- Waiting for a game to finish writing before publishing its snapshot.
- Rejection of backup destinations inside the save directory and missing save roots.
- Persistent baselines across restart, changed content while the app was closed, and suppression of duplicate automatic uploads.
- Legacy configuration migration and the required first manual upload for an existing cloud version without a fingerprint.
- Local ZIP preservation when cloud setup fails and rejection of corrupted cached ZIPs.
- Existing restore protections, configured backup locations and path traversal rejection.

Manual acceptance on a disposable save folder:

1. Create `Saves/Farm1/save` and `Saves/Farm2/nested/save`. Add the game and select a separate backup directory.
2. Disable automatic cloud upload. Edit Farm2. Confirm a local ZIP appears under `<backup directory>/<game name>-<game ID>` and contains both farms. Cloud upload time must remain unchanged.
3. Exit the app completely, edit a file and restart. Confirm the startup scan creates a new local snapshot. Also verify the 30-second fallback by creating a save root that was unavailable at startup.
4. With a test cloud account, upload once and enable automatic cloud upload. Edit a farm. Confirm a new cloud version appears and the open Versions dialog refreshes.
5. Disconnect the cloud and edit a farm. Confirm the local ZIP remains readable and the cloud error is visible. Reconnect and retry; confirm the existing verified ZIP is reused if the save has not changed.
6. Change the save during a slow upload. The completed upload must not mark the newer local content as synchronized; another pass must back up that content.
7. Restore a test cloud version while local-only files exist. Confirm a protective ZIP is created at the selected backup location and remaining local-only files keep the content state different from the cloud snapshot.

Automated tests use temporary folders and a disconnected provider. They do not validate a live Stardew Valley session or authenticated cloud uploads. Local snapshots are retained without automatic cleanup. Content stability checks cannot prove game-level consistency if a game pauses for a long interval between writes; close the game before important manual restores.
