export type Provider = "gdrive" | "customCloud";
export type SyncState = "idle" | "changed" | "syncing" | "synced" | "error";
export type CompressionLevel = "fast" | "best";

export interface WatchedFolder {
  id: string;
  path: string;
  provider: Provider;
  syncState: SyncState;
  lastSync: string | null; // ISO-8601
  lastError: string | null;
}

export interface Game {
  id: string;
  name: string;
  savePath: string;
  provider: Provider;
  autoUpload: boolean | null; // null = dedí globálne nastavenie
  syncState: SyncState;
  lastChange: string | null;
  lastUpload: string | null;
  lastError: string | null;
}

export interface ProviderStatus {
  connected: boolean;
  account: string | null; // e-mail / užívateľ
  endpoint: string | null; // len customCloud
  error: string | null;
}

export interface Settings {
  theme: "dark" | "light";
  debounceSeconds: number;
  autoUploadSaves: boolean; // globálny slider zap/vyp
  compressionLevel: CompressionLevel;
  gdriveClientId: string; // OAuth client ID (Desktop app) z Google Cloud Console
  gdriveClientSecret: string; // OAuth client secret (Google ho pri Desktop app vyžaduje)
}

export interface AppConfig {
  watchedFolders: WatchedFolder[];
  games: Game[];
  providers: { gdrive: ProviderStatus; customCloud: ProviderStatus };
  settings: Settings;
}

export interface VersionInfo {
  id: string; // ID súboru na cloude
  name: string; // napr. Elden-Ring_2026-07-16_14-30-00.zip
  createdAt: string; // ISO-8601
  sizeBytes: number;
}

export interface SyncProgress {
  targetType: "folder" | "game";
  id: string;
  status: "uploading" | "done" | "error";
  message: string | null;
}
