# Release notes

The latest packaged release is **v0.5.2**, published on November 9, 2026. These notes describe that installer. Changes added afterward are listed under Unreleased in the changelog. [Changelog](CHANGELOG.md)

## Backup Sync v0.5.2

The dashboard presents backup status, pending work, and recent activity in a redesigned layout.

### Changes

- A synchronization summary shows actual item counts, errors, pending changes, and the last recorded synchronization time.
- Navigation cards open the game library, watched folders, and cloud settings.
- An attention panel groups errors and items waiting for synchronization.
- An activity timeline shows operations received during the current session.
- A cloud panel shows provider connection status.
- Responsive layouts support the minimum 900 × 620 application window and provide distinct empty and synchronization states.

The installer includes the earlier game-card redesign, public Steam artwork, and latest-cloud-version restore action from v0.5.0 and v0.5.1.

### Install

Download `Backup.Sync_0.5.2_x64-setup.exe` and its `.sha256` sidecar from the release page. The installer supports updating an existing installation and includes the offline WebView2 runtime. It installs for the current Windows user.

The installer is unsigned. Windows SmartScreen may show a warning. Compare the downloaded file with the published checksum.

```text
2979bd4874a958576ff43cddbb015be346b3dd268cca44bbb5d5788fc0b85a67
```

### Validation recorded for v0.5.2

- TypeScript and Vite production build passed.
- Rust tests recorded 33 passes and one ignored live-network test.
- Clippy passed with warnings denied.
- npm audit reported zero known vulnerabilities at release time. Cargo audit completed with 10 upstream warnings.
- Dashboard layout, navigation, long names, empty states, and synchronization states were checked using preview data.
- Clean-profile installation and authenticated cloud transfer were outside that release's recorded validation.

### Current source development

The `main` branch includes Wikipedia artwork fallback and selection for games outside Steam, together with the English documentation updates. Build from `main` to use those changes until a later installer includes them.

Current test counts and audit findings belong to their individual runs. Consult the technical guides for the current behavior and verification commands. [Documentation](docs/README.md)

[Download v0.5.2](https://github.com/ValentinSK0/backup-sync/releases/tag/v0.5.2) · [Latest release](https://github.com/ValentinSK0/backup-sync/releases/latest)
