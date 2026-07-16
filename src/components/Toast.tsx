import type { ToastKind } from "../context";
import { AlertIcon, CheckIcon, InfoIcon, XIcon } from "./icons";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

const KIND: Record<
  ToastKind,
  { border: string; iconCls: string; Icon: typeof CheckIcon }
> = {
  success: {
    border: "border-emerald-500/40",
    iconCls: "text-emerald-400",
    Icon: CheckIcon,
  },
  error: {
    border: "border-red-500/40",
    iconCls: "text-red-400",
    Icon: AlertIcon,
  },
  info: {
    border: "border-sky-500/40",
    iconCls: "text-sky-400",
    Icon: InfoIcon,
  },
};

export default function ToastStack({
  toasts,
  onDismiss,
}: {
  toasts: ToastItem[];
  onDismiss: (id: number) => void;
}) {
  if (toasts.length === 0) return null;
  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[100] flex w-80 flex-col gap-2">
      {toasts.map((t) => {
        const k = KIND[t.kind];
        return (
          <div
            key={t.id}
            className={`toast-in pointer-events-auto flex items-start gap-3 rounded-xl border ${k.border}
              bg-zinc-900 px-4 py-3 shadow-xl shadow-black/40`}
          >
            <k.Icon className={`mt-0.5 h-4 w-4 shrink-0 ${k.iconCls}`} />
            <p className="min-w-0 flex-1 break-words text-sm text-zinc-200">
              {t.message}
            </p>
            <button
              onClick={() => onDismiss(t.id)}
              className="shrink-0 text-zinc-500 transition-colors hover:text-zinc-200"
              aria-label="Zavrieť upozornenie"
            >
              <XIcon className="h-4 w-4" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
