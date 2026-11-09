export type Provider = "gdrive" | "customCloud" | "ulozto";
export type SyncState = "idle" | "changed" | "syncing" | "synced" | "error";
export type CompressionLevel = "fast" | "best";

export interface WatchedFolder {
  id: string;
  path: string;
  provider: Provider;
  cloudId: string | null; // stabilné ID priečinka u storage providera
  cloudName: string; // názov vzdialenej zálohy
  autoBackup: boolean | null; // null = dedí globálne nastavenie
  syncState: SyncState;
  lastSync: string | null; // ISO-8601
  lastError: string | null;
  lastErrorAction: "upload" | "download" | null;
}

export interface Game {
  id: string;
  name: string;
  savePath: string;
  cloudName: string;
  backupPath: string;
  provider: Provider;
  autoUpload: boolean | null; // null = dedí globálne nastavenie
  syncState: SyncState;
  lastChange: string | null;
  lastUpload: string | null;
  lastCloudVersionId: string | null;
  lastError: string | null;
  local: {
    observedHash: string | null;
    backupHash: string | null;
    uploadedHash: string | null;
    lastBackup: string | null;
    backupFile: string | null;
    lastCheck: string | null;
    watchError: string | null;
  };
  artwork: GameArtwork | null;
  artworkCheckedName: string | null;
}

export interface GameArtwork {
  source: ArtworkSource | "steamgriddb";
  gameId: number;
  gameName: string;
  imageId: number;
  kind: "grid" | "hero";
  url: string;
  author: string;
  selectedManually: boolean;
}

export type ArtworkSource = "steam" | "wikipedia";

export interface ArtworkGame {
  id: number;
  name: string;
  source: ArtworkSource;
}

export interface ArtworkSearchResult {
  games: ArtworkGame[];
  warnings: string[];
}

export interface ArtworkImage {
  id: number;
  url: string;
  thumb: string;
  width: number;
  height: number;
  author: { name: string };
}

export interface GameCloudStatus {
  id: string;
  updateAvailable: boolean;
  latestAt: string | null;
}

export interface ProviderStatus {
  connected: boolean;
  account: string | null; // označenie účtu / používateľ
  endpoint: string | null; // len customCloud
  error: string | null;
}

export interface Settings {
  theme: "dark" | "light";
  debounceSeconds: number;
  autoBackupFolders: boolean; // globálne automatické zálohovanie priečinkov
  autoUploadSaves: boolean; // globálny slider zap/vyp
  compressionLevel: CompressionLevel;
  gdriveClientId: string; // OAuth client ID (Desktop app) z Google Cloud Console
  gdriveClientSecretConfigured: boolean; // hodnota ostáva vo Windows Credential Manager
}

export interface AppConfig {
  watchedFolders: WatchedFolder[];
  games: Game[];
  providers: {
    gdrive: ProviderStatus;
    customCloud: ProviderStatus;
    ulozto: ProviderStatus;
  };
  settings: Settings;
}

export interface VersionInfo {
  id: string; // ID súboru na cloude
  name: string; // napr. Elden-Ring_2026-07-16_14-30-00.zip
  createdAt: string | null; // ISO-8601
  sizeBytes: number;
}

export interface CloudFolderBackup {
  cloudId: string;
  name: string;
  modifiedAt: string | null;
  fileCount: number;
  sizeBytes: number;
  alreadyConnected: boolean;
}

export interface CloudGameBackup {
  cloudId: string;
  name: string; // čitateľný názov hry z cloudu
  latestAt: string | null;
  versionCount: number;
  sizeBytes: number;
  alreadyConnected: boolean;
}

export interface SyncProgress {
  targetType: "folder" | "game";
  id: string;
  status: "uploading" | "done" | "error";
  message: string | null;
}

export interface AppUpdateInfo {
  currentVersion: string;
  feedAvailable: boolean;
  update: {
    version: string;
    notes: string | null;
    downloaded: boolean;
  } | null;
}

export interface AppUpdateProgress {
  downloaded: number;
  total: number | null;
}
