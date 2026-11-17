// Browser-only preview data (vite dev outside Tauri). Never used inside the app.
import type { DisplaysView, LibraryView, Settings, Status } from "../api";

const params = new URLSearchParams(location.search);
const mode = params.get("mode") === "game" ? "game" : "idle";

let settings: Settings = {
  language: params.get("lang") === "en" ? "en" : "ar",
  sources: { steam: true, epic: true, xbox: false },
  gameFolders: ["C:\\Users\\Kai\\Pictures\\games"],
  folderPicks: {},
  manualGames: [{ path: "C:\\Emu\\Cemu\\Cemu.exe", name: "Cemu" }],
  gameEnabled: {},
  hdrDisplay: null,
  autostart: true,
  desktopShortcut: true,
  debounceMs: 3000,
};

const status: Status = {
  mode,
  detail: mode === "game" ? "Elden Ring" : null,
  paused: params.get("paused") === "1",
  rawMode: mode,
  rawDetail: null,
  scanning: false,
  hdr: { display: "Beyond TV", supported: true, on: mode === "game", onByKaisync: mode === "game" },
};

const root = "C:\\Users\\Kai\\Pictures\\games";
const library: LibraryView = {
  games: [
    { id: "steam:1245620", name: "Elden Ring", source: "steam", installDir: "D:\\SteamLibrary\\steamapps\\common\\ELDEN RING", exe: null, candidates: [], needsPick: false, defaultEnabled: true, enabled: true, folderRoot: null },
    { id: "steam:1091500", name: "Cyberpunk 2077", source: "steam", installDir: "D:\\SteamLibrary\\steamapps\\common\\Cyberpunk 2077", exe: null, candidates: [], needsPick: false, defaultEnabled: true, enabled: true, folderRoot: null },
    { id: "epic:Fortnite", name: "Fortnite", source: "epic", installDir: "C:\\Program Files\\Epic Games\\Fortnite", exe: "C:\\Program Files\\Epic Games\\Fortnite\\FortniteGame\\Binaries\\Win64\\FortniteClient-Win64-Shipping.exe", candidates: [], needsPick: false, defaultEnabled: true, enabled: false, folderRoot: null },
    { id: `folder:${root}\\hollow knight`.toLowerCase(), name: "Hollow Knight", source: "folder", installDir: null, exe: `${root}\\Hollow Knight\\hollow_knight.exe`, candidates: [], needsPick: false, defaultEnabled: true, enabled: true, folderRoot: root },
    { id: `folder:${root}\\game folder 2`.toLowerCase(), name: "Game Folder 2", source: "folder", installDir: null, exe: null, candidates: [`${root}\\Game Folder 2\\Game.exe`, `${root}\\Game Folder 2\\bin\\GameDX12.exe`, `${root}\\Game Folder 2\\tools\\Editor.exe`], needsPick: true, defaultEnabled: true, enabled: true, folderRoot: root },
    { id: "manual:c:\\emu\\cemu\\cemu.exe", name: "Cemu", source: "manual", installDir: null, exe: "C:\\Emu\\Cemu\\Cemu.exe", candidates: [], needsPick: false, defaultEnabled: true, enabled: true, folderRoot: null },
    { id: "steam:431960", name: "Wallpaper Engine", source: "steam", installDir: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\wallpaper_engine", exe: null, candidates: [], needsPick: false, defaultEnabled: false, enabled: false, folderRoot: null },
  ],
  folders: [{ path: root, exists: true, games: 2, needsPick: 1 }],
};

const displays: DisplaysView = {
  displays: [{ id: "tv", name: "Beyond TV", primary: true, hdrSupported: true, hdrOn: status.hdr.on, api: "v2" }],
  selected: "tv",
};

export async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const r = (v: unknown) => v as T;
  switch (cmd) {
    case "get_status":
      return r(status);
    case "get_library":
      return r(library);
    case "get_settings":
      return r(settings);
    case "list_displays":
      return r(displays);
    case "set_language":
      settings = { ...settings, language: args?.language as "ar" | "en" };
      return r(settings);
    case "set_general":
      settings = { ...settings, [args?.key as string]: args?.value };
      return r(settings);
    case "set_source":
      settings = { ...settings, sources: { ...settings.sources, [args?.source as string]: args?.enabled } };
      return r(settings);
    case "set_game_enabled": {
      const g = library.games.find((x) => x.id === args?.id);
      if (g) g.enabled = !!args?.enabled;
      return r(settings);
    }
    case "hdr_test":
      await new Promise((ok) => setTimeout(ok, 600));
      return r(args?.on);
    default:
      return r(settings);
  }
}
