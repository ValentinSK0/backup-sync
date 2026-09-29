interface IconProps {
  className?: string;
}

function svgProps(className: string | undefined) {
  return {
    className,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.8,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true,
  };
}

export function LogoIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)} strokeWidth={1.6}>
      <path d="M17.5 18.5H7a4.5 4.5 0 0 1-.6-8.96A6 6 0 0 1 18 10.5a4 4 0 0 1-.5 8Z" />
      <path d="M12 15.5v-5" />
      <path d="m9.8 12.7 2.2-2.2 2.2 2.2" />
    </svg>
  );
}

export function HomeIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)}>
      <path d="m3 10.5 9-7.5 9 7.5" />
      <path d="M5 9v11h5v-6h4v6h5V9" />
    </svg>
  );
}

export function FolderIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)}>
      <path d="M3 6.5A1.5 1.5 0 0 1 4.5 5h4l2 2.5h9A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5v-12Z" />
    </svg>
  );
}

export function GamepadIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)}>
      <path d="M6.7 7h10.6a4.5 4.5 0 0 1 4.43 5.29l-.83 4.68a2.5 2.5 0 0 1-4.35 1.2L14.6 16H9.4l-1.95 2.17a2.5 2.5 0 0 1-4.35-1.2l-.83-4.68A4.5 4.5 0 0 1 6.7 7Z" />
      <path d="M8 10.5v3M6.5 12h3" />
      <path d="M15.5 10.75h.01M17.75 13h.01" />
    </svg>
  );
}

export function SettingsIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)}>
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1.11-1.56 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.65 8.9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34h.08A1.7 1.7 0 0 0 10.12 3V3a2 2 0 1 1 4 0v.09c0 .68.4 1.3 1.03 1.56.6.27 1.32.13 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87v.08c.26.63.88 1.03 1.56 1.03H21a2 2 0 1 1 0 4h-.09c-.68 0-1.3.4-1.51 1.03Z" />
    </svg>
  );
}

export function PlusIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)} strokeWidth={2.2}>
      <path d="M12 5v14M5 12h14" />
    </svg>
  );
}

export function TrashIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)}>
      <path d="M4 7h16M9.5 7V5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v2" />
      <path d="M6 7l1 12.1a1.5 1.5 0 0 0 1.5 1.4h7a1.5 1.5 0 0 0 1.5-1.4L18 7" />
      <path d="M10 11v6M14 11v6" />
    </svg>
  );
}

export function UploadIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)} strokeWidth={2}>
      <path d="M12 16V5" />
      <path d="m7 9.5 5-5 5 5" />
      <path d="M4.5 19.5h15" />
    </svg>
  );
}

export function DownloadIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)} strokeWidth={2}>
      <path d="M12 5v11" />
      <path d="m7 11.5 5 5 5-5" />
      <path d="M4.5 19.5h15" />
    </svg>
  );
}

export function LinkIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)}>
      <path d="m9.5 14.5 5-5" />
      <path d="M7.1 17.9 5.7 19.3a3.5 3.5 0 0 1-5-5l3.2-3.2a3.5 3.5 0 0 1 5 0" />
      <path d="m16.9 6.1 1.4-1.4a3.5 3.5 0 0 1 5 5l-3.2 3.2a3.5 3.5 0 0 1-5 0" />
    </svg>
  );
}

export function CloudIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)}>
      <path d="M17.5 18.5H7a4.5 4.5 0 0 1-.6-8.96A6 6 0 0 1 18 10.5a4 4 0 0 1-.5 8Z" />
    </svg>
  );
}

export function HistoryIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)}>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 7.5V12l3 2" />
    </svg>
  );
}

export function XIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)} strokeWidth={2.2}>
      <path d="m6 6 12 12M18 6 6 18" />
    </svg>
  );
}

export function CheckIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)} strokeWidth={2.4}>
      <path d="m4.5 12.5 5 5 10-11" />
    </svg>
  );
}

export function AlertIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)}>
      <path d="M10.3 4.1 2.9 17a2 2 0 0 0 1.73 3h14.74a2 2 0 0 0 1.73-3L13.7 4.1a2 2 0 0 0-3.4 0Z" />
      <path d="M12 9.5v4M12 16.8h.01" />
    </svg>
  );
}

export function InfoIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)}>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 11v5M12 7.8h.01" />
    </svg>
  );
}

export function SpinnerIcon({ className }: IconProps) {
  return (
    <svg
      className={`animate-spin ${className ?? ""}`}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2.5}
      strokeLinecap="round"
      aria-hidden
    >
      <path d="M12 3a9 9 0 1 0 9 9" />
    </svg>
  );
}

export function ListIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)}>
      <line x1="8" y1="6" x2="21" y2="6" />
      <line x1="8" y1="12" x2="21" y2="12" />
      <line x1="8" y1="18" x2="21" y2="18" />
      <line x1="3" y1="6" x2="3.01" y2="6" />
      <line x1="3" y1="12" x2="3.01" y2="12" />
      <line x1="3" y1="18" x2="3.01" y2="18" />
    </svg>
  );
}

export function GridIcon({ className }: IconProps) {
  return (
    <svg {...svgProps(className)}>
      <rect x="3" y="3" width="7" height="7" rx="1" />
      <rect x="14" y="3" width="7" height="7" rx="1" />
      <rect x="14" y="14" width="7" height="7" rx="1" />
      <rect x="3" y="14" width="7" height="7" rx="1" />
    </svg>
  );
}
