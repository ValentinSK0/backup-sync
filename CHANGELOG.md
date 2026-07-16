# Changelog

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
