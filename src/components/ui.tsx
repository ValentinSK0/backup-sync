import type { ButtonHTMLAttributes, ReactNode } from "react";
import { SpinnerIcon } from "./icons";

export const inputCls =
  "w-full rounded-lg border border-zinc-700 bg-zinc-800/60 px-3 py-2 text-sm text-zinc-100 " +
  "placeholder-zinc-500 outline-none transition-colors focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500";

export const labelCls = "mb-1.5 block text-xs font-medium text-zinc-400";

type Variant = "primary" | "secondary" | "danger" | "ghost";

const variantCls: Record<Variant, string> = {
  primary:
    "bg-indigo-600 text-white hover:bg-indigo-500 active:bg-indigo-700 shadow-sm",
  secondary:
    "border border-zinc-700 bg-zinc-800 text-zinc-200 hover:bg-zinc-700 hover:text-white",
  danger: "bg-red-600/90 text-white hover:bg-red-500 active:bg-red-700",
  ghost: "text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100",
};

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  loading?: boolean;
  variant?: Variant;
}

export function Button({
  loading = false,
  variant = "secondary",
  className = "",
  children,
  disabled,
  type = "button",
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      disabled={disabled || loading}
      className={`inline-flex items-center justify-center gap-2 rounded-lg px-3.5 py-2 text-sm font-medium
        transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500
        disabled:cursor-not-allowed disabled:opacity-50 ${variantCls[variant]} ${className}`}
      {...rest}
    >
      {loading && <SpinnerIcon className="h-4 w-4" />}
      {children}
    </button>
  );
}

export function Card({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={`rounded-xl border border-zinc-800 bg-zinc-900 ${className}`}>
      {children}
    </div>
  );
}

export function Toggle({
  checked,
  onChange,
  disabled = false,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors
        focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500
        disabled:cursor-not-allowed disabled:opacity-50
        ${checked ? "bg-indigo-600" : "bg-zinc-700"}`}
    >
      <span
        className={`inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform
          ${checked ? "translate-x-6" : "translate-x-1"}`}
      />
    </button>
  );
}

export function PageHeader({
  title,
  subtitle,
  action,
}: {
  title: string;
  subtitle?: string;
  action?: ReactNode;
}) {
  return (
    <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
      <div>
        <h1 className="text-xl font-semibold text-zinc-100">{title}</h1>
        {subtitle && <p className="mt-1 text-sm text-zinc-500">{subtitle}</p>}
      </div>
      {action}
    </div>
  );
}

export function EmptyState({
  icon,
  message,
  hint,
}: {
  icon: ReactNode;
  message: string;
  hint?: string;
}) {
  return (
    <Card className="flex flex-col items-center justify-center gap-2 px-6 py-12 text-center">
      <div className="text-zinc-600">{icon}</div>
      <p className="text-sm font-medium text-zinc-400">{message}</p>
      {hint && <p className="text-xs text-zinc-600">{hint}</p>}
    </Card>
  );
}
