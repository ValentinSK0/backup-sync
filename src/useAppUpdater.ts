import { useCallback, useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { checkAppUpdate, downloadAppUpdate, installAppUpdate } from "./api";
import { errMsg } from "./format";
import type { AppUpdateInfo, AppUpdateProgress } from "./types";

export function useAppUpdater() {
  const [currentVersion, setCurrentVersion] = useState("");
  const [info, setInfo] = useState<AppUpdateInfo | null>(null);
  const [phase, setPhase] = useState<"idle" | "checking" | "downloading" | "installing">("idle");
  const [progress, setProgress] = useState<AppUpdateProgress>({ downloaded: 0, total: null });
  const [error, setError] = useState<string | null>(null);
  const busy = useRef(false);

  const check = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    setPhase("checking");
    setError(null);
    try {
      const result = await checkAppUpdate();
      setCurrentVersion(result.currentVersion);
      setInfo(result);
    } catch (error) {
      setError(errMsg(error));
    } finally {
      busy.current = false;
      setPhase("idle");
    }
  }, []);

  useEffect(() => {
    void getVersion().then(setCurrentVersion).catch(() => {});
    const startup = window.setTimeout(() => void check(), 1500);
    const periodic = window.setInterval(() => void check(), 6 * 60 * 60 * 1000);
    return () => {
      window.clearTimeout(startup);
      window.clearInterval(periodic);
    };
  }, [check]);

  async function download() {
    if (busy.current) return;
    busy.current = true;
    setPhase("downloading");
    setError(null);
    setProgress({ downloaded: 0, total: null });
    try {
      setInfo(await downloadAppUpdate(setProgress));
    } catch (error) {
      setError(errMsg(error));
    } finally {
      busy.current = false;
      setPhase("idle");
    }
  }

  async function install() {
    if (busy.current) return;
    busy.current = true;
    setPhase("installing");
    setError(null);
    try {
      await installAppUpdate();
    } catch (error) {
      setError(errMsg(error));
    } finally {
      busy.current = false;
      setPhase("idle");
    }
  }

  return { currentVersion, info, phase, progress, error, check, download, install };
}

export type AppUpdater = ReturnType<typeof useAppUpdater>;
