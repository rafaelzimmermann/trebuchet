use std::path::PathBuf;
use std::sync::OnceLock;

use iced::widget::{image, svg};
use rust_embed::RustEmbed;
use serde::Deserialize;

/// Icons (and manifest.json) bundled into the binary at compile time from `assets/icons/`.
#[derive(RustEmbed)]
#[folder = "assets/icons/"]
struct EmbeddedIcons;

// ── Manifest ──────────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct ManifestEntry {
    file: String,
    #[serde(default)]
    wm_class: Vec<String>,
    #[serde(default)]
    icon_name: Vec<String>,
    #[serde(default)]
    app_name: Vec<String>,
}

static MANIFEST: OnceLock<Vec<ManifestEntry>> = OnceLock::new();

fn manifest() -> &'static [ManifestEntry] {
    MANIFEST.get_or_init(|| {
        let Some(file) = EmbeddedIcons::get("manifest.json") else {
            return Vec::new();
        };
        serde_json::from_slice(&file.data).unwrap_or_default()
    })
}

/// Load an embedded icon file by exact filename (e.g. `"code.svg"`).
fn load_embedded(filename: &str) -> Option<IconHandle> {
    let file = EmbeddedIcons::get(filename)?;
    let data: Vec<u8> = file.data.into_owned();
    Some(if filename.ends_with(".svg") {
        IconHandle::Vector(svg::Handle::from_memory(data))
    } else {
        IconHandle::Raster(image::Handle::from_bytes(data))
    })
}

/// A resolved, ready-to-render icon handle.
#[derive(Clone)]
pub enum IconHandle {
    Vector(svg::Handle),
    Raster(image::Handle),
}

/// Fallback icon shown when no icon can be resolved for an entry.
/// A faint rounded square containing a 2×2 grid of tiles — evokes
/// "application" without being tied to any specific look.
pub const FALLBACK_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <rect x="6" y="6" width="52" height="52" rx="12"
        fill="white" fill-opacity="0.06"
        stroke="white" stroke-opacity="0.22" stroke-width="1.5"/>
  <rect x="16" y="19" width="11" height="11" rx="2.5" fill="white" fill-opacity="0.32"/>
  <rect x="31" y="19" width="11" height="11" rx="2.5" fill="white" fill-opacity="0.32"/>
  <rect x="16" y="34" width="11" height="11" rx="2.5" fill="white" fill-opacity="0.32"/>
  <rect x="31" y="34" width="11" height="11" rx="2.5" fill="white" fill-opacity="0.32"/>
</svg>"#;

// ── Name candidates ───────────────────────────────────────────────────────────

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

// ── Low-level icon lookup ─────────────────────────────────────────────────────

/// Try to find an embedded icon by normalising an app's display name.
pub(crate) fn try_embedded_by_name(name: &str) -> Option<IconHandle> {
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
    // Manifest app_name lookup — covers display names whose slugified form
    // doesn't match the icon filename (e.g. "Visual Studio Code" → code.svg).
    let lower = name.to_lowercase();
    for entry in manifest() {
        if entry.app_name.iter().any(|n| n.to_lowercase() == lower) {
            if let Some(handle) = load_embedded(&entry.file) {
                return Some(handle);
            }
        }
    }
    None
}

/// Look up an icon by exact icon name.
/// Search order: embedded assets → manifest icon_name aliases (embedded) →
///               absolute path → system icon theme dirs (all known aliases).
pub(crate) fn resolve_icon(icon_name: &str) -> Option<IconHandle> {
    // 1. Embedded assets — exact filename match.
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

    // 2. Manifest icon_name aliases → embedded file.
    //    e.g. "com.visualstudio.code" → code.svg
    let lower = icon_name.to_lowercase();
    for entry in manifest() {
        if entry.icon_name.iter().any(|n| n.to_lowercase() == lower) {
            if let Some(handle) = load_embedded(&entry.file) {
                return Some(handle);
            }
        }
    }

    // 3. Absolute path in the .desktop file.
    let p = PathBuf::from(icon_name);
    if p.is_absolute() && p.exists() {
        return Some(path_handle(&p));
    }

    // 4. System icon theme directories — search the requested name plus any
    //    manifest icon_name aliases so that e.g. resolve_icon("code") can
    //    find "com.visualstudio.code.svg" installed by VS Code.
    let mut search_names: Vec<&str> = vec![icon_name];
    for entry in manifest() {
        if entry.icon_name.iter().any(|n| n.to_lowercase() == lower) {
            for alias in &entry.icon_name {
                if alias.to_lowercase() != lower {
                    search_names.push(alias.as_str());
                }
            }
            break;
        }
    }

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
        for name in &search_names {
            for ext in ["svg", "png"] {
                let candidate = PathBuf::from(dir).join(format!("{name}.{ext}"));
                if candidate.exists() {
                    return Some(path_handle(&candidate));
                }
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

// ── Window icon lookup ────────────────────────────────────────────────────────

/// Resolve an icon for a running Hyprland window given its WM class and
/// initial title.  Strategies tried in order (first match wins):
///
/// 1. Exact class as icon name (e.g. "firefox" → "firefox.svg")
/// 2. Last dot-segment of a reverse-DNS class
///    (e.g. "com.mitchellh.ghostty" → "ghostty")
/// 3. `initial_title` as a display-name lookup via `try_embedded_by_name`
///    (covers apps whose class is an opaque bundle ID but whose initial title
///    matches the human-readable name, e.g. "Ghostty")
pub(crate) fn icon_for_window(class: &str, initial_title: &str) -> Option<IconHandle> {
    let lower = class.to_lowercase();

    // 1. Exact class as icon name (e.g. "firefox" → "firefox.svg")
    resolve_icon(class)
        // 2. Lowercased class (e.g. "Code" → "code.svg" for VS Code)
        .or_else(|| if lower != class { resolve_icon(&lower) } else { None })
        // 3. Last dot-segment of a reverse-DNS class, lowercased
        //    (e.g. "com.mitchellh.ghostty" → "ghostty")
        .or_else(|| {
            let stem = lower.rsplit('.').next().unwrap_or(lower.as_str());
            if stem != lower { resolve_icon(stem) } else { None }
        })
        // 4. initialTitle as a display-name lookup
        //    (covers apps whose class is an opaque bundle ID but whose initial
        //    title matches the human-readable name, e.g. "Ghostty")
        .or_else(|| {
            if !initial_title.is_empty() { try_embedded_by_name(initial_title) } else { None }
        })
        // 5. Manifest wm_class lookup — explicit WM-class → embedded file mapping
        //    written by fetch-icons.sh from StartupWMClass= in .desktop files.
        .or_else(|| {
            for entry in manifest() {
                if entry.wm_class.iter().any(|c| c.to_lowercase() == lower) {
                    if let Some(handle) = load_embedded(&entry.file) {
                        return Some(handle);
                    }
                }
            }
            None
        })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::name_candidates;

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
        let c = name_candidates("WhatsApp Web");
        assert_eq!(c, vec!["whatsapp-web", "whatsappweb", "whatsapp"]);
    }

    #[test]
    fn web_suffix_only_entry_doesnt_produce_empty() {
        let c = name_candidates("Web");
        assert!(!c.contains(&String::new()));
    }

    #[test]
    fn no_duplicates_when_stripped_matches_original() {
        let c = name_candidates("OpenAI");
        assert_eq!(c, vec!["openai"]);
    }

    #[test]
    fn single_word_no_web() {
        assert_eq!(name_candidates("Spotify"), vec!["spotify"]);
    }
}
