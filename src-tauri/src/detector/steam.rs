//! Steam: `libraryfolders.vdf` -> every `appmanifest_*.acf` -> `steamapps\common\<installdir>`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use winreg::{enums::*, RegKey};

use super::{ignore, Game, Source};

/// Minimal Valve KeyValues (VDF) tree.
#[derive(Debug)]
pub enum Vdf {
    Str(String),
    Obj(Vec<(String, Vdf)>),
}

impl Vdf {
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Obj(items) => items
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(Vdf::Str(s)) => Some(s),
            _ => None,
        }
    }
}

/// Parse a VDF document into a root object.
pub fn parse_vdf(text: &str) -> Vdf {
    let mut chars = text.chars().peekable();

    fn token(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<String> {
        loop {
            match chars.peek()? {
                c if c.is_whitespace() => {
                    chars.next();
                }
                '/' => {
                    // `// comment` to end of line
                    chars.next();
                    if chars.peek() == Some(&'/') {
                        for c in chars.by_ref() {
                            if c == '\n' {
                                break;
                            }
                        }
                    }
                }
                '{' | '}' => return chars.next().map(|c| c.to_string()),
                '"' => {
                    chars.next();
                    let mut s = String::new();
                    while let Some(c) = chars.next() {
                        match c {
                            '"' => break,
                            '\\' => match chars.next() {
                                Some('n') => s.push('\n'),
                                Some('t') => s.push('\t'),
                                Some(o) => s.push(o),
                                None => break,
                            },
                            o => s.push(o),
                        }
                    }
                    // Mark as a string token (can't collide with a bare `{`/`}` token).
                    return Some(format!("\u{0}{s}"));
                }
                _ => {
                    let mut s = String::new();
                    while let Some(&c) = chars.peek() {
                        if c.is_whitespace() || c == '{' || c == '}' || c == '"' {
                            break;
                        }
                        s.push(c);
                        chars.next();
                    }
                    return Some(format!("\u{0}{s}"));
                }
            }
        }
    }

    fn object(chars: &mut std::iter::Peekable<std::str::Chars>) -> Vec<(String, Vdf)> {
        let mut items = Vec::new();
        while let Some(t) = token(chars) {
            if t == "}" {
                break;
            }
            if t == "{" {
                // malformed; skip nested block
                object(chars);
                continue;
            }
            let key = t.trim_start_matches('\u{0}').to_string();
            match token(chars) {
                Some(v) if v == "{" => items.push((key, Vdf::Obj(object(chars)))),
                Some(v) if v == "}" => break,
                Some(v) => items.push((key, Vdf::Str(v.trim_start_matches('\u{0}').to_string()))),
                None => break,
            }
        }
        items
    }

    Vdf::Obj(object(&mut chars))
}

fn steam_root() -> Option<PathBuf> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(k) = hkcu.open_subkey(r"Software\Valve\Steam") {
        if let Ok(p) = k.get_value::<String, _>("SteamPath") {
            let p = PathBuf::from(p.replace('/', "\\"));
            if p.exists() {
                return Some(p);
            }
        }
    }
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for key in [r"SOFTWARE\WOW6432Node\Valve\Steam", r"SOFTWARE\Valve\Steam"] {
        if let Ok(k) = hklm.open_subkey(key) {
            if let Ok(p) = k.get_value::<String, _>("InstallPath") {
                let p = PathBuf::from(p);
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    let p = PathBuf::from(r"C:\Program Files (x86)\Steam");
    p.exists().then_some(p)
}

fn library_paths(root: &Path) -> Vec<PathBuf> {
    let mut libs = vec![root.to_path_buf()];
    for vdf in [
        root.join(r"steamapps\libraryfolders.vdf"),
        root.join(r"config\libraryfolders.vdf"),
    ] {
        let Ok(text) = fs::read_to_string(&vdf) else { continue };
        let doc = parse_vdf(&text);
        let Some(Vdf::Obj(entries)) = doc.get("libraryfolders") else { continue };
        for (_, v) in entries {
            let path = match v {
                Vdf::Str(s) if Path::new(s).is_absolute() => Some(s.as_str()), // old format
                Vdf::Obj(_) => v.str("path"),
                _ => None,
            };
            if let Some(p) = path {
                let p = PathBuf::from(p);
                if !libs.iter().any(|l| super::norm(l) == super::norm(&p)) {
                    libs.push(p);
                }
            }
        }
        break;
    }
    libs
}

pub fn scan() -> Vec<Game> {
    let Some(root) = steam_root() else {
        tracing::info!("steam: not installed");
        return Vec::new();
    };
    let mut games = Vec::new();
    for lib in library_paths(&root) {
        let apps = lib.join("steamapps");
        let Ok(dir) = fs::read_dir(&apps) else {
            tracing::warn!("steam: library {} not readable", lib.display());
            continue;
        };
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Ok(text) = fs::read_to_string(entry.path()) else { continue };
            let doc = parse_vdf(&text);
            let Some(state) = doc.get("AppState") else { continue };
            let (Some(appid), Some(installdir)) = (state.str("appid"), state.str("installdir")) else {
                continue;
            };
            if ignore::STEAM_SKIP_APPIDS.contains(&appid) {
                continue;
            }
            let dir = apps.join("common").join(installdir);
            if !dir.is_dir() {
                continue;
            }
            let title = state.str("name").unwrap_or(installdir).to_string();
            let mut g = Game::new(format!("steam:{appid}"), title, Source::Steam);
            g.install_dir = Some(dir);
            g.default_enabled = !ignore::STEAM_DEFAULT_OFF_APPIDS.contains(&appid);
            games.push(g);
        }
    }
    games
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_library_and_manifest() {
        let lib = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps" { "228980" "123" }
	}
	"1" { "path" "D:\\SteamLibrary" }
}"#;
        let d = parse_vdf(lib);
        let Some(Vdf::Obj(e)) = d.get("libraryfolders") else { panic!() };
        assert_eq!(e[0].1.str("path"), Some(r"C:\Program Files (x86)\Steam"));
        assert_eq!(e[1].1.str("path"), Some(r"D:\SteamLibrary"));

        let acf = "\"AppState\"\n{\n\t\"appid\"\t\t\"1245620\"\n\t\"name\"\t\t\"ELDEN RING\"\n\t\"installdir\"\t\t\"ELDEN RING\"\n}";
        let d = parse_vdf(acf);
        let s = d.get("appstate").unwrap();
        assert_eq!(s.str("name"), Some("ELDEN RING"));
        assert_eq!(s.str("installdir"), Some("ELDEN RING"));
    }
}
