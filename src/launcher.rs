use std::path::PathBuf;

use iced::widget::{image, svg};
use rust_embed::RustEmbed;

/// Icons bundled into the binary at compile time from `assets/icons/`.
#[derive(RustEmbed)]
#[folder = "assets/icons/"]
struct EmbeddedIcons;

/// A resolved, ready-to-render icon handle.
#[derive(Clone)]
pub enum IconHandle {
    Vector(svg::Handle),
    Raster(image::Handle),
}

#[derive(Clone)]
pub struct AppEntry {
    pub name: String,
    pub exec: String,
    pub terminal: bool,
    pub icon: Option<IconHandle>,
}

pub fn scan_applications() -> Vec<AppEntry> {
    let mut entries = Vec::new();

    let mut dirs = vec![PathBuf::from("/usr/share/applications")];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/applications"));
    }

    for dir in &dirs {
        if !dir.exists() {
            continue;
        }
        let read_dir = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let desktop = match freedesktop_desktop_entry::DesktopEntry::from_str(
                &path,
                &content,
                None::<&[&str]>,
            ) {
                Ok(d) => d,
                Err(_) => continue,
            };

            if desktop.no_display() || desktop.hidden() {
                continue;
            }

            let locales: &[&str] = &[];
            let name = match desktop.name(locales) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let exec = match desktop.exec() {
                Some(e) => e.to_string(),
                None => continue,
            };

            // Prefer an embedded SVG (fetched by fetch-icons.sh) over whatever
            // the system resolves — Chrome/Brave web-app .desktop files use
            // opaque icon names (chrome-<hash>-Default) that point to low-res
            // PNGs.  If the system lookup didn't yield a vector, try the
            // embedded icons by normalising the app's display name.
            let system_icon = desktop.icon().and_then(resolve_icon);
            let icon = match &system_icon {
                Some(IconHandle::Vector(_)) => system_icon,
                _ => try_embedded_by_name(&name).or(system_icon),
            };

            let terminal = content.lines().any(|l| l.trim() == "Terminal=true");

            entries.push(AppEntry { name, exec, terminal, icon });
        }
    }

    entries.sort_by(|a, b| {
        match (a.icon.is_some(), b.icon.is_some()) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    entries
}

/// Return candidate icon filenames (without extension) derived from an app's
/// display name, in preference order.  Duplicates are suppressed.
///
/// Examples:
///   "WhatsApp Web" → ["whatsapp-web", "whatsappweb", "whatsapp"]
///   "Google Gemini" → ["google-gemini", "googlegemini"]
///   "Claude"        → ["claude"]
pub(crate) fn name_candidates(name: &str) -> Vec<String> {
    let base = name.to_lowercase();
    let stripped = base.replace(" web", "");
    let stripped = stripped.trim();
    let mut seen = std::collections::HashSet::new();
    let raw = [
        base.replace(' ', "-"),
        base.replace(' ', ""),
        stripped.replace(' ', "-"),
        stripped.replace(' ', ""),
    ];
    raw.into_iter()
        .filter(|s| !s.is_empty() && seen.insert(s.clone()))
        .collect()
}

/// Try to find an embedded icon by normalising the app's display name.
fn try_embedded_by_name(name: &str) -> Option<IconHandle> {
    for candidate in name_candidates(name) {
        for ext in ["svg", "png"] {
            let filename = format!("{candidate}.{ext}");
            if let Some(file) = EmbeddedIcons::get(&filename) {
                let data: Vec<u8> = file.data.into_owned();
                return Some(if ext == "svg" {
                    IconHandle::Vector(svg::Handle::from_memory(data))
                } else {
                    IconHandle::Raster(image::Handle::from_bytes(data))
                });
            }
        }
    }
    None
}

fn resolve_icon(icon_name: &str) -> Option<IconHandle> {
    // 1. Embedded assets (compiled into the binary).
    for ext in ["svg", "png"] {
        let filename = format!("{icon_name}.{ext}");
        if let Some(file) = EmbeddedIcons::get(&filename) {
            let data: Vec<u8> = file.data.into_owned();
            return Some(if ext == "svg" {
                IconHandle::Vector(svg::Handle::from_memory(data))
            } else {
                IconHandle::Raster(image::Handle::from_bytes(data))
            });
        }
    }

    // 2. Absolute path in the .desktop file.
    let p = PathBuf::from(icon_name);
    if p.is_absolute() && p.exists() {
        return Some(path_handle(&p));
    }

    // 3. System icon theme directories.
    let home = std::env::var("HOME").unwrap_or_default();
    let system_dirs = [
        format!("{home}/.local/share/icons/hicolor/scalable/apps"),
        format!("{home}/.local/share/icons/hicolor/96x96/apps"),
        format!("{home}/.local/share/icons/hicolor/64x64/apps"),
        format!("{home}/.local/share/icons/hicolor/48x48/apps"),
        format!("{home}/.local/share/icons"),
        "/usr/share/icons/hicolor/scalable/apps".to_string(),
        "/usr/share/icons/hicolor/96x96/apps".to_string(),
        "/usr/share/icons/hicolor/64x64/apps".to_string(),
        "/usr/share/icons/hicolor/48x48/apps".to_string(),
        "/usr/share/pixmaps".to_string(),
    ];

    for dir in &system_dirs {
        for ext in ["svg", "png"] {
            let candidate = PathBuf::from(dir).join(format!("{icon_name}.{ext}"));
            if candidate.exists() {
                return Some(path_handle(&candidate));
            }
        }
    }

    None
}

fn path_handle(path: &PathBuf) -> IconHandle {
    if path.extension().and_then(|e| e.to_str()) == Some("svg") {
        IconHandle::Vector(svg::Handle::from_path(path))
    } else {
        IconHandle::Raster(image::Handle::from_path(path))
    }
}

/// Strip desktop entry field codes (§ 4 of the spec) from an Exec value.
pub(crate) fn clean_exec(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|t| {
            !matches!(
                *t,
                "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%i" | "%c" | "%k"
                    | "%v" | "%m"
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Find an available terminal emulator, returning (binary, exec_flag).
/// Most terminals use `-e`; wezterm uses `start --`.
fn find_terminal() -> Option<(&'static str, &'static str)> {
    let candidates: &[(&str, &str)] = &[
        ("foot",     "-e"),
        ("kitty",    "-e"),
        ("alacritty","-e"),
        ("ghostty",  "-e"),
        ("wezterm",  "start --"),
        ("xterm",    "-e"),
    ];
    // Honour $TERMINAL if set and it matches one of the known candidates.
    if let Ok(t) = std::env::var("TERMINAL") {
        if let Some(&entry) = candidates.iter().find(|(bin, _)| *bin == t.as_str()) {
            return Some(entry);
        }
    }
    candidates.iter().find(|(bin, _)| {
        std::process::Command::new("sh")
            .args(["-c", &format!("command -v {bin}")])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }).copied()
}

/// Strip desktop field codes and spawn the application.
/// When `terminal` is true the exec is wrapped with a terminal emulator.
pub fn launch_app(exec: &str, terminal: bool) {
    let clean = clean_exec(exec);

    if terminal {
        if let Some((term, flag)) = find_terminal() {
            let _ = std::process::Command::new("sh")
                .args(["-c", &format!("{term} {flag} {clean}")])
                .spawn();
            return;
        }
        // No terminal found — fall through and try to launch directly.
    }

    let mut parts = clean.split_whitespace();
    if let Some(cmd) = parts.next() {
        let args: Vec<&str> = parts.collect();
        let _ = std::process::Command::new(cmd).args(args).spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::{clean_exec, name_candidates};

    // ── name_candidates ───────────────────────────────────────────────────────

    #[test]
    fn simple_name_lowercased() {
        assert_eq!(name_candidates("Claude"), vec!["claude"]);
    }

    #[test]
    fn two_word_name_produces_dash_and_joined() {
        assert_eq!(
            name_candidates("Google Gemini"),
            vec!["google-gemini", "googlegemini"]
        );
    }

    #[test]
    fn web_suffix_stripped() {
        // "WhatsApp Web" → whatsapp-web, whatsappweb, whatsapp (no dup)
        let c = name_candidates("WhatsApp Web");
        assert_eq!(c, vec!["whatsapp-web", "whatsappweb", "whatsapp"]);
    }

    #[test]
    fn web_suffix_only_entry_doesnt_produce_empty() {
        // "Web" alone strips to "" which should be filtered out
        let c = name_candidates("Web");
        assert!(!c.contains(&String::new()));
    }

    #[test]
    fn no_duplicates_when_stripped_matches_original() {
        // "OpenAI" has no spaces so dash/joined variants are the same
        let c = name_candidates("OpenAI");
        assert_eq!(c, vec!["openai"]);
    }

    #[test]
    fn single_word_no_web() {
        assert_eq!(name_candidates("Spotify"), vec!["spotify"]);
    }

    #[test]
    fn strips_common_field_codes() {
        assert_eq!(clean_exec("firefox %U"), "firefox");
        assert_eq!(clean_exec("code %F"), "code");
        assert_eq!(clean_exec("gimp %f"), "gimp");
        assert_eq!(clean_exec("xdg-open %u"), "xdg-open");
    }

    #[test]
    fn strips_all_field_codes() {
        let all = "app %f %F %u %U %d %D %n %N %i %c %k %v %m";
        assert_eq!(clean_exec(all), "app");
    }

    #[test]
    fn preserves_real_args() {
        assert_eq!(
            clean_exec("env FOO=bar myapp --flag %U"),
            "env FOO=bar myapp --flag"
        );
    }

    #[test]
    fn no_field_codes_unchanged() {
        assert_eq!(clean_exec("alacritty --title Launcher"), "alacritty --title Launcher");
    }

    #[test]
    fn empty_string() {
        assert_eq!(clean_exec(""), "");
    }

    #[test]
    fn only_field_codes_yields_empty() {
        assert_eq!(clean_exec("%f %F %u %U"), "");
    }
}
