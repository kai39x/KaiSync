// App-wide state: mirrors the Rust engine through commands + events.
import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, inTauri, type DisplaysView, type LibraryView, type Settings, type Status } from "../api";
import { applyLanguage, type Lang } from "../i18n";

interface Store {
  status: Status | null;
  library: LibraryView | null;
  settings: Settings | null;
  displays: DisplaysView;
  setSettings: (s: Settings) => void;
  refreshDisplays: () => Promise<void>;
  setLanguage: (l: Lang) => Promise<void>;
}

const Ctx = createContext<Store | null>(null);

export function StoreProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<Status | null>(null);
  const [library, setLibrary] = useState<LibraryView | null>(null);
  const [settings, setSettingsState] = useState<Settings | null>(null);
  const [displays, setDisplays] = useState<DisplaysView>({ displays: [], selected: null });

  const setSettings = useCallback((s: Settings) => {
    setSettingsState(s);
    applyLanguage(s.language);
  }, []);

  const refreshDisplays = useCallback(async () => {
    setDisplays(await api.listDisplays());
  }, []);

  useEffect(() => {
    let alive = true;
    (async () => {
      const [st, lib, set] = await Promise.all([api.getStatus(), api.getLibrary(), api.getSettings()]);
      if (!alive) return;
      setStatus(st);
      setLibrary(lib);
      setSettings(set);
      refreshDisplays();
    })();
    if (!inTauri) return () => { alive = false; };
    const un = [
      listen<Status>("status-changed", (e) => setStatus(e.payload)),
      listen("library-changed", async () => setLibrary(await api.getLibrary())),
      // Another window (or the backend) changed the settings, e.g. the language.
      listen<Settings>("settings-changed", (e) => setSettings(e.payload)),
    ];
    return () => {
      alive = false;
      un.forEach((p) => p.then((f) => f()));
    };
  }, [setSettings, refreshDisplays]);

  const setLanguage = useCallback(
    async (l: Lang) => {
      applyLanguage(l);
      setSettings(await api.setLanguage(l));
    },
    [setSettings],
  );

  const value = useMemo(
    () => ({ status, library, settings, displays, setSettings, refreshDisplays, setLanguage }),
    [status, library, settings, displays, setSettings, refreshDisplays, setLanguage],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useStore() {
  const s = useContext(Ctx);
  if (!s) throw new Error("useStore outside StoreProvider");
  return s;
}
