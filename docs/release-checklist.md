# Release checklist

Use this checklist for a new Windows release of the current code. Record the actual results for the release being prepared. Counts, audit results, signatures, and installer hashes must come from that run.

## 1. Confirm scope and version

- Review `git status` and the full diff. Exclude local configuration, credentials, saves, backups, preview fixtures, and build output from commits.
- Fetch the target branch and check for upstream changes before publication.
- Choose an unused version and update `package.json`, the root package entries in `package-lock.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.
- Let Cargo update the application package version in `src-tauri/Cargo.lock` and review that change.
- Update release notes and the changelog to describe the final code and actual validation.
- Check that documentation describes both current behavior and any difference between `main` and the packaged release.

## 2. Run validation

From the repository root, run these checks sequentially.

```powershell
npm ci
npm audit
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo audit --file src-tauri/Cargo.lock
npm run build
git diff --check
```

Install `cargo-audit` separately if it is unavailable. Investigate vulnerabilities and record remaining upstream warnings accurately. An ignored live test has not run as part of the default test suite.

Run public artwork tests when artwork or networking changes warrant them. [Artwork verification](artwork.md)

Use disposable data for manual acceptance of local snapshots, cloud upload, version detection, restoration, and cloud reconnection. Include relevant folder-backup checks when that path changes. [Backup acceptance checks](game-save-verification.md)

## 3. Prepare Google OAuth configuration

If the release supplies a default Google Client ID, set `BACKUP_SYNC_GOOGLE_CLIENT_ID` in the build process before compilation. Read it from the intended release configuration without logging credential values.

- Use an OAuth client of type Desktop app and enable the Google Drive API for its project.
- Check that the OAuth project's publishing status and allowed users fit the release audience.
- Supply only the public Client ID through the build environment. The user enters the matching Client Secret in the app, which stores it in Windows Credential Manager.
- Keep OAuth credentials, refresh tokens, and personal config files out of source control and release attachments.

A successful build does not verify that an OAuth project is configured correctly or that sign-in works for every account.

## 4. Build the Windows installer

Use the Windows Rust/MSVC and Tauri build prerequisites described in the [project README](../README.md).

```powershell
npm run tauri build -- --bundles nsis
```

The build runs the frontend production build and produces `BackupSync.exe` plus the NSIS setup under `src-tauri/target/release/bundle/nsis/`.

Verify the packaging configuration against `tauri.conf.json`.

- x64 Windows NSIS installer
- Current-user installation
- Offline WebView2 installer
- Expected product name, identifier, icon, and version
- Release executable DLL dependencies and Authenticode status

Test in a disposable Windows user or VM, including first launch, upgrade, app-data persistence, tray exit, and a restart. Redirecting only `APPDATA` does not isolate Windows Credential Manager. Avoid using real saves or production accounts for this check.

If a clean-install or authenticated-cloud check could not run, state that in the release notes. Unsigned installers can trigger Windows SmartScreen.

## 5. Freeze the artifact and checksum

Copy the final installer to an ignored release directory with a stable asset name, for example `Backup.Sync_<version>_x64-setup.exe`. Confirm the copied bytes match the build output.

Generate a SHA-256 sidecar containing the lowercase hash and asset filename.

```powershell
$releaseInstaller = "release-artifacts/<version>/Backup.Sync_<version>_x64-setup.exe"
$releaseHash = (Get-FileHash -LiteralPath $releaseInstaller -Algorithm SHA256).Hash.ToLowerInvariant()
$releaseName = Split-Path -Leaf $releaseInstaller
"$releaseHash  $releaseName" | Set-Content -LiteralPath "$releaseInstaller.sha256" -Encoding ascii
```

Replace the placeholders with the actual version and artifact paths before running the examples in this guide. Record the installer size, version, signature status, and hash. A SHA-256 checksum verifies file identity and integrity.

## 6. Commit, tag, and upload a draft

Commit the reviewed release changes to the intended branch, create an annotated `vX.Y.Z` tag at that commit, and push the branch and tag. Verify that both identify the intended source revision. Use normal pushes.

Create the GitHub release as a draft and require the tag to exist.

```powershell
gh release create vX.Y.Z <installer.exe> <installer.exe.sha256> --verify-tag --draft --title "Backup Sync vX.Y.Z" --notes-file <release-notes.md>
```

The release notes should explain user-visible changes, installation, actual test coverage, and material limitations. Attach the installer and checksum. Verify asset names, byte sizes, and upload completion.

## 7. Verify downloads and publish

1. Download the installer and checksum from the draft release to a separate verification directory.
2. Compare the downloaded installer hash with the local installer, downloaded checksum, and GitHub asset digest where available.
3. Publish the release and mark it latest.

```powershell
gh release edit vX.Y.Z --draft=false --latest
```

4. Verify the canonical release URL, tag, target commit, published state, asset links, and the repository's latest-release endpoint.
5. Confirm the local working tree and remote branch are in the expected state.

A draft can use a temporary URL. Use the final `/releases/tag/vX.Y.Z` URL after publication. Rebuilding after publication changes the artifact identity and requires a new verification and publication decision.
