import { createContext, useContext } from "react";
import type { AppConfig } from "./types";
import type { AppUpdater } from "./useAppUpdater";

export type ToastKind = "success" | "error" | "info";

export interface AppContextValue {
  config: AppConfig;
  setConfig: (config: AppConfig) => void;
  toast: (kind: ToastKind, message: string) => void;
  updater: AppUpdater;
}

export const AppContext = createContext<AppContextValue | null>(null);

export function useApp(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) {
    throw new Error("useApp musí byť použité vnútri AppContext.Provider");
  }
  return ctx;
}
