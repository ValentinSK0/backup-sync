# Custom cloud API

This contract describes the HTTP requests made by the current custom storage provider. A server implementing these endpoints can store both versioned game ZIPs and individual folder files.

The implementation is in [custom.rs](../src-tauri/src/storage/custom.rs). Provider-independent interfaces are in [storage/mod.rs](../src-tauri/src/storage/mod.rs).

## Endpoint configuration

Enter a base endpoint such as `https://backup.example.com` in the application. The client appends `/api/login` or `/api/files` to that endpoint. A base path is supported, for example `https://example.com/backups`.

The client accepts HTTPS endpoints and HTTP endpoints on loopback hosts for local development. It rejects embedded usernames or passwords, query strings, and fragments. Trailing slashes are normalized.

All file requests carry `Authorization: Bearer <token>`. Login sends the supplied username and password as JSON. The returned token is stored in Windows Credential Manager.

## Login

```http
POST /api/login
Content-Type: application/json

{"username":"example-user","password":"example-password"}
```

Return a successful HTTP status with a JSON object containing a nonempty token.

```json
{"token":"example-session-token"}
```

The client rejects empty tokens and malformed responses. Failed HTTP login responses are shown as a login error. There is no custom-provider token refresh protocol. When a token expires, the user must sign in again.

## File endpoints

| Method | Route | Request | Successful response |
| --- | --- | --- | --- |
| `PUT` | `/api/files/{path}` | Raw file bytes | Any 2xx status |
| `GET` | `/api/files/{path}` | Bearer token | 2xx with raw file bytes |
| `DELETE` | `/api/files/{path}` | Bearer token | Any 2xx status |
| `GET` | `/api/files?prefix={prefix}` | Bearer token | 2xx with a JSON array |

Uploads use `Content-Type: application/octet-stream` and `Content-Length`. Uploads and downloads stream file bytes. Upload and delete response bodies are ignored.

A failed file request is returned to the UI as an operation-specific error. The client has no dedicated 401 refresh or retry flow for this provider.

### Paths and identifiers

A file ID is its full logical path, including the filename. Path segments are URL-encoded individually by the client. Preserve their decoded names and hierarchy.

The current application uses these roots.

```text
BackupSync/
  folders/<stable-folder-name>/<relative-file-path>
  games/<stable-game-cloud-name>/<version-filename>.zip
```

Game cloud names are persisted separately from display names, so renaming a game in the UI keeps its existing cloud directory. Folder cloud IDs are paths in this provider.

Game version filenames contain a sanitized display name, local timestamp, and UUID, for example `Example-Game_2026-11-09_12-30-00-123_<uuid>.zip`. Treat the entire filename as an opaque name.

### Upload semantics

`PUT` creates a file or replaces the file at the same path. The server must create missing parent directories as needed. Folder creation is implicit because the client has no HTTP endpoint for creating an empty directory.

Return success only when the complete file is available to future listings and downloads. A server should stage incomplete uploads and publish them atomically.

### Listing semantics

Return all files beneath the requested prefix, including nested files. The client has no pagination protocol, so each response must be complete.

```http
GET /api/files?prefix=BackupSync%2Fgames%2FExample-Game%2F
Authorization: Bearer example-session-token
```

```json
[
  {
    "path": "BackupSync/games/Example-Game/Example-Game_2026-11-09_12-30-00-123.zip",
    "size": 2450000,
    "createdAt": "2026-11-09T11:30:00Z"
  }
]
```

| Field | Requirement |
| --- | --- |
| `path` | Required string containing the full logical path |
| `size` | File size in bytes, defaulting to zero if omitted |
| `createdAt` | Optional RFC3339 timestamp or `null` |

Return an empty array for an empty prefix. Directory records are unnecessary. The client infers subdirectories from file paths, so empty directories do not appear in backup discovery.

Supply valid timestamps where possible. Game-version sorting can fall back to timestamps embedded in recognized ZIP filenames. Unknown dates remain available for manual version selection.

## Backup behavior

Folder uploads replace changed paths and retain files that exist only on the server. Downloads replace matching local files and retain other local files. The current interface has no retention or automatic deletion control, although the storage adapter implements `DELETE`.

Each uploaded game ZIP has a distinct version filename. Keep those names intact so the application can list, select, and restore versions.

## Server implementation requirements

Authenticate every file request and isolate each account's storage namespace. Validate decoded paths, prevent traversal outside the storage root, and enforce your own quotas and upload limits. These checks belong on the server even though the desktop app validates paths during restoration.

Use a disposable server account to verify login, replacement uploads, nested listings, Unicode names, spaces, downloads, and authentication failures before relying on a deployment for backups.
