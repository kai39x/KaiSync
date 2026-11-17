import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import { Button, Dot, Glass, Sphere, StatusOrb, orbMode } from "../components/ui";
import { IconChevron, IconGamepad, IconHdr, IconMonitor, IconPause, IconPlay, IconPlayCircle, IconRestore } from "../components/ui/icons";
import { useStore } from "../state/store";
import s from "./screens.module.css";

export function Home() {
  const { t } = useTranslation();
  const { status, library, settings } = useStore();
  const paused = !!status?.paused;
  const mode = status?.mode ?? "idle";
  const hdr = status?.hdr;

  const title = paused ? t("modes.paused") : t(`modes.${mode}`);
  const videoKind =
    mode === "videoHdr" ? t("home.videoHdr", { kind: status?.video?.hdr ?? "HDR" }) : mode === "video" && status?.video ? t("home.videoSdr") : null;
  const sub = paused
    ? t("home.pausedSub")
    : mode === "idle"
      ? t("home.idleSub")
      : [status?.detail, videoKind].filter(Boolean).join(" · ");
  const enabledGames = library?.games.filter((g) => g.enabled && !g.needsPick).length ?? 0;

  return (
    <>
      <header className={s.header}>
        <h1 className={s.h1}>{t("nav.home")}</h1>
        <p className={s.sub}>{t("home.sub")}</p>
      </header>

      <Glass hero className={s.heroCard}>
        <div className={s.heroText}>
          <span className={`lbl ${s.heroLabel}`}>
            <Dot color={paused ? "var(--warn)" : "var(--ok)"} size={7} />
            {t("home.now")}
          </span>
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={`${title}|${sub}`}
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.2 }}
              style={{ display: "flex", flexDirection: "column", gap: 6, minWidth: 0 }}
            >
              <span className={s.heroTitle}>{title}</span>
              <span className={`${s.heroSub} ${s.ellipsis}`}>{sub}</span>
            </motion.div>
          </AnimatePresence>
          <div className={s.heroActions}>
            <Button aria-pressed={paused} icon={paused ? <IconPlay size={16} /> : <IconPause size={16} />} onClick={() => api.setPaused(!paused)}>
              {paused ? t("home.resume") : t("home.pause")}
            </Button>
          </div>
        </div>
        <StatusOrb mode={orbMode(mode)} paused={paused} />
      </Glass>

      <div className={s.stats}>
        <Glass className={s.stat}>
          <div className={s.statTop}>
            <Sphere size={40}><IconHdr color="#5cc8ff" /></Sphere>
            <Dot color={hdr?.on ? "var(--ok)" : "var(--off)"} />
          </div>
          <div className={s.statBody}>
            <span className="lbl">{t("home.hdr")}</span>
            <span className={s.statValue}>{hdr?.display ? (hdr.on ? t("home.on") : t("home.off")) : "—"}</span>
            <span className={`${s.statSub} ${s.ellipsis}`}>{hdr?.display ?? t("home.noDisplay")}</span>
          </div>
        </Glass>

        <Glass className={s.stat}>
          <div className={s.statTop}>
            <Sphere size={40}><IconGamepad color="#9b8cff" /></Sphere>
            <Dot color={enabledGames > 0 ? "var(--ok)" : "var(--off)"} />
          </div>
          <div className={s.statBody}>
            <span className="lbl">{t("home.games")}</span>
            <span className={s.statValue}>{t("home.gamesValue", { count: enabledGames })}</span>
            <span className={`${s.statSub} ${s.ellipsis}`}>{status?.scanning ? t("home.scanning") : t("home.gamesSub")}</span>
          </div>
        </Glass>

        <Glass className={s.stat}>
          <div className={s.statTop}>
            <Sphere size={40}><IconRestore color="#ffb84d" /></Sphere>
            <Dot color={hdr?.onByKaisync || hdr?.offByKaisync ? "var(--accent)" : "var(--off)"} />
          </div>
          <div className={s.statBody}>
            <span className="lbl">{t("home.restore")}</span>
            <span className={s.statValue}>
              {hdr?.onByKaisync ? t("home.restoreOurs") : hdr?.offByKaisync ? t("home.restoreOn") : t("home.restoreYours")}
            </span>
            <span className={`${s.statSub} ${s.ellipsis}`}>
              {hdr?.onByKaisync ? t("home.restoreOursSub") : hdr?.offByKaisync ? t("home.restoreOnSub") : t("home.restoreYoursSub")}
            </span>
          </div>
        </Glass>
      </div>

      <Glass className={s.rules}>
        <span className="lbl" style={{ flex: "none" }}>{t("home.rules")}</span>
        <div className={s.rule}>
          <IconGamepad color="#9b8cff" />
          <span className={s.ruleName}>{t("home.rGame")}</span>
          <span className={s.ruleArrow}><IconChevron size={14} /></span>
          <span className={s.ruleDo}>{t("home.rGameDo")}</span>
        </div>
        <div className={s.rule}>
          <IconPlayCircle color="#ffb84d" />
          <span className={s.ruleName}>{t("home.rVideo")}</span>
          <span className={s.ruleArrow}><IconChevron size={14} /></span>
          <span className={s.ruleDo}>{settings?.videoMatchHdr ? t("home.rVideoMatch") : t("home.rVideoDo")}</span>
        </div>
        <div className={s.rule}>
          <IconMonitor color="#bdbdbd" />
          <span className={s.ruleName}>{t("home.rIdle")}</span>
          <span className={s.ruleArrow}><IconChevron size={14} /></span>
          <span className={s.ruleDo}>{t("home.rIdleDo")}</span>
        </div>
      </Glass>
    </>
  );
}
