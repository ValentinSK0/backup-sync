# Backup Sync v0.5.1

Ak cloud obsahuje novší save hry, kartička ponúkne **Stiahnuť najnovšiu verziu**. Po potvrdení aplikácia sama vyberie a obnoví najnovšiu zálohu.

## Zmeny

- Priame stiahnutie najnovšieho save z kartičky hry.
- Opätovná kontrola cloudových verzií pri stiahnutí a ochranná záloha aktuálneho save pred obnovou.
- Po obnove sa aktualizuje stav kartičky a upozornenie na obnovenú verziu zmizne.
- História obsahuje čitateľné dátumy a najnovšiu verziu navrchu s označením **Najnovšia**.
- Ak cloud nevráti dátum, aplikácia ho odvodí z názvu ZIPu. Podporuje staršie názvy aj názvy s UUID.
- Dátumy s rôznymi časovými pásmami sa správne zoradia podľa skutočného času.
- Opravné aktualizácie vývojových závislostí vrátane Vite.

## Inštalácia

Stiahni priložený x64 setup EXE a spusti ho. Existujúcu verziu môžeš aktualizovať týmto installerom. Súbor `.sha256` obsahuje kontrolný súčet inštalačky.

Installer nemá Authenticode podpis. Windows SmartScreen môže pri spustení zobraziť upozornenie.

## Overenie a obmedzenia

- Produkčný frontend build, 33 Rust testov a Clippy s `-D warnings`.
- npm audit hlási 0 známych zraniteľností. Rust audit hlási 0 známych zraniteľností a 10 upstream upozornení na neudržiavané balíky a Linux GLib. GLib sa do Windows aplikácie nekompiluje.
- Ovládanie priameho stiahnutia a aktualizácia kartičky overené na ukážkových dátach. Obnova skutočného save z používateľského cloudu ani inštalácia do čistého Windows profilu neboli testované.
- Staré ZIP názvy neobsahujú časové pásmo. Pri chýbajúcom dátume z cloudu sa ich čas interpretuje v miestnom pásme počítača.
- Ak dátum zálohy nemožno určiť, zostáva dostupná cez **Verzie** pre ručnú obnovu.
