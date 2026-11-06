# Backup Sync v0.5.0

Herná knižnica má nový dizajn kartičiek a obrázky načítava automaticky z verejného katalógu Steam. Prihlasovanie ani API kľúč netreba.

## Novinky

- Prepracované kartičky v mriežke aj zozname s oddelenými údajmi o lokálnej a cloudovej zálohe.
- Automatický výber obalu pri jednoznačnej zhode názvu hry.
- Nastavenia kartičky obsahujú tlačidlo **Vybrať obrázok**. Až po kliknutí sa načíta galéria obalov, pozadí a screenshotov.
- Ručne vybraný obrázok sa uloží ku konkrétnej hre a zostane dostupný aj po reštarte.
- Stiahnuté obrázky sa ukladajú lokálne a zobrazujú sa aj bez internetu.
- Aktualizované knižnice pre sieťovú komunikáciu, TLS a ZIP archívy.

## Obrázky hier

Automatické vyhľadanie používa názov hry v aplikácii. Pri nejasnej zhode otvor **Nastavenia kartičky / Vybrať obrázok** a vyhľadaj oficiálny názov. Obrázky sú dostupné pre hry z verejného katalógu Steam.

## Inštalácia

Stiahni priložený x64 setup EXE a spusti ho. Existujúcu verziu môžeš aktualizovať priamo týmto installerom. Súbor `.sha256` obsahuje kontrolný súčet inštalačky.

Installer nemá Authenticode podpis. Windows SmartScreen môže pri spustení zobraziť upozornenie.

## Kontroly vydania

- Produkčný frontend build, 30 Rust testov a Clippy s `-D warnings`.
- Rust audit hlási 0 známych zraniteľností. Zostáva 10 upstream upozornení na neudržiavané balíky a Linux GLib; GLib sa do Windows aplikácie nekompiluje.
- Produkčné npm závislosti majú 0 známych zraniteľností.
