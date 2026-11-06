# Changelog

## 0.5.1 — 2026-11-06

- Tlačidlo „Stiahnuť najnovšiu verziu“ priamo na kartičke hry, keď cloud obsahuje novší save.
- Pred obnovou sa znovu načítajú cloudové verzie a aktuálny lokálny save sa ochranne zazálohuje.
- História zobrazuje čitateľné dátumy, najnovšie zálohy navrchu a označenie „Najnovšia“.
- Pri chýbajúcom dátume z cloudu sa použije čas v názve ZIPu vrátane starších názvov a názvov s UUID.
- Dátumy s rôznymi časovými pásmami sa porovnávajú podľa skutočného času.
- Opravné aktualizácie vývojových závislostí vrátane Vite.

## 0.5.0 — 2026-11-06

- Nový dizajn herných kartičiek v mriežke aj zozname s prehľadnejšími stavmi záloh.
- Automatické obrázky hier z verejného katalógu Steam bez prihlasovania a API kľúča.
- Výber vlastného obalu, pozadia alebo screenshotu v nastaveniach hry.
- Galéria sa načíta až po kliknutí na „Vybrať obrázok“ a vybraný obrázok zostáva uložený aj po reštarte.
- Lokálna cache obrázkov pre zobrazenie bez internetu a ochrana pred nesprávnym priradením hry.
- Aktualizované závislosti pre TLS, ZIP archívy a sieťovú komunikáciu.

## 0.1.1 — 2026-07-16

- Opravené Google OAuth prihlásenie pre klientov vyžadujúcich Client Secret.
- Pridané pole Client Secret do Nastavení.
- Client Secret sa ukladá iba do Windows Credential Manager, nikdy do konfigurácie.
- Client Secret sa posiela pri prvom prihlásení aj obnove Google tokenu.

## 0.1.0 — 2026-07-16

- Prvé verejné Windows vydanie.
- Inkrementálne zálohy priečinkov na Google Drive a vlastný cloud.
- Verziované ZIP zálohy herných save súborov a ich obnova.
- Pripojenie existujúcej priečinkovej zálohy na novom PC pomocou stabilného cloud ID.
- Individuálne nastavenie automatiky: dediť globálne, zapnuté alebo vypnuté.
- Retry po chybe uploadu alebo downloadu.
- OAuth PKCE + `state`, tokeny vo Windows Credential Manager a minimálny Drive scope.
- Streamovaný upload/download, bezpečnejšia obnova ciest a ZIP ochrany.
- Atómové ukladanie konfigurácie so záložnou kópiou.
- Offline WebView2 v NSIS installeri.
