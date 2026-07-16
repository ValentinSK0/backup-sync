import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { AppConfig, Provider, Settings, VersionInfo } from "./types";

export function getConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("get_config");
}

export function updateSettings(settings: Settings): Promise<AppConfig> {
  return invoke<AppConfig>("update_settings", { settings });
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

export function setGameAutoUpload(
  id: string,
  value: boolean | null,
): Promise<AppConfig> {
  return invoke<AppConfig>("set_game_auto_upload", { id, value });
}

export function backupFolderNow(id: string): Promise<AppConfig> {
  return invoke<AppConfig>("backup_folder_now", { id });
}

export function uploadGameSave(id: string): Promise<AppConfig> {
  return invoke<AppConfig>("upload_game_save", { id });
}

export function listGameVersions(id: string): Promise<VersionInfo[]> {
  return invoke<VersionInfo[]>("list_game_versions", { id });
}

export function restoreGameVersion(
  id: string,
  versionId: string,
): Promise<null> {
  return invoke<null>("restore_game_version", { id, versionId });
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

/** Otvorí natívny dialóg pre výber priečinka. Vráti cestu alebo null (zrušené). */
export async function pickFolder(): Promise<string | null> {
  const result = await open({ directory: true, multiple: false });
  return typeof result === "string" ? result : null;
}
