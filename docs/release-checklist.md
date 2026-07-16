# Release checklist

## Kód

```powershell
npm ci
npm audit --omit=dev
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo audit --file src-tauri/Cargo.lock
npm run build
```

## Google Cloud

- Google Drive API je zapnuté.
- OAuth Client je typu **Desktop app**.
- Consent screen je pre verejných používateľov v stave **In production**.
- Build dostane `BACKUP_SYNC_GOOGLE_CLIENT_ID`.
- Client Secret sa do zdrojov ani binárky nevkladá; používateľ ho zadá v aplikácii a uloží sa do Windows Credential Manager.

## Windows bundle

- NSIS target.
- `currentUser` install mode.
- Offline WebView2 installer.
- Spustenie release EXE s čistým `%APPDATA%`.
- Kontrola PE DLL importov a SHA-256.
- Kontrola Authenticode podpisu.

Bez dôveryhodného code-signing certifikátu bude výsledok funkčný, ale SmartScreen môže zobraziť varovanie.

## GitHub

- Commit všetkých release zmien.
- Tag `vX.Y.Z`.
- Release notes a setup EXE.
- Súbor so SHA-256 hashom.
- Po uploade znovu stiahnuť asset a porovnať hash.
