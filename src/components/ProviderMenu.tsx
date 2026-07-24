import { useState, type ReactNode } from "react";
import { useApp } from "../context";
import { providerLabel } from "../format";
import { CloudIcon } from "./icons";
import { Button } from "./ui";
import type { Provider } from "../types";

/**
 * Tlačidlo s dropdown výberom PRIHLÁSENÉHO cloudu.
 * - žiadny pripojený cloud → toast s upozornením
 * - jeden pripojený → vyberie sa rovno bez menu
 * - viac pripojených → rozbaľovacie menu
 */
export default function ProviderMenu({
  label,
  icon,
  variant = "secondary",
  disabled = false,
  onSelect,
}: {
  label: string;
  icon?: ReactNode;
  variant?: "primary" | "secondary";
  disabled?: boolean;
  onSelect: (provider: Provider) => void;
}) {
  const { config, toast } = useApp();
  const [open, setOpen] = useState(false);

  const connected: Provider[] = (
    ["gdrive", "ulozto", "customCloud"] as Provider[]
  ).filter((p) => config.providers[p].connected);

  function handleClick() {
    if (connected.length === 0) {
      toast("error", "Najprv pripoj aspoň jeden cloud v Nastaveniach.");
      return;
    }
    if (connected.length === 1) {
      onSelect(connected[0]);
      return;
    }
    setOpen((value) => !value);
  }

  return (
    <div className="relative">
      <Button variant={variant} onClick={handleClick} disabled={disabled}>
        {icon}
        {label}
        <svg
          className="h-3.5 w-3.5 opacity-70"
          viewBox="0 0 20 20"
          fill="currentColor"
          aria-hidden
        >
          <path
            fillRule="evenodd"
            d="M5.23 7.21a.75.75 0 0 1 1.06.02L10 11.06l3.71-3.83a.75.75 0 1 1 1.08 1.04l-4.25 4.39a.75.75 0 0 1-1.08 0L5.21 8.27a.75.75 0 0 1 .02-1.06Z"
            clipRule="evenodd"
          />
        </svg>
      </Button>

      {open && (
        <>
          <div
            className="fixed inset-0 z-40"
            onClick={() => setOpen(false)}
            aria-hidden
          />
          <div className="absolute right-0 top-full z-50 mt-1.5 w-52 overflow-hidden rounded-lg border border-zinc-700 bg-zinc-900 shadow-xl shadow-black/50">
            {connected.map((p) => (
              <button
                key={p}
                type="button"
                className="flex w-full items-center gap-2.5 px-3 py-2.5 text-left text-sm text-zinc-200 transition-colors hover:bg-indigo-600/15 hover:text-indigo-300"
                onClick={() => {
                  setOpen(false);
                  onSelect(p);
                }}
              >
                <CloudIcon className="h-4 w-4 text-zinc-500" />
                {providerLabel(p)}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
