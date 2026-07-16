# Backup Sync v0.1.1

Rýchla opravná verzia Google OAuth prihlásenia.

## Oprava

- Nastavenia znovu obsahujú pole **OAuth Client Secret**.
- Secret sa bezpečne ukladá do Windows Credential Manager, nie do `config.json`.
- Aplikácia ho posiela Googlu pri prihlásení aj obnove tokenu.
- Pred pripojením treba uložiť Client ID aj Client Secret.

## Inštalácia

Stiahni x64 setup EXE priložený k release a spusti ho. Verziu 0.1.0 môžeš priamo aktualizovať týmto installerom.

Installer zatiaľ nie je podpísaný plateným code-signing certifikátom, preto môže Windows SmartScreen zobraziť varovanie. Over SHA-256 hash priložený k release.
