import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AppConfig,
  CloudFolderBackup,
  CloudGameBackup,
  GameCloudStatus,
  Provider,
  Settings,
  VersionInfo,
} from "./types";

export function getConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("get_config");
}

export function updateSettings(settings: Settings): Promise<AppConfig> {
  return invoke<AppConfig>("update_settings", { settings });
}

export function saveGdriveCredentials(
  clientId: string,
  clientSecret: string,
): Promise<AppConfig> {
  return invoke<AppConfig>("save_gdrive_credentials", {
    clientId,
    clientSecret,
  });
}

export function addWatchedFolder(
  path: string,
  provider: Provider,
): Promise<AppConfig> {
  return invoke<AppConfig>("add_watched_folder", { path, provider });
}

export function removeWatchedFolder(id: string): Promise<AppConfig> {
  return invoke<AppConfig>("remove_watched_folder", { id });
}

export function setFolderAutoBackup(
  id: string,
  value: boolean | null,
): Promise<AppConfig> {
  return invoke<AppConfig>("set_folder_auto_backup", { id, value });
}

export function addGame(
  name: string,
  savePath: string,
  provider: Provider,
): Promise<AppConfig> {
  return invoke<AppConfig>("add_game", { name, savePath, provider });
}

export function removeGame(id: string): Promise<AppConfig> {
  return invoke<AppConfig>("remove_game", { id });
}

export function updateGame(
  id: string,
  name: string,
  savePath: string,
  backupPath: string,
  provider: Provider,
): Promise<AppConfig> {
  return invoke<AppConfig>("update_game", {
    id,
    name,
    savePath,
    backupPath,
    provider,
  });
}

export function setGameAutoUpload(
  id: string,
  value: boolean | null,
): Promise<AppConfig> {
  return invoke<AppConfig>("set_game_auto_upload", { id, value });
}

export function backupFolderNow(id: string): Promise<AppConfig> {
  return invoke<AppConfig>("backup_folder_now", { id });
}

export function listFolderBackups(
  provider: Provider,
): Promise<CloudFolderBackup[]> {
  return invoke<CloudFolderBackup[]>("list_folder_backups", { provider });
}

export function connectFolderBackup(
  provider: Provider,
  cloudId: string,
  path: string,
): Promise<AppConfig> {
  return invoke<AppConfig>("connect_folder_backup", {
    provider,
    cloudId,
    path,
  });
}

export function restoreFolderFromCloud(id: string): Promise<AppConfig> {
  return invoke<AppConfig>("restore_folder_from_cloud", { id });
}

export function uploadGameSave(id: string): Promise<AppConfig> {
  return invoke<AppConfig>("upload_game_save", { id });
}

export function listGameVersions(id: string): Promise<VersionInfo[]> {
  return invoke<VersionInfo[]>("list_game_versions", { id });
}

export function listGameCloudStatuses(): Promise<GameCloudStatus[]> {
  return invoke<GameCloudStatus[]>("list_game_cloud_statuses");
}

export function restoreGameVersion(
  id: string,
  versionId: string,
): Promise<null> {
  return invoke<null>("restore_game_version", { id, versionId });
}

/** Otvorí sledovaný priečinok (save hry / zálohovaný priečinok) v prieskumníkovi. */
export function openLocalFolder(path: string): Promise<null> {
  return invoke<null>("open_local_folder", { path });
}

/** Zoznam všetkých herných záloh na zvolenom cloude (na napojenie na tomto PC). */
export function listGameBackups(
  provider: Provider,
): Promise<CloudGameBackup[]> {
  return invoke<CloudGameBackup[]>("list_game_backups", { provider });
}

/** Stiahne najnovšiu verziu hry z cloudu do zvoleného priečinka a pridá ju. */
export function connectGameBackup(
  provider: Provider,
  cloudId: string,
  path: string,
  name: string,
): Promise<AppConfig> {
  return invoke<AppConfig>("connect_game_backup", {
    provider,
    cloudId,
    path,
    name,
  });
}

export function gdriveConnect(): Promise<AppConfig> {
  return invoke<AppConfig>("gdrive_connect");
}

export function gdriveDisconnect(): Promise<AppConfig> {
  return invoke<AppConfig>("gdrive_disconnect");
}

export function customCloudLogin(
  endpoint: string,
  username: string,
  password: string,
): Promise<AppConfig> {
  return invoke<AppConfig>("custom_cloud_login", {
    endpoint,
    username,
    password,
  });
}

export function customCloudLogout(): Promise<AppConfig> {
  return invoke<AppConfig>("custom_cloud_logout");
}

/** Prihlásenie do Ulož.to (login + aplikačný token; API kľúč voliteľný). */
export function uloztoLogin(
  login: string,
  authToken: string,
  appToken: string,
): Promise<AppConfig> {
  return invoke<AppConfig>("ulozto_login", { login, authToken, appToken });
}

export function uloztoLogout(): Promise<AppConfig> {
  return invoke<AppConfig>("ulozto_logout");
}

/** Otvorí natívny dialóg pre výber priečinka. Vráti cestu alebo null (zrušené). */
export async function pickFolder(): Promise<string | null> {
  const result = await open({ directory: true, multiple: false });
  return typeof result === "string" ? result : null;
}
