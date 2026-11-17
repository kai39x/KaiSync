import { type ButtonHTMLAttributes, type CSSProperties, type HTMLAttributes, type ReactNode, useId } from "react";
import { motion, useReducedMotion } from "motion/react";
import s from "./ui.module.css";

const cx = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(" ");

/* ---------- Glass panel ---------- */
export function Glass({
  as: Tag = "section",
  hero,
  className,
  ...rest
}: HTMLAttributes<HTMLElement> & { as?: "section" | "div" | "aside"; hero?: boolean }) {
  return <Tag className={cx(s.glass, hero && s.hero, className)} {...rest} />;
}

/* ---------- Glass sphere (icon badges, swatches…) ---------- */
export function Sphere({ size, children, style, className }: { size: number; children?: ReactNode; style?: CSSProperties; className?: string }) {
  return (
    <div className={cx(s.sphere, className)} style={{ width: size, height: size, ...style }}>
      {children}
    </div>
  );
}

/* ---------- Mode palettes (from the mockups) ---------- */
export type OrbMode = "game" | "video" | "idle";
export const PALETTE: Record<OrbMode, { c: string[]; glow: string; blobs: string[] }> = {
  game: { c: ["#5cc8ff", "#9b8cff", "#6ee7ff", "#5b6cff"], glow: "rgba(92,170,255,0.30)", blobs: ["#2f6fd6", "#6d4bd8", "#1f8fbf"] },
  video: { c: ["#ffb84d", "#ff7aa8", "#ffd76a", "#ff9361"], glow: "rgba(255,170,90,0.26)", blobs: ["#c46a1f", "#b8406e", "#a07a1a"] },
  idle: { c: ["#e8e8e8", "#8a8a8a", "#f5f5f5", "#5a5a5a"], glow: "rgba(255,255,255,0.08)", blobs: ["#3a3a3a", "#2a2a2a", "#333333"] },
};
const conic = (c: string[]) => `conic-gradient(from 0deg, ${c[0]}, ${c[1]}, ${c[2]}, ${c[3]}, ${c[0]})`;

/** Small orb (sidebar card, tray). */
export function MiniOrb({ mode, size = 36 }: { mode: OrbMode; size?: number }) {
  return (
    <div className={cx(s.sphere, s.orb)} style={{ width: size, height: size }}>
      <div className={s.orbSwirlMini} style={{ background: conic(PALETTE[mode].c) }} />
      <div className={s.sphere} style={{ position: "absolute", inset: 0, boxShadow: "inset 0 0 0 1px rgba(255,255,255,0.3)" }} />
    </div>
  );
}

/** Home hero: 176px glass sphere with the mode colors swirling inside, plus floating bubbles. */
export function StatusOrb({ mode, paused }: { mode: OrbMode; paused: boolean }) {
  const p = PALETTE[mode];
  return (
    <motion.div className={s.orbStage} aria-hidden animate={{ opacity: paused ? 0.35 : 1 }} transition={{ duration: 0.4 }}>
      <div
        className={cx(s.sphere, s.orb, s.breath)}
        style={{
          position: "absolute",
          insetInlineStart: 40,
          top: 12,
          width: 176,
          height: 176,
          boxShadow: `inset 0 0 0 1px rgba(255,255,255,0.3), 0 0 80px ${paused ? "rgba(0,0,0,0)" : p.glow}`,
          transition: "box-shadow .6s",
        }}
      >
        <div className={s.orbSwirl} style={{ background: conic(p.c) }} />
        <div className={cx(s.sphere, s.orbGlass)} />
      </div>
      <div className={cx(s.sphere, s.float)} style={{ width: 46, height: 46, top: 0, insetInlineStart: 196, animationDelay: "-2s" }} />
      <div className={cx(s.sphere, s.float)} style={{ width: 26, height: 26, top: 158, insetInlineStart: 18, animationDelay: "-5s" }} />
      <div className={cx(s.sphere, s.float)} style={{ width: 13, height: 13, top: 40, insetInlineStart: 14, animationDelay: "-3s" }} />
      <div className={cx(s.sphere, s.float)} style={{ width: 10, height: 10, top: 184, insetInlineStart: 222, animationDelay: "-1s" }} />
    </motion.div>
  );
}

/* ---------- Toggle ---------- */
export function Toggle({
  on,
  onChange,
  label,
  labelledBy,
  disabled,
}: {
  on: boolean;
  onChange: (on: boolean) => void;
  label?: string;
  labelledBy?: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      aria-labelledby={labelledBy}
      disabled={disabled}
      className={cx(s.toggle, on && s.toggleOn)}
      onClick={() => onChange(!on)}
    >
      <span className={s.knob} />
    </button>
  );
}

/* ---------- Button ---------- */
export function Button({
  variant,
  size,
  icon,
  iconOnly,
  className,
  children,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "pri" | "ghost";
  size?: "small";
  icon?: ReactNode;
  iconOnly?: boolean;
}) {
  return (
    <button
      type="button"
      className={cx(s.btn, variant === "pri" && s.pri, variant === "ghost" && s.ghost, size === "small" && s.small, iconOnly && s.iconOnly, className)}
      {...rest}
    >
      {icon}
      {children}
    </button>
  );
}

/* ---------- Segmented control ---------- */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
  labelledBy,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  labelledBy?: string;
}) {
  const id = useId();
  const reduce = useReducedMotion();
  return (
    <div className={s.seg} role="radiogroup" aria-labelledby={labelledBy}>
      {options.map((o) => {
        const on = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={on}
            className={cx(s.segBtn, on && s.segOn)}
            onClick={() => onChange(o.value)}
            title={o.label}
          >
            {on ? (
              <motion.span
                layoutId={`seg-${id}`}
                className={s.segPill}
                transition={reduce ? { duration: 0 } : { type: "spring", stiffness: 500, damping: 34 }}
              />
            ) : null}
            <span className={s.segLabel}>{o.label}</span>
          </button>
        );
      })}
    </div>
  );
}

/* ---------- Status dot ---------- */
export function Dot({ color, size = 8 }: { color: string; size?: number }) {
  return <span className={s.dot} style={{ width: size, height: size, background: color }} />;
}
