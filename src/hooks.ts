import { useCallback, useState } from "react";
import { useApp } from "./context";
import { errMsg } from "./format";
import type { AppConfig } from "./types";

/**
 * Spúšťanie commandov, ktoré vracajú čerstvý AppConfig.
 * Drží kľúč práve bežiacej akcie (loading stav tlačidiel),
 * po úspechu nahradí root config, pri chybe zobrazí červený toast.
 */
export function useConfigAction() {
  const { setConfig, toast } = useApp();
  const [busy, setBusy] = useState<string | null>(null);

  const run = useCallback(
    async (
      key: string,
      fn: () => Promise<AppConfig>,
      successMsg?: string,
    ): Promise<boolean> => {
      setBusy(key);
      try {
        const config = await fn();
        setConfig(config);
        if (successMsg) toast("success", successMsg);
        return true;
      } catch (e) {
        toast("error", errMsg(e));
        return false;
      } finally {
        setBusy(null);
      }
    },
    [setConfig, toast],
  );

  return { busy, run };
}
