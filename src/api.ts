import { invoke as tauriInvoke } from "@tauri-apps/api/core";

/** True inside the KaiSync window; false in a plain browser (dev preview with mock data). */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) return tauriInvoke<T>(cmd, args);
  const { mockInvoke } = await import("./dev/mock");
  return mockInvoke<T>(cmd, args);
}

export type Mode = "idle" | "video" | "game";
export type Source = "steam" | "epic" | "xbox" | "folder" | "manual";

export interface HdrStatus {
  display: string | null;
  supported: boolean;
  on: boolean;
  onByKaisync: boolean;
  offByKaisync: boolean;
}

export interface Status {
  mode: Mode;
  detail: string | null;
  paused: boolean;
  rawMode: Mode;
  rawDetail: string | null;
  scanning: boolean;
  hdr: HdrStatus;
}

export interface Display {
  id: string;
  name: string;
  primary: boolean;
  hdrSupported: boolean;
  hdrOn: boolean;
  api: "v1" | "v2";
}

export interface Game {
  id: string;
  name: string;
  source: Source;
  installDir: string | null;
  exe: string | null;
  candidates: string[];
  needsPick: boolean;
  defaultEnabled: boolean;
  enabled: boolean;
  folderRoot: string | null;
}

export interface FolderReport {
  path: string;
  exists: boolean;
  games: number;
  needsPick: number;
}

export interface PlayerInfo {
  name: string;
  path: string;
  exists: boolean;
}

export interface LibraryView {
  games: Game[];
  folders: FolderReport[];
  player: PlayerInfo | null;
}

export interface Settings {
  language: "ar" | "en";
  sources: { steam: boolean; epic: boolean; xbox: boolean };
  gameFolders: string[];
  folderPicks: Record<string, string>;
  manualGames: { path: string; name: string }[];
  gameEnabled: Record<string, boolean>;
  hdrDisplay: string | null;
  playerPath: string | null;
  autostart: boolean;
  desktopShortcut: boolean;
  debounceMs: number;
}

export interface DisplaysView {
  displays: Display[];
  selected: string | null;
}

export const api = {
  getStatus: () => invoke<Status>("get_status"),
  getLibrary: () => invoke<LibraryView>("get_library"),
  getSettings: () => invoke<Settings>("get_settings"),
  setPaused: (paused: boolean) => invoke<void>("set_paused", { paused }),
  rescan: () => invoke<void>("rescan"),
  setSource: (source: string, enabled: boolean) => invoke<Settings>("set_source", { source, enabled }),
  addGameFolder: (path: string) => invoke<Settings>("add_game_folder", { path }),
  removeGameFolder: (path: string) => invoke<Settings>("remove_game_folder", { path }),
  addManualGame: (path: string) => invoke<Settings>("add_manual_game", { path }),
  removeManualGame: (path: string) => invoke<Settings>("remove_manual_game", { path }),
  setGameEnabled: (id: string, enabled: boolean) => invoke<Settings>("set_game_enabled", { id, enabled }),
  pickFolderExe: (id: string, exe: string) => invoke<Settings>("pick_folder_exe", { id, exe }),
  setPlayer: (path: string) => invoke<Settings>("set_player", { path }),
  setLanguage: (language: string) => invoke<Settings>("set_language", { language }),
  setGeneral: (key: "autostart" | "desktopShortcut", value: boolean) => invoke<Settings>("set_general", { key, value }),
  openLogsFolder: () => invoke<void>("open_logs_folder"),
  listDisplays: () => invoke<DisplaysView>("list_displays"),
  setHdrDisplay: (id: string) => invoke<Settings>("set_hdr_display", { id }),
  hdrTest: (on: boolean) => invoke<boolean>("hdr_test", { on }),
};
