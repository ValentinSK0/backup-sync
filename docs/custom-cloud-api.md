# Vlastný cloud — očakávané API

Aplikácia vie nahrávať herné save súbory (a voliteľne aj priečinky) na vlastný server.
Server musí implementovať toto malé REST API. Všetky chránené volania nesú hlavičku
`Authorization: Bearer <token>`.

Produkčný endpoint musí používať HTTPS. Nešifrované HTTP aplikácia povolí iba pre
`localhost`, `127.0.0.1` alebo inú loopback IP počas lokálneho vývoja.

## Prihlásenie

```
POST /api/login
Content-Type: application/json

{ "username": "valentin", "password": "..." }

→ 200 { "token": "..." }
→ 401 pri zlom hesle
```

Token si aplikácia uloží do Windows Credential Manager. Keď server vráti 401,
aplikácia zobrazí „Prihlás sa znova".

Token nesmie byť prázdny. Endpoint nesmie obsahovať prihlasovacie údaje, query ani fragment.

## Súbory

Cesta súboru je hierarchická, napr. `BackupSync/games/Elden-Ring/Elden-Ring_2026-07-16_14-30-00.zip`.

```
PUT    /api/files/{cesta}            telo = raw bytes  → 2xx
GET    /api/files/{cesta}            → 200 + raw bytes
DELETE /api/files/{cesta}            → 2xx
GET    /api/files?prefix={prefix}/   → 200 + JSON zoznam:
       [ { "path": "BackupSync/games/…/x.zip", "size": 12345, "createdAt": "2026-07-16T14:30:00Z" } ]
```

`createdAt` je voliteľné (ISO-8601). Zoznam vracia len súbory, ktorých cesta začína na `prefix`.
Upload a download sú streamované, takže server nemusí držať celý súbor v pamäti.

## Minimálna implementácia

Stačí ~100 riadkov (Node/Express, Python/FastAPI, Go…) nad diskom alebo S3.
Príklad úložiska: každý `path` = súbor na disku pod dátovým priečinkom servera.
