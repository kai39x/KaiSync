import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import { Button, Glass, Segmented, Sphere, Toggle } from "../components/ui";
import { IconHdr, IconPlayCircle } from "../components/ui/icons";
import type { Lang } from "../i18n";
import { useStore } from "../state/store";
import s from "./screens.module.css";

function GeneralCard() {
  const { t } = useTranslation();
  const { settings, setSettings, setLanguage } = useStore();
  const opts = [
    { key: "autostart" as const, label: t("settings.autostart"), hint: t("settings.autostartHint") },
    { key: "desktopShortcut" as const, label: t("settings.desktop"), hint: t("settings.desktopHint") },
  ];
  return (
    <Glass className={s.generalCard}>
      <h2 className="h2" style={{ marginBottom: 12 }}>{t("settings.general")}</h2>
      <div className={s.field} style={{ paddingBottom: 14 }}>
        <span id="langlbl" style={{ fontSize: 14 }}>{t("settings.language")}</span>
        <Segmented<Lang>
          labelledBy="langlbl"
          value={settings?.language ?? "ar"}
          options={[
            { value: "ar", label: "العربية" },
            { value: "en", label: "English" },
          ]}
          onChange={(l) => setLanguage(l)}
        />
        <span className="lbl">{t("settings.langNote")}</span>
      </div>
      {opts.map((o) => (
        <div className={s.optRow} key={o.key}>
          <div className={s.optText}>
            <span id={`opt-${o.key}`} style={{ fontSize: 14 }}>{o.label}</span>
            <span className="lbl">{o.hint}</span>
          </div>
          <Toggle on={settings?.[o.key] ?? true} labelledBy={`opt-${o.key}`} onChange={async (on) => setSettings(await api.setGeneral(o.key, on))} />
        </div>
      ))}
    </Glass>
  );
}

function PlayerCard() {
  const { t } = useTranslation();
  const { library, settings, setSettings } = useStore();
  const p = library?.player;
  const change = async () => {
    const f = await open({ multiple: false, title: t("settings.pickPlayer"), filters: [{ name: t("games.programs"), extensions: ["exe"] }] });
    if (typeof f === "string") setSettings(await api.setPlayer(f));
  };
  return (
    <Glass className={s.card}>
      <h2 className="h2">{t("settings.player")}</h2>
      <div className={s.deviceRow}>
        <Sphere size={42}><IconPlayCircle color="#ffb84d" /></Sphere>
        <div className={s.grow} style={{ display: "flex", flexDirection: "column", gap: 1 }}>
          <span className={s.deviceName}>{p?.name ?? "—"}</span>
          <span className={`mono ${s.ellipsis}`} style={{ fontSize: 11.5, color: p && !p.exists ? "var(--warn)" : "#8a8a8a", textAlign: "end" }} title={p?.path}>
            {p ? (p.exists ? p.path : `${p.path} · ${t("settings.playerMissing")}`) : ""}
          </span>
        </div>
        <Button onClick={change}>{t("settings.change")}</Button>
      </div>
      <div className={s.optRow} style={{ minHeight: 58 }}>
        <div className={s.optText}>
          <span id="opt-matchHdr" style={{ fontSize: 14 }}>{t("settings.matchHdr")}</span>
          <span className="lbl" style={{ lineHeight: 1.6 }}>{t("settings.matchHdrHint")}</span>
        </div>
        <Toggle on={settings?.videoMatchHdr ?? true} labelledBy="opt-matchHdr" onChange={async (on) => setSettings(await api.setGeneral("videoMatchHdr", on))} />
      </div>
      <p className="lbl" style={{ margin: 0 }}>{t("settings.playerHint")}</p>
    </Glass>
  );
}

function HdrCard() {
  const { t } = useTranslation();
  const { displays, refreshDisplays, setSettings, status } = useStore();
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ text: string; ok: boolean } | null>(null);

  useEffect(() => {
    refreshDisplays();
  }, [refreshDisplays]);

  // Keep the display list fresh when HDR changes (Win+Alt+B, a game…).
  useEffect(() => {
    refreshDisplays();
  }, [status?.hdr.on, refreshDisplays]);

  const current = displays.displays.find((d) => d.id === displays.selected) ?? displays.displays[0];

  const test = async (on: boolean) => {
    setBusy(true);
    setMsg({ text: t("settings.working"), ok: true });
    try {
      const now = await api.hdrTest(on);
      setMsg({ text: now ? t("settings.nowOn") : t("settings.nowOff"), ok: true });
    } catch (e) {
      setMsg({ text: t("settings.failed", { error: String(e) }), ok: false });
    } finally {
      setBusy(false);
      refreshDisplays();
    }
  };

  return (
    <Glass className={s.card} style={{ gap: 18 }}>
      <h2 className="h2">{t("settings.hdr")}</h2>
      {current ? (
        <>
          <div className={s.deviceRow}>
            <Sphere size={42}><IconHdr color="#5cc8ff" /></Sphere>
            <div className={s.grow} style={{ display: "flex", flexDirection: "column", gap: 1 }}>
              <span className={`${s.deviceName} ${s.ellipsis}`}>
                {current.name}
                {current.primary ? <span className="lbl" style={{ fontWeight: 400 }}> · {t("settings.primary")}</span> : null}
              </span>
              <span style={{ fontSize: 12, color: current.hdrOn ? "var(--ok)" : "var(--label)" }}>
                {current.hdrOn ? t("settings.hdrOn") : t("settings.hdrOff")}
              </span>
            </div>
          </div>
          {displays.displays.length > 1 ? (
            <div className={s.field}>
              <span id="displbl" className={s.fieldLabel}>{t("settings.display")}</span>
              <Segmented
                labelledBy="displbl"
                value={current.id}
                options={displays.displays.map((d) => ({ value: d.id, label: d.name }))}
                onChange={async (id) => {
                  setSettings(await api.setHdrDisplay(id));
                  refreshDisplays();
                }}
              />
            </div>
          ) : null}
          <span className="lbl" style={{ lineHeight: 1.6 }}>{t("settings.rule")}</span>
          <div className={s.btnRow}>
            <Button variant="pri" disabled={busy} onClick={() => test(true)}>{t("settings.tryOn")}</Button>
            <Button disabled={busy} onClick={() => test(false)}>{t("settings.tryOff")}</Button>
          </div>
          <span className={`lbl ${s.msg}`} role="status" style={{ color: msg && !msg.ok ? "var(--warn)" : undefined }}>
            {msg?.text ?? ""}
          </span>
        </>
      ) : (
        <div className={s.deviceRow}>
          <Sphere size={42}><IconHdr color="#8a8a8a" /></Sphere>
          <div className={s.grow} style={{ display: "flex", flexDirection: "column", gap: 2 }}>
            <span className={s.deviceName}>{t("settings.noDisplay")}</span>
            <span className="lbl" style={{ lineHeight: 1.6 }}>{t("settings.noDisplayHint")}</span>
          </div>
        </div>
      )}
    </Glass>
  );
}

export function Settings() {
  const { t } = useTranslation();
  return (
    <>
      <header className={s.header}>
        <h1 className={s.h1}>{t("settings.title")}</h1>
        <p className={s.sub}>{t("settings.sub")}</p>
      </header>
      <div className={s.settingsGrid}>
        <div className={s.col}>
          <PlayerCard />
          <HdrCard />
        </div>
        <div className={s.col}>
          <GeneralCard />
        </div>
      </div>
    </>
  );
}
