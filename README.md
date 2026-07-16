# Backup Sync

Desktopová aplikácia (Tauri 2 + React) na zálohovanie priečinkov a herných save súborov medzi počítačmi.

## Funkcie
- Sledovanie zvolených priečinkov a automatická inkrementálna záloha na Google Drive
- Tracker herných save súborov: zmena sa zvýrazní, upload jedným klikom alebo automaticky (prepínač v Nastaveniach)
- Kompresia saveov do zip archívu pred uploadom (šetrí dáta aj miesto)
- Verzovanie saveov + obnova ľubovoľnej verzie
- Vlastný cloud pre save súbory (viď docs/custom-cloud-api.md)
- Tokeny bezpečne vo Windows Credential Manager

## Vývoj
```
npm install
npm run tauri dev
```

## Build
```
npm run tauri build
```

## Google Drive
V Google Cloud Console vytvor OAuth 2.0 Client ID typu **Desktop app**, zapni Drive API,
a vlož Client ID v aplikácii do Nastavenia → Google Drive.
