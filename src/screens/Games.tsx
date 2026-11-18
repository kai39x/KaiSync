import { useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { api, type Game } from "../api";
import { Button, Glass, Sphere, Toggle } from "../components/ui";
import { IconCheck, IconFolder, IconPlus, IconRefresh, IconSearch, IconTrash } from "../components/ui/icons";
import { useStore } from "../state/store";
import s from "./screens.module.css";

const baseName = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? p;

/** Path shown relative to the game folder when possible (shorter, still unambiguous). */
function shortPath(full: string, root?: string | null) {
  if (root && full.toLowerCase().startsWith(root.toLowerCase())) {
    return full.slice(root.length).replace(/^[\\/]+/, "");
  }
  return full;
}

function PickPopover({ game, onClose, up }: { game: Game; onClose: () => void; up: boolean }) {
  const { t } = useTranslation();
  const { setSettings } = useStore();
  const ref = useRef<HTMLDivElement>(null);
  const root = game.id.startsWith("folder:") ? game.id.slice("folder:".length) : null;

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    ref.current?.querySelector("button")?.focus();
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  return (
    <motion.div
      ref={ref}
      role="menu"
      aria-label={t("games.pickTitle")}
      className={`${s.popover} ${up ? s.popoverUp : ""}`}
      style={{ border: "1px solid rgba(255,255,255,.12)", boxShadow: "0 18px 40px rgba(0,0,0,.6)", backdropFilter: "blur(28px)" }}
      initial={{ opacity: 0, y: -6, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: -4 }}
      transition={{ type: "spring", stiffness: 500, damping: 32 }}
    >
      <span className={`lbl ${s.popTitle}`}>{t("games.pickTitle")}</span>
      {game.candidates.map((c, i) => (
        <button
          key={c}
          role="menuitem"
          className={s.popItem}
          onClick={async () => {
            setSettings(await api.pickFolderExe(game.id, c));
            onClose();
          }}
        >
          <span className={`mono ${s.grow} ${s.ellipsis} ${i === 0 ? s.popBest : ""}`} title={c}>
            {shortPath(c, root)}
          </span>
          {i === 0 ? <IconCheck size={14} color="var(--accent)" /> : null}
        </button>
      ))}
    </motion.div>
  );
}

function GameRow({ g }: { g: Game }) {
  const { t } = useTranslation();
  const { setSettings } = useStore();
  const [picking, setPicking] = useState(false);
  const [up, setUp] = useState(false);
  const rowRef = useRef<HTMLDivElement>(null);

  const src =
    g.source === "folder" ? t("games.src.folder", { name: baseName(g.folderRoot ?? "") }) : t(`games.src.${g.source}`);
  const file = g.needsPick ? t("games.many") : baseName(g.exe ?? g.installDir ?? "");
  const fileTitle = g.exe ?? g.installDir ?? "";

  return (
    <div ref={rowRef} className={s.tableRow} style={{ opacity: g.enabled || g.needsPick ? 1 : 0.5 }}>
      <span className={`${s.gameName} ${s.ellipsis}`} title={g.name}>{g.name}</span>
      <span className={s.gameSrc}>
        <span className={s.ellipsis}>
          {src}
          {!g.enabled && !g.needsPick ? ` · ${t("games.excluded")}` : ""}
        </span>
        {g.source === "manual" && g.exe ? (
          <button
            className={s.miniIconBtn}
            aria-label={t("games.remove", { name: g.name })}
            title={t("games.remove", { name: g.name })}
            onClick={async () => setSettings(await api.removeManualGame(g.exe!))}
          >
            <IconTrash size={14} />
          </button>
        ) : null}
      </span>
      <span className={`${g.needsPick ? s.amber : "mono"} ${s.gameFile} ${s.ellipsis}`} title={fileTitle}>
        {file}
      </span>
      <div className={s.onCell}>
        {g.needsPick ? (
          <Button size="small" aria-haspopup="menu" aria-expanded={picking} onClick={() => {
              // Open upwards when the row sits in the lower half of the list.
              const row = rowRef.current?.getBoundingClientRect();
              const list = rowRef.current?.parentElement?.getBoundingClientRect();
              setUp(!!row && !!list && row.top - list.top > list.height / 2);
              setPicking((v) => !v);
            }}>
            {t("games.pick")}
          </Button>
        ) : (
          <Toggle on={g.enabled} label={t("games.enable", { name: g.name })} onChange={async (on) => setSettings(await api.setGameEnabled(g.id, on))} />
        )}
      </div>
      <AnimatePresence>{picking ? <PickPopover game={g} up={up} onClose={() => setPicking(false)} /> : null}</AnimatePresence>
    </div>
  );
}

export function Games() {
  const { t } = useTranslation();
  const { library, settings, setSettings, status } = useStore();
  const [q, setQ] = useState("");

  const games = useMemo(() => {
    const all = library?.games ?? [];
    const query = q.trim().toLowerCase();
    const list = query ? all.filter((g) => g.name.toLowerCase().includes(query)) : all;
    // Games that need a pick first, then by name (already sorted by the backend).
    return [...list].sort((a, b) => Number(b.needsPick) - Number(a.needsPick));
  }, [library, q]);

  const addExe = async () => {
    const f = await open({ multiple: false, title: t("games.pickExe"), filters: [{ name: t("games.programs"), extensions: ["exe"] }] });
    if (typeof f === "string") setSettings(await api.addManualGame(f));
  };
  const addFolder = async () => {
    const dir = await open({ directory: true, multiple: false, title: t("games.pickFolder") });
    if (typeof dir === "string") setSettings(await api.addGameFolder(dir));
  };

  const sources = [
    { id: "steam", name: "Steam" },
    { id: "epic", name: "Epic Games" },
    { id: "xbox", name: "Xbox" },
  ] as const;

  return (
    <>
      <header className={s.headerRow}>
        <div className={s.header}>
          <h1 className={s.h1}>{t("games.title")}</h1>
          <p className={s.sub}>{t("games.sub")}</p>
        </div>
        <div className={s.headerTools}>
          <div className={s.search}>
            <label htmlFor="gsearch" className="sr-only">{t("games.search")}</label>
            <span className={s.searchIcon}><IconSearch size={16} /></span>
            <input id="gsearch" type="search" className={s.searchInput} placeholder={t("games.search")} value={q} onChange={(e) => setQ(e.target.value)} />
          </div>
          <Button variant="pri" icon={<IconPlus size={16} />} onClick={addExe}>{t("games.add")}</Button>
        </div>
      </header>

      <div className={s.gamesGrid}>
        <Glass className={s.table}>
          <div className={`lbl ${s.tableHead}`}>
            <span>{t("games.colGame")}</span>
            <span>{t("games.colSource")}</span>
            <span>{t("games.colFile")}</span>
            <span style={{ textAlign: "end" }}>{t("games.colOn")}</span>
          </div>
          <div className={s.tableBody}>
            {games.length === 0 ? (
              <div className={s.empty}>{q ? t("games.noMatch") : t("games.empty")}</div>
            ) : (
              games.map((g) => <GameRow key={g.id} g={g} />)
            )}
          </div>
        </Glass>

        <aside className={s.aside}>
          <Glass className={s.sourcesCard}>
            <h2 className="h2" style={{ marginBottom: 6 }}>{t("games.sources")}</h2>
            {sources.map((src) => (
              <div className={s.row} style={{ minHeight: 48 }} key={src.id}>
                <span style={{ flex: 1, fontSize: 14 }}>{src.name}</span>
                <Toggle
                  on={settings?.sources[src.id] ?? true}
                  label={t("games.readLib", { name: src.name })}
                  onChange={async (on) => setSettings(await api.setSource(src.id, on))}
                />
              </div>
            ))}
          </Glass>

          <Glass className={s.foldersCard}>
            <h2 className="h2">{t("games.folders")}</h2>
            {(library?.folders ?? []).map((f) => {
              const statusText = status?.scanning
                ? t("games.scanningNow")
                : !f.exists
                  ? t("games.missing")
                  : f.needsPick > 0
                    ? `${t("games.scanned", { count: f.games })} · ${t("games.needsPick", { count: f.needsPick })}`
                    : t("games.scanned", { count: f.games });
              const color = !f.exists ? "var(--danger)" : f.needsPick > 0 ? "var(--warn)" : status?.scanning ? "var(--label)" : "var(--ok)";
              return (
                <div className={s.folderRow} key={f.path}>
                  <Sphere size={38}><IconFolder color="#ffb84d" /></Sphere>
                  <div className={s.folderText}>
                    <span className={`mono ${s.folderPath} ${s.ellipsis}`} title={f.path}>{f.path}</span>
                    <span className={s.folderStatus} style={{ color }}>{statusText}</span>
                  </div>
                  <button className={s.miniIconBtn} aria-label={t("games.rescan")} title={t("games.rescan")} onClick={() => api.rescan()}>
                    <IconRefresh size={16} />
                  </button>
                  <button className={s.miniIconBtn} aria-label={t("games.removeFolder")} title={t("games.removeFolder")} onClick={async () => setSettings(await api.removeGameFolder(f.path))}>
                    <IconTrash size={15} />
                  </button>
                </div>
              );
            })}
            <Button icon={<IconPlus size={16} />} onClick={addFolder}>{t("games.addFolder")}</Button>
            <p className={`lbl ${s.hint}`}>{t("games.folderHint")}</p>
          </Glass>
        </aside>
      </div>
    </>
  );
}
