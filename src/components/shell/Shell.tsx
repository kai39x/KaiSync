import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useTranslation } from "react-i18next";
import type { CSSProperties, ReactNode } from "react";
import { Dot, Glass, MiniOrb, PALETTE, Sphere, type OrbMode } from "../ui";
import { IconGamepad, IconHome, IconSliders } from "../ui/icons";
import { useStore } from "../../state/store";
import { inTauri } from "../../api";
import s from "./shell.module.css";

export type Page = "home" | "games" | "settings";

// Outside Tauri (browser preview) the caption buttons do nothing.
const win = inTauri
  ? getCurrentWindow()
  : { minimize: async () => {}, toggleMaximize: async () => {}, hide: async () => {} };

/* ---------- Title bar ---------- */
function TitleBar() {
  const { t } = useTranslation();
  return (
    <div className={s.titlebar}>
      <div className={s.brand} data-tauri-drag-region>
        <div className={s.brandIcon} aria-hidden>
          <Sphere
            size={16}
            style={{
              position: "absolute",
              top: 0,
              insetInlineStart: 0,
              background:
                "radial-gradient(circle at 32% 26%, #fff 0 10%, rgba(255,255,255,0) 30%), radial-gradient(circle, #5cc8ff 0, #3a7bd5 70%, rgba(255,255,255,0.5) 98%)",
            }}
          />
          <Sphere size={9} style={{ position: "absolute", bottom: 0, insetInlineEnd: 0 }} />
        </div>
        <span className={s.brandName}>KaiSync</span>
      </div>
      <div className={s.drag} data-tauri-drag-region onDoubleClick={() => win.toggleMaximize()} />
      <div className={s.caps}>
        <button className={s.cap} aria-label={t("app.minimize")} onClick={() => win.minimize()}>
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5h10" stroke="currentColor" strokeWidth="1" /></svg>
        </button>
        <button className={s.cap} aria-label={t("app.maximize")} onClick={() => win.toggleMaximize()}>
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x="0.5" y="0.5" width="9" height="9" rx="1.5" fill="none" stroke="currentColor" strokeWidth="1" /></svg>
        </button>
        <button className={`${s.cap} ${s.capClose}`} aria-label={t("app.close")} onClick={() => win.hide()}>
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 0l10 10M10 0L0 10" stroke="currentColor" strokeWidth="1" /></svg>
        </button>
      </div>
    </div>
  );
}

/* ---------- Light bubbles ---------- */
type Blob = { size: number; color: string; opacity: number; pos: CSSProperties };

function blobsFor(page: Page, mode: OrbMode): Blob[] {
  if (page === "home") {
    const b = PALETTE[mode].blobs;
    return [
      { size: 460, color: b[0], opacity: 0.55, pos: { top: -120, insetInlineEnd: 60 } },
      { size: 360, color: b[1], opacity: 0.35, pos: { top: 300, insetInlineStart: -120 } },
      { size: 300, color: b[2], opacity: 0.3, pos: { bottom: -140, insetInlineEnd: 340 } },
    ];
  }
  if (page === "games") {
    return [
      { size: 420, color: "#2f6fd6", opacity: 0.4, pos: { top: -160, insetInlineEnd: 120 } },
      { size: 340, color: "#6d4bd8", opacity: 0.32, pos: { bottom: -120, insetInlineStart: 80 } },
    ];
  }
  return [
    { size: 420, color: "#1f8fbf", opacity: 0.38, pos: { top: -150, insetInlineStart: 160 } },
    { size: 360, color: "#6d4bd8", opacity: 0.28, pos: { bottom: -150, insetInlineEnd: 260 } },
  ];
}

function Bubbles({ page, mode }: { page: Page; mode: OrbMode }) {
  const key = page === "home" ? `home-${mode}` : page;
  return (
    <div className={s.bubbles} aria-hidden>
      <AnimatePresence>
        <motion.div key={key} style={{ position: "absolute", inset: 0 }} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.8 }}>
          {blobsFor(page, mode).map((b, i) => (
            <div
              key={i}
              className={s.blob}
              style={{ width: b.size, height: b.size, opacity: b.opacity, background: `radial-gradient(circle, ${b.color} 0, rgba(0,0,0,0) 70%)`, ...b.pos }}
            />
          ))}
        </motion.div>
      </AnimatePresence>
    </div>
  );
}

/* ---------- Sidebar ---------- */
function Sidebar({ page, onNavigate }: { page: Page; onNavigate: (p: Page) => void }) {
  const { t } = useTranslation();
  const { status } = useStore();
  const reduce = useReducedMotion();
  const items: { id: Page; icon: ReactNode }[] = [
    { id: "home", icon: <IconHome /> },
    { id: "games", icon: <IconGamepad /> },
    { id: "settings", icon: <IconSliders /> },
  ];
  const paused = !!status?.paused;
  const mode = status?.mode ?? "idle";
  return (
    <nav className={s.nav} aria-label={t("nav.label")}>
      {items.map((it) => {
        const cur = it.id === page;
        return (
          <button key={it.id} className={`${s.item} ${cur ? s.itemCur : ""}`} aria-current={cur ? "page" : undefined} onClick={() => onNavigate(it.id)}>
            {cur ? (
              <motion.span layoutId="nav-pill" className={s.pill} transition={reduce ? { duration: 0 } : { type: "spring", stiffness: 420, damping: 34 }} />
            ) : null}
            <span className={s.itemIcon}>{it.icon}</span>
            <span className={s.itemLabel}>{t(`nav.${it.id}`)}</span>
          </button>
        );
      })}
      <div className={s.spacer} />
      <Glass as="div" className={s.statusCard}>
        <div style={{ opacity: paused ? 0.35 : 1, transition: "opacity .4s" }}>
          <MiniOrb mode={mode} />
        </div>
        <div className={s.statusText}>
          <span className={s.statusTitle}>{t(paused ? "modes.paused" : `modes.${mode}`)}</span>
          <span className={s.statusSub}>
            <Dot color={paused ? "var(--warn)" : "var(--ok)"} size={6} />
            {t(paused ? "status.paused" : "status.auto")}
          </span>
        </div>
      </Glass>
    </nav>
  );
}

/* ---------- Shell ---------- */
export function Shell({ page, onNavigate, children }: { page: Page; onNavigate: (p: Page) => void; children: ReactNode }) {
  const { status } = useStore();
  const reduce = useReducedMotion();
  return (
    <div className={s.app}>
      <Bubbles page={page} mode={status?.paused ? "idle" : (status?.mode ?? "idle")} />
      <TitleBar />
      <div className={s.body}>
        <Sidebar page={page} onNavigate={onNavigate} />
        <main className={s.main}>
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={page}
              className={s.page}
              initial={reduce ? false : { opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={reduce ? undefined : { opacity: 0, y: -6 }}
              transition={{ duration: 0.18 }}
            >
              {children}
            </motion.div>
          </AnimatePresence>
        </main>
      </div>
    </div>
  );
}
