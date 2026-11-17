//! Built-in ignore lists: launchers, helpers, anti-cheat, installers.

/// Running processes with these names never count as a game (glob, lowercase, `*` wildcard).
const IGNORED_PROCESSES: &[&str] = &[
    // Steam
    "steam.exe", "steamwebhelper.exe", "steamservice.exe", "steamerrorreporter*.exe",
    "gameoverlayui*.exe", "steamtours.exe",
    // Epic
    "epicgameslauncher.exe", "epicwebhelper.exe", "epiconlineservices*.exe",
    "eosoverlayrenderer*.exe", "unrealcefsubprocess.exe",
    // Other launchers / overlays
    "eadesktop.exe", "eabackgroundservice.exe", "eacefsubprocess.exe", "origin*.exe",
    "upc.exe", "uplaywebcore.exe", "ubisoftconnect*.exe", "galaxyclient*.exe",
    "gogalaxy*.exe", "battle.net*.exe", "riotclient*.exe", "xboxpcapp.exe",
    "gamingservices*.exe", "gamebar*.exe", "discord*.exe", "nvcontainer.exe",
    "nvidia overlay.exe", "rtss*.exe", "msiafterburner.exe",
    // Crash handlers / helpers
    "unitycrashhandler*.exe", "crashreportclient*.exe", "crashpad_handler.exe",
    "cefsharp.browsersubprocess.exe", "qtwebengineprocess.exe",
    // Anti-cheat
    "easyanticheat*.exe", "beservice*.exe", "battleye*.exe", "vgc.exe", "vgtray.exe",
    "faceit*.exe", "mhyprot*.exe", "nprotect*.exe", "xigncode*.exe",
    // Installers / redistributables
    "unins*.exe", "setup*.exe", "install*.exe", "vcredist*.exe", "vc_redist*.exe",
    "dxsetup*.exe", "dxwebsetup*.exe", "ue4prereqsetup*.exe", "ueprereqsetup*.exe",
    "dotnetfx*.exe", "ndp*-kb*.exe", "oalinst.exe", "physx*.exe",
];

/// Steam apps that are not games.
pub const STEAM_SKIP_APPIDS: &[&str] = &[
    "228980", // Steamworks Common Redistributables
];

/// Steam apps that run constantly or alongside games: known, but off by default.
pub const STEAM_DEFAULT_OFF_APPIDS: &[&str] = &[
    "431960", // Wallpaper Engine
    "993090", // Lossless Scaling
];

/// Simple glob with `*` wildcards; both sides lowercase.
pub fn glob(pattern: &str, text: &str) -> bool {
    let p = pattern.as_bytes();
    let t = text.as_bytes();
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == b'*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}

pub fn is_ignored(process_name_lower: &str) -> bool {
    IGNORED_PROCESSES.iter().any(|p| glob(p, process_name_lower))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globbing() {
        assert!(glob("unins*.exe", "unins000.exe"));
        assert!(glob("steam.exe", "steam.exe"));
        assert!(!glob("steam.exe", "steamx.exe"));
        assert!(glob("*launcher*", "mygamelauncher.exe"));
        assert!(!glob("setup*.exe", "mysetup.exe"));
        assert!(is_ignored("easyanticheat_eos.exe"));
        assert!(!is_ignored("eldenring.exe"));
    }
}
