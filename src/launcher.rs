use std::path::PathBuf;

pub struct AppEntry {
    pub name: String,
    pub exec: String,
    pub icon: Option<PathBuf>,
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

fn resolve_icon(icon_name: &str) -> Option<PathBuf> {
    let p = PathBuf::from(icon_name);
    if p.is_absolute() && p.exists() {
        return Some(p);
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let search_dirs = [
        format!("{home}/.local/share/icons/hicolor/96x96/apps"),
        format!("{home}/.local/share/icons/hicolor/48x48/apps"),
        format!("{home}/.local/share/icons/hicolor/scalable/apps"),
        format!("{home}/.local/share/icons"),
        "/usr/share/icons/hicolor/96x96/apps".to_string(),
        "/usr/share/icons/hicolor/48x48/apps".to_string(),
        "/usr/share/icons/hicolor/scalable/apps".to_string(),
        "/usr/share/pixmaps".to_string(),
    ];

    for dir in &search_dirs {
        for ext in ["png", "svg"] {
            let candidate = PathBuf::from(dir).join(format!("{icon_name}.{ext}"));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Strip desktop field codes and spawn the application.
pub fn launch_app(exec: &str) {
    let clean: String = exec
        .split_whitespace()
        .filter(|t| {
            !matches!(
                *t,
                "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%i" | "%c" | "%k"
                    | "%v" | "%m"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");

    let mut parts = clean.split_whitespace();
    if let Some(cmd) = parts.next() {
        let args: Vec<&str> = parts.collect();
        let _ = std::process::Command::new(cmd).args(args).spawn();
    }
}
