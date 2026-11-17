// The glass tray menu (design/Tray.html). Runs in its own small transparent window.
import { useEffect, useLayoutEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useTranslation } from "react-i18next";
import { inTauri } from "../api";
import { MiniOrb } from "../components/ui";
import { useStore } from "../state/store";
import s from "./tray.module.css";

const action = (a: "open" | "pause" | "exit" | "close") => (inTauri ? invoke("tray_action", { action: a }) : Promise.resolve());

function Icon({ d }: { d: string }) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={d} />
    </svg>
  );
}

export function TrayMenu() {
  const { t } = useTranslation();
  const { status } = useStore();
  const card = useRef<HTMLDivElement>(null);
  const paused = !!status?.paused;
  const mode = status?.mode ?? "idle";

  const title = paused ? t("modes.paused") : t(`modes.${mode}`);
  const sub = paused
    ? t("status.paused")
    : [mode !== "idle" ? status?.detail : null, status?.hdr.display ? `HDR ${status.hdr.on ? t("home.on") : t("home.off")}` : null]
        .filter(Boolean)
        .join(" · ");

  // Fit the window to the card (the Rust side positions it above the click).
  useLayoutEffect(() => {
    if (!inTauri || !card.current) return;
    const r = card.current.getBoundingClientRect();
    getCurrentWindow().setSize(new LogicalSize(Math.ceil(r.width) + 16, Math.ceil(r.height) + 16));
  }, [title, sub, t]);

  // Focus the first item each time it opens; Escape closes.
  useEffect(() => {
    const focusFirst = () => card.current?.querySelector<HTMLButtonElement>("button")?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") action("close");
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        const items = [...(card.current?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
        const i = items.indexOf(document.activeElement as HTMLButtonElement);
        const n = e.key === "ArrowDown" ? (i + 1) % items.length : (i - 1 + items.length) % items.length;
        items[n]?.focus();
        e.preventDefault();
      }
    };
    document.addEventListener("keydown", onKey);
    const un = inTauri ? listen("tray-menu-opened", focusFirst) : null;
    return () => {
      document.removeEventListener("keydown", onKey);
      un?.then((f) => f());
    };
  }, []);

  return (
    <div className={s.stage}>
      <div ref={card} role="menu" aria-label="KaiSync" className={s.menu}>
        <div className={s.blobA} aria-hidden />
        <div className={s.blobB} aria-hidden />
        <div className={s.head}>
          <div style={{ opacity: paused ? 0.35 : 1 }}>
            <MiniOrb mode={mode} size={38} />
          </div>
          <div className={s.headText}>
            <span className={s.title}>{title}</span>
            {sub ? <span className={s.sub}>{sub}</span> : null}
          </div>
        </div>
        <div className={s.sep} style={{ margin: "2px 8px 4px" }} />
        <button role="menuitem" className={s.item} onClick={() => action("open")}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <rect x="3" y="4" width="18" height="14" rx="2" />
            <path d="M3 8h18" />
          </svg>
          {t("tray.open")}
        </button>
        <button role="menuitem" className={s.item} onClick={() => action("pause")}>
          <Icon d={paused ? "M8 5.5v13l10-6.5z" : "M9 6v12M15 6v12"} />
          {paused ? t("home.resume") : t("home.pause")}
        </button>
        <div className={s.sep} style={{ margin: "4px 8px" }} />
        <button role="menuitem" className={`${s.item} ${s.danger}`} onClick={() => action("exit")}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M12 3v8" />
            <path d="M6.3 7.3a8 8 0 1 0 11.4 0" />
          </svg>
          {t("tray.exit")}
        </button>
      </div>
    </div>
  );
}
