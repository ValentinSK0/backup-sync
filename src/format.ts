import type { Provider } from "./types";

/** ISO-8601 → "16. 7. 2026 14:30" (sk-SK). Null/neplatný dátum → "—". */
export function formatDate(iso: string | null | undefined): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  const date = d.toLocaleDateString("sk-SK", {
    day: "numeric",
    month: "numeric",
    year: "numeric",
  });
  const time = d.toLocaleTimeString("sk-SK", {
    hour: "2-digit",
    minute: "2-digit",
  });
  return `${date} ${time}`;
}

/** Epoch ms → "14:30:05" (čas pre feed aktivity). */
export function formatTime(epochMs: number): string {
  return new Date(epochMs).toLocaleTimeString("sk-SK", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

/** Bajty → ľudsky čitateľný reťazec ("1.4 MB"). */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["kB", "MB", "GB", "TB"];
  let value = bytes;
  let i = -1;
  do {
    value /= 1024;
    i += 1;
  } while (value >= 1024 && i < units.length - 1);
  return `${value.toFixed(1)} ${units[i]}`;
}

export function providerLabel(provider: Provider): string {
  return provider === "gdrive" ? "Google Drive" : "Vlastný cloud";
}

/** Posledná časť cesty (názov priečinka). */
export function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

/** Chyba z invoke (reject s reťazcom) → text pre toast. */
export function errMsg(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
