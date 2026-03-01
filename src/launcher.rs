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

pub struct AppEntry {
    pub name: String,
    pub exec: String,
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

            let icon = desktop.icon().and_then(resolve_icon);

            entries.push(AppEntry { name, exec, icon });
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

/// Strip desktop field codes and spawn the application.
pub fn launch_app(exec: &str) {
    let clean = clean_exec(exec);
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
