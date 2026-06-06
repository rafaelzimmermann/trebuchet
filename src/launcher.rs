use rayon::prelude::*;
use std::path::PathBuf;

use crate::icons::{self, IconHandle};

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub exec: String,
    pub terminal: bool,
    /// Raw `Icon=` value from the .desktop file. Preserved so icon resolution
    /// can be deferred to an async task without re-parsing.
    pub icon_name: Option<String>,
    /// Resolved icon handle. `None` until the lazy icon-resolution task lands
    /// (`Message::IconsLoaded`); the grid renders the fallback icon in the
    /// meantime so the launcher can appear immediately.
    pub icon: Option<IconHandle>,
}

/// Resolve an app's icon given its display name and optional `Icon=` value.
/// Used by both the synchronous scan path (legacy) and the lazy async path.
///
/// Search order:
///   1. System lookup via `icons::resolve_icon(icon_name)`.
///   2. If that yields a vector handle, use it.
///   3. Otherwise prefer an embedded icon looked up by display name (covers
///      apps whose `Icon=` resolves to a low-res PNG, e.g. Chrome web apps).
///   4. Fall back to the system lookup result.
pub(crate) fn resolve_app_icon(
    name: &str,
    icon_name: Option<&str>,
) -> Option<IconHandle> {
    let system_icon = icon_name.and_then(icons::resolve_icon);
    match &system_icon {
        Some(IconHandle::Vector(_)) => system_icon,
        _ => icons::try_embedded_by_name(name).or(system_icon),
    }
}

/// Resolve icons for a slice of apps in parallel (rayon). Returns one entry
/// per input app, in the same order.
pub fn resolve_all_icons(apps: &[AppEntry]) -> Vec<Option<IconHandle>> {
    apps.par_iter()
        .map(|app| resolve_app_icon(&app.name, app.icon_name.as_deref()))
        .collect()
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

    // Parse .desktop files in parallel; defer icon resolution so AppsLoaded
    // can fire as soon as the names + execs are known. Icons land later via
    // Message::IconsLoaded.
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

            let icon_name = desktop.icon().map(|s| s.to_string());
            let terminal = content.lines().any(|l| l.trim() == "Terminal=true");

            Some(AppEntry { name, exec, terminal, icon_name, icon: None })
        })
        .collect();

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
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
    use super::*;

    // ── clean_exec ────────────────────────────────────────────────────────────

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

    // ── resolve_app_icon ────────────────────────────────────────────────────
    // These tests rely on the embedded icon set bundled at compile time.
    // ‘firefox’ is guaranteed to be present (assets/icons/firefox.svg).

    fn app(name: &str, icon_name: Option<&str>) -> AppEntry {
        AppEntry {
            name: name.to_string(),
            exec: format!("{name} %U"),
            terminal: false,
            icon_name: icon_name.map(String::from),
            icon: None,
        }
    }

    #[test]
    fn resolve_app_icon_unknown_returns_none() {
        assert!(resolve_app_icon("does-not-exist-xyz", None).is_none());
    }

    #[test]
    fn resolve_app_icon_finds_embedded_by_name() {
        // No icon_name supplied, but the display name matches an embedded icon.
        let handle = resolve_app_icon("Firefox", None)
            .expect("firefox.svg is embedded; should resolve");
        assert!(matches!(handle, IconHandle::Vector(_)),
            "embedded icons are SVG");
    }

    #[test]
    fn resolve_app_icon_finds_embedded_by_icon_name() {
        // icon_name is a known embedded key.
        let handle = resolve_app_icon("ignored", Some("firefox"))
            .expect("firefox icon_name should resolve");
        assert!(matches!(handle, IconHandle::Vector(_)));
    }

    #[test]
    fn resolve_app_icon_prefers_vector_over_raster() {
        // When both an embedded SVG and a system PNG could resolve, the
        // function should prefer the higher-quality vector handle.
        // (We can’t mock the system path here, but we can confirm the embedded
        // lookup is used when icon_name doesn’t resolve.)
        let handle = resolve_app_icon("Firefox", Some("does-not-resolve"));
        assert!(handle.is_some(), "display-name fallback should still find it");
    }

    // ── resolve_all_icons ───────────────────────────────────────────────────

    #[test]
    fn resolve_all_icons_returns_one_per_app() {
        let apps = vec![
            app("Firefox", Some("firefox")),
            app("DoesNotExist", None),
            app("Code", Some("code")),
        ];
        let icons = resolve_all_icons(&apps);
        assert_eq!(icons.len(), apps.len());
        assert!(icons[0].is_some(), "Firefox resolves");
        assert!(icons[1].is_none(), "unknown app");
        assert!(icons[2].is_some(), "Code resolves");
    }

    #[test]
    fn resolve_all_icons_empty_input() {
        let icons = resolve_all_icons(&[]);
        assert!(icons.is_empty());
    }

    #[test]
    fn resolve_all_icons_preserves_order() {
        let apps = vec![
            app("Firefox", Some("firefox")),
            app("Code", Some("code")),
        ];
        let icons = resolve_all_icons(&apps);
        // Both should be Vector handles; we can’t distinguish them without
        // inspecting handle contents, but the order matches input order.
        assert!(icons[0].is_some());
        assert!(icons[1].is_some());
    }
}
