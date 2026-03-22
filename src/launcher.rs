use rayon::prelude::*;
use std::path::PathBuf;

use crate::icons::{self, IconHandle};

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub exec: String,
    pub terminal: bool,
    pub icon: Option<IconHandle>,
}

pub fn scan_applications() -> Vec<AppEntry> {
    let mut files: Vec<(PathBuf, String)> = Vec::new();

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

            files.push((path, content));
        }
    }

    let mut entries: Vec<AppEntry> = files
        .par_iter()
        .filter_map(|(path, content)| {
            let desktop = match freedesktop_desktop_entry::DesktopEntry::from_str(
                path,
                content,
                None::<&[&str]>,
            ) {
                Ok(d) => d,
                Err(_) => return None,
            };

            if desktop.no_display() || desktop.hidden() {
                return None;
            }

            let locales: &[&str] = &[];
            let name = match desktop.name(locales) {
                Some(n) => n.to_string(),
                None => return None,
            };

            let exec = match desktop.exec() {
                Some(e) => e.to_string(),
                None => return None,
            };

            // Prefer an embedded SVG (fetched by fetch-icons.sh) over whatever
            // the system resolves — Chrome/Brave web-app .desktop files use
            // opaque icon names (chrome-<hash>-Default) that point to low-res
            // PNGs.  If the system lookup didn't yield a vector, try the
            // embedded icons by normalising the app's display name.
            let system_icon = desktop.icon().and_then(icons::resolve_icon);
            let icon = match &system_icon {
                Some(IconHandle::Vector(_)) => system_icon,
                _ => icons::try_embedded_by_name(&name).or(system_icon),
            };

            let terminal = content.lines().any(|l| l.trim() == "Terminal=true");

            Some(AppEntry { name, exec, terminal, icon })
        })
        .collect();

    entries.sort_by(|a, b| {
        match (a.icon.is_some(), b.icon.is_some()) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    entries
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
        ("foot", "-e"),
        ("kitty", "-e"),
        ("alacritty", "-e"),
        ("ghostty", "-e"),
        ("wezterm", "start --"),
        ("xterm", "-e"),
    ];
    // Honour $TERMINAL if set and it matches one of the known candidates.
    if let Ok(t) = std::env::var("TERMINAL") {
        if let Some(&entry) = candidates.iter().find(|(bin, _)| *bin == t.as_str()) {
            return Some(entry);
        }
    }
    candidates
        .iter()
        .find(|(bin, _)| {
            std::process::Command::new("sh")
                .args(["-c", &format!("command -v {bin}")])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
        .copied()
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
    use super::clean_exec;

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
        assert_eq!(
            clean_exec("alacritty --title Launcher"),
            "alacritty --title Launcher"
        );
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
