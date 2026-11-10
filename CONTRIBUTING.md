# Contributing to Backup Sync

Backup Sync is a Windows desktop application built with React, TypeScript and Tauri 2. Contributions can include fixes, focused improvements, documentation and reproducible bug reports.

## Discuss a change

Use the issue forms to describe a bug or a feature that would improve an actual backup workflow. Include the app version and Windows version when reporting a problem. Search existing issues first and keep each report focused on one problem.

For security vulnerabilities, follow the private reporting guidance in the [security policy](.github/SECURITY.md).

## Run locally

Install Node.js 20.19+ or 22.12+, Rust with the Windows MSVC toolchain, Microsoft C++ Build Tools, the Windows SDK and WebView2. The [README](README.md#development) describes the application setup.

```powershell
npm ci
npm run tauri dev
```

Use disposable directories and test saves while developing backup or restore changes. Cloud integration checks require the corresponding provider configuration.

## Prepare a pull request

- Explain the problem and the resulting behavior.
- Keep the change focused and preserve compatibility with existing configuration and backups.
- Update the relevant English documentation when behavior changes.
- Include the checks you actually ran and any validation that still needs an account or environment you could not test.

Run the core checks before submitting code changes.

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
npm run build
git diff --check
```

The default test suite excludes live artwork tests. When changing catalogue networking, run those separately.

```powershell
cargo test --manifest-path src-tauri/Cargo.toml live_ -- --ignored --nocapture
```

Release packaging requires the maintainer's updater signing key. The [release checklist](docs/release-checklist.md) covers installer builds and publication.
