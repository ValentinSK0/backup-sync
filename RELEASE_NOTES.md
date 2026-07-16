# Backup Sync v0.1.0

Prvé verejné Windows vydanie aplikácie Backup Sync.

## Hlavné funkcie

- Zálohovanie priečinkov a herných save súborov na Google Drive.
- Automatické sledovanie zmien alebo ručné spustenie.
- Verzie herných save a obnova zvolenej verzie.
- Pripojenie existujúcej priečinkovej zálohy na inom PC.
- Tlačidlá na opakovanie operácie po chybe.
- Offline installer — používateľ nemusí doinštalovať WebView2, Node.js, Rust ani Visual C++ runtime.

## Inštalácia

Stiahni x64 setup EXE priložený k release a spusti ho. Aplikácia sa nainštaluje iba pre aktuálneho používateľa bez potreby administrátorských práv.

Installer zatiaľ nie je podpísaný plateným code-signing certifikátom, preto môže Windows SmartScreen zobraziť varovanie. Over SHA-256 hash priložený k release.

## Známe limity

- Bez automatického porovnania „cloud novší / lokál novší“ a bez riešenia konfliktov medzi dvoma PC.
- Folder sync nemaže cloudové súbory, ktoré boli lokálne odstránené.
- Hry sa párujú podľa upraveného názvu hry, nie stabilného game ID.
