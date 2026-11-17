// Line icons from the design mockups (24px grid, 1.8 stroke).
import type { ReactNode } from "react";

function Svg({ size = 18, color = "currentColor", children }: { size?: number; color?: string; children: ReactNode }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {children}
    </svg>
  );
}

type P = { size?: number; color?: string };

export const IconHome = (p: P) => (
  <Svg {...p}><path d="M4 11l8-7 8 7v8a1 1 0 0 1-1 1h-4v-6h-6v6H5a1 1 0 0 1-1-1z" /></Svg>
);
export const IconGamepad = (p: P) => (
  <Svg {...p}>
    <path d="M6 11h4M8 9v4M15 12h.01M18 10h.01" />
    <path d="M17.3 5H6.7a4 4 0 0 0-3.96 3.4L2 14.6A3 3 0 0 0 7.2 17l1.3-1.5h7l1.3 1.5a3 3 0 0 0 5.2-2.4l-.74-6.2A4 4 0 0 0 17.3 5z" />
  </Svg>
);
export const IconSliders = (p: P) => (
  <Svg {...p}>
    <path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12" />
    <circle cx="16" cy="6" r="2" />
    <circle cx="10" cy="12" r="2" />
    <circle cx="18" cy="18" r="2" />
  </Svg>
);
export const IconPause = (p: P) => (
  <Svg {...p}><path d="M9 6v12M15 6v12" /></Svg>
);
export const IconPlay = (p: P) => (
  <Svg {...p}><path d="M8 5.5v13l10-6.5z" /></Svg>
);
export const IconHdr = (p: P) => (
  <Svg {...p}>
    <rect x="3" y="4" width="18" height="13" rx="2" />
    <path d="M8 21h8M12 17v4" />
    <path d="M7 13l3-5 2 3 2-2 3 4" />
  </Svg>
);
export const IconMonitor = (p: P) => (
  <Svg {...p}>
    <rect x="3" y="4" width="18" height="12" rx="2" />
    <path d="M8 20h8M12 16v4" />
  </Svg>
);
export const IconRestore = (p: P) => (
  <Svg {...p}>
    <path d="M4 12a8 8 0 1 0 2.3-5.7" />
    <path d="M4 4v5h5" />
  </Svg>
);
export const IconChevron = (p: P) => (
  <Svg {...p}><path d="M15 6l-6 6 6 6" /></Svg>
);
export const IconSearch = (p: P) => (
  <Svg {...p}>
    <circle cx="11" cy="11" r="7" />
    <path d="M20 20l-4-4" />
  </Svg>
);
export const IconPlus = (p: P) => (
  <Svg {...p}><path d="M12 5v14M5 12h14" /></Svg>
);
export const IconFolder = (p: P) => (
  <Svg {...p}><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /></Svg>
);
export const IconRefresh = (p: P) => (
  <Svg {...p}><path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7" /></Svg>
);
export const IconTrash = (p: P) => (
  <Svg {...p}>
    <path d="M4 7h16M10 11v6M14 11v6" />
    <path d="M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3" />
  </Svg>
);
export const IconCheck = (p: P) => (
  <Svg {...p}><path d="M5 12l5 5 9-10" /></Svg>
);
