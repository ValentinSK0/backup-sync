Backup Sync now detects game-save changes inside nested folders, including new Stardew Valley farms. It checks content when the app starts and every 30 seconds, so changes made while it was closed are found after restart.

- Save snapshots are stored as verified local ZIP files in the selected backup directory. Cloud uploads use the same ZIP, and a cloud failure leaves the local copy intact.
- The Games page shows local changes, local backups, and cloud uploads separately. It also offers list and grid views.
- The local ZIP location is chosen when adding a game or connecting one from the cloud.
- Updated `rustls` to address RUSTSEC-2026-0285.

**Existing games:** Check that the selected local save is the one you want, then use **Nahrať na cloud** once. Older cloud versions have no content fingerprint, so this first manual upload establishes the baseline for automatic uploads.

Validation: 21 Rust tests, Clippy with warnings denied, frontend build, and Windows NSIS build passed. `npm audit --omit=dev` and `cargo audit` found no vulnerabilities; Cargo still reports warnings for some indirect dependencies. Authenticated cloud uploads and a live game session were not exercised during automated validation.

Windows installer SHA-256: `d25f713a9ad6915ee72fb6fec121dec24edc112bc68159e566cd274d24d7d3cb`
