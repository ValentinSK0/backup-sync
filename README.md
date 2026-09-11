# Backup Sync

Windows aplikácia na ručné alebo automatické zálohovanie priečinkov a herných save súborov na Google Drive, Ulož.to Disk alebo vlastný cloud.

## Inštalácia

1. V časti [Releases](https://github.com/ValentinSK0/backup-sync/releases) stiahni súbor `Backup-Sync_*_x64-setup.exe`.
2. Spusti ho a dokonči inštaláciu.
3. Otvor **Backup Sync** zo Štartu.

Nie je potrebné inštalovať Node.js, Rust, Visual C++ Redistributable ani WebView2. Windows installer obsahuje offline WebView2 runtime a inštaluje aplikáciu iba pre aktuálneho používateľa, takže nevyžaduje administrátorské práva.

Požiadavky: 64-bit Windows 10 alebo Windows 11 a internetové pripojenie pre cloudové operácie.

> Aktuálny installer nie je podpísaný plateným code-signing certifikátom. Windows SmartScreen preto môže pri prvom spustení zobraziť varovanie „Windows protected your PC“. Skontroluj, že súbor pochádza z tohto GitHub repozitára a že sedí zverejnený SHA-256 hash.

## Čo aplikácia vie

- Inkrementálne zálohovať zvolené priečinky — nahrávajú sa iba zmenené súbory.
- Automaticky sledovať zmeny alebo zálohovať ručne.
- Zabaliť save priečinok hry do ZIP archívu a uchovávať viac verzií.
- Obnoviť vybranú verziu herného save.
- Zálohovať na výber medzi Google Drive, Ulož.to Diskom a vlastným cloudom — cloud sa volí pri každej hre a priečinku.
- Prehliadať herné aj priečinkové zálohy na zvolenom cloude a stiahnuť ktorúkoľvek z nich do vybraného priečinka na novom PC.
- Otvoriť priečinok so save súbormi priamo z karty hry.
- Po chybe zopakovať upload alebo download bez odstraňovania položky.
- Uložiť prihlasovacie tokeny vo Windows Credential Manager.
- Bežať na pozadí v systémovej lište: zatvorenie okna appku neukončí, sledovanie beží ďalej.

## Systémová lišta (tray)

- Zatvorenie okna krížikom appku iba schová do lišty — sledovanie a automatické zálohy bežia ďalej.
- Ľavý klik na ikonu v lište otvorí okno; pravý klik ponúkne **Otvoriť Backup Sync** a **Ukončiť**.
- Appku naozaj vypneš iba cez **Ukončiť** v lište.
- Druhé spustenie appky neotvorí novú inštanciu, len zobrazí okno bežiacej.

## Globálne / zapnuté / vypnuté

Každý priečinok a hra má vlastné nastavenie automatického zálohovania:

- **Dediť globálne** — používa hlavný prepínač z Nastavení.
- **Zapnuté** — automatika je pre túto položku vždy zapnutá.
- **Vypnuté** — automatika je pre túto položku vždy vypnutá.

Nová záloha pripojená z cloudu má automatiku zámerne vypnutú, aby sa hneď po obnove omylom neprepísal cloud.

## Prenos priečinka na iný PC

1. Na novom PC sa prihlás do rovnakého Google účtu.
2. Otvor **Priečinky → Pripojiť z cloudu**.
3. Vyber existujúcu cloud zálohu.
4. Vyber lokálny cieľový priečinok.
5. Klikni **Stiahnuť a pripojiť**.

Aplikácia uloží rovnaké cloud ID. Obe PC potom pracujú s rovnakou priečinkovou zálohou.

## Prenos hry na iný PC

1. Na novom PC sa prihlás do rovnakého Google účtu.
2. Otvor **Hry → Vybrať z cloudu**.
3. Vyber hru zo zoznamu záloh — vidíš dátum poslednej verzie, počet verzií aj veľkosť.
4. Skontroluj názov hry a vyber priečinok na tomto PC, kam sa má hra synchronizovať.
5. Klikni **Stiahnuť a napojiť**.

Stiahne sa najnovšia verzia save a hra sa pridá medzi sledované. Automatické nahrávanie je pre novo napojenú hru zámerne vypnuté.
Ak vybraný lokálny priečinok už obsahuje save súbory, aplikácia ich najprv uloží do ZIPu v
`Backup Sync local saves` vedľa vybraného priečinka. Cloud save sa potom rozbalí priamo do
vybraného priečinka, bez pridania ďalšieho vnoreného priečinka.

## Ulož.to Disk

Tretí podporovaný cloud. V **Nastavenia → Ulož.to Disk** zadaj svoj **login** a **aplikačný token**
(vygeneruješ na webe Ulož.to v Nastaveniach — používa sa namiesto hesla). Voliteľne vlož vlastný
**API kľúč aplikácie** (žiada sa od podpory Ulož.to); bez neho sa použije verejný testovací kľúč
z dokumentácie, ktorý sa môže kedykoľvek zmeniť.

Upload beží cez oficiálne resumable API (bloky po 4 MB, prerušený prenos sa opakuje),
sťahovanie cez rýchly odkaz pre vlastné súbory. Tokeny sa ukladajú iba do Windows Credential
Manager. Pri hrách aj priečinkoch stačí pri pridávaní vybrať cloud **Ulož.to Disk**.

## Google Drive

Oficiálny release môže mať OAuth Client ID vložený už pri builde. V Nastaveniach treba zadať aj Client Secret z rovnakého OAuth klienta. Pri vlastnom builde vytvor v Google Cloud Console OAuth 2.0 Client ID typu **Desktop app**, zapni Google Drive API a nastav:

```powershell
$env:BACKUP_SYNC_GOOGLE_CLIENT_ID="123456789.apps.googleusercontent.com"
```

Aplikácia používa PKCE, kontrolu OAuth `state` a iba scope `drive.file`. Client Secret ukladá iba vo Windows Credential Manager; nikdy nie v `config.json`.

Pre verejný release musí byť OAuth consent screen v Google Cloud nastavený na **In production**. V režime **Testing** fungujú iba testovací používatelia a refresh token môže po siedmich dňoch vypršať.

## Dôležité limity v0.4.1

- Aplikácia zatiaľ automaticky nerozhoduje, či je novšia lokálna alebo cloud verzia, ani nerieši konflikt zmien z dvoch PC. Smer operácie vyberá používateľ tlačidlom **Zálohovať** alebo **Stiahnuť**.
- Priečinková záloha prepíše súbor s rovnakou cestou, ale nemaže súbory, ktoré zostali iba na cloude. Obnova prepíše rovnaké lokálne súbory a ostatné lokálne súbory ponechá.
- Hry sa na cloude zatiaľ párujú podľa upraveného názvu hry. Na druhom PC zadaj rovnaký názov; premenovanie vytvorí nový cloud priečinok a rovnaké názvy môžu kolidovať.
- Vlastný cloud je pokročilá funkcia a vyžaduje kompatibilný server podľa [API kontraktu](docs/custom-cloud-api.md).

## Vývoj a build

```powershell
npm ci
npm run tauri dev
npm run tauri build
```

Produkčný NSIS installer vznikne v `src-tauri/target/release/bundle/nsis/`.

Podrobný postup pred vydaním je v [release checkliste](docs/release-checklist.md).
