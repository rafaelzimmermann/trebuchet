use std::collections::HashMap;
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
#[derive(Debug, Clone)]
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

// ── System icon directory index ──────────────────────────────────────────────
// Replaces the old per-call `path.exists()` waterfall (up to 10 dirs × 2 exts
// × N aliases per app ≈ thousands of stat() syscalls per scan) with a single
// read_dir per directory, cached for the process lifetime. Trebuchet is
// short-lived so a process-local cache is sufficient.

/// Scan each icon directory once and return an ordered index of
/// `(dir, { file_stem → full_path })`. Missing directories are skipped.
///
/// When both `<stem>.svg` and `<stem>.png` exist in the same directory, the
/// SVG entry wins — matching the previous `for ext in ["svg", "png"]` order.
/// Subdirectories are skipped (they are not icon files).
fn build_dir_index(dirs: &[PathBuf]) -> Vec<(PathBuf, HashMap<String, PathBuf>)> {
    let mut index = Vec::with_capacity(dirs.len());
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        let mut map: HashMap<String, PathBuf> = HashMap::new();
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() { continue; }

            let path = entry.path();
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            let new_is_svg = path.extension().and_then(|e| e.to_str()) == Some("svg");
            let existing_is_svg =
                map.get(stem).and_then(|p| p.extension()).and_then(|e| e.to_str())
                    == Some("svg");

            // Insert when no entry yet, or when the new file is svg and the
            // existing one isn't (svg > png > anything else).
            if !map.contains_key(stem) || (new_is_svg && !existing_is_svg) {
                map.insert(stem.to_string(), path);
            }
        }
        index.push((dir.clone(), map));
    }
    index
}

/// Cached index of system icon directories, built once on first access.
fn icon_dirs_index() -> &'static [(PathBuf, HashMap<String, PathBuf>)] {
    static CACHE: OnceLock<Vec<(PathBuf, HashMap<String, PathBuf>)>> = OnceLock::new();
    CACHE.get_or_init(|| build_dir_index(&icon_search_dirs()))
}

/// Ordered list of icon directories probed by [`resolve_icon`].
///
/// User-side (`~/.local/share/icons/hicolor/…`) takes priority over system
/// (`/usr/share/…`) so per-user overrides win. Sizes probed:
///
/// - **scalable** — SVG, renders cleanly at any configured `icon_size`.
/// - **96×96** — exact match for the default `icon_size = 96`.
/// - **48×48** — common fallback for apps that don’t ship a 96×96 raster.
///
/// Dropped intentionally:
/// - `~/.local/share/icons` (the parent of `hicolor/`) — never contains
///   flat icon files; only subdirectories.
/// - `64x64` raster — intermediate between 48 and 96; rare in practice and
///   usually duplicated by an entry in another size.
fn icon_search_dirs() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    [
        // user-side hicolor (priority: scalable → 96 → 48)
        format!("{home}/.local/share/icons/hicolor/scalable/apps"),
        format!("{home}/.local/share/icons/hicolor/96x96/apps"),
        format!("{home}/.local/share/icons/hicolor/48x48/apps"),
        // system hicolor (same size order)
        "/usr/share/icons/hicolor/scalable/apps".to_string(),
        "/usr/share/icons/hicolor/96x96/apps".to_string(),
        "/usr/share/icons/hicolor/48x48/apps".to_string(),
        // legacy fallback
        "/usr/share/pixmaps".to_string(),
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

/// Look up `names` across an ordered index, returning the first matching path.
/// Iterates dirs in order, then names within each dir — same priority as the
/// previous `for dir ... for name ... for ext` waterfall, but with O(1) hash
/// lookups instead of O(N) stat() syscalls.
fn lookup_in_index(
    names: &[&str],
    index: &[(PathBuf, HashMap<String, PathBuf>)],
) -> Option<PathBuf> {
    for (_, files) in index {
        for name in names {
            if let Some(path) = files.get(*name) {
                return Some(path.clone());
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

    let path = lookup_in_index(&search_names, icon_dirs_index());
    path.map(|p| path_handle(&p))
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
    use super::{build_dir_index, lookup_in_index, name_candidates};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

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

    // ── build_dir_index ───────────────────────────────────────────────────────────

    #[test]
    fn build_dir_index_reads_file_stems() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("firefox.svg"), b"<svg/>").unwrap();
        fs::write(dir.path().join("code.png"), b"PNG").unwrap();

        let index = build_dir_index(&[dir.path().to_path_buf()]);
        assert_eq!(index.len(), 1);
        let (_, map) = &index[0];
        assert!(map.contains_key("firefox"), "firefox stem should be indexed");
        assert!(map.contains_key("code"), "code stem should be indexed");
    }

    #[test]
    fn build_dir_index_skips_missing_dirs() {
        let index =
            build_dir_index(&[PathBuf::from("/nonexistent/trebuchet/test/abc")]);
        assert!(index.is_empty());
    }

    #[test]
    fn build_dir_index_preserves_directory_order() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        fs::write(d1.path().join("a.svg"), b"").unwrap();
        fs::write(d2.path().join("b.svg"), b"").unwrap();

        let index =
            build_dir_index(&[d1.path().to_path_buf(), d2.path().to_path_buf()]);
        assert_eq!(index.len(), 2);
        assert_eq!(index[0].0, d1.path());
        assert_eq!(index[1].0, d2.path());
    }

    #[test]
    fn build_dir_index_prefers_svg_over_png_regardless_of_order() {
        // Build two dirs: one with png first, one with svg first.
        // Hash iteration order is non-deterministic so we test both shapes.
        let dir_png_first = tempdir().unwrap();
        fs::write(dir_png_first.path().join("app.png"), b"PNG").unwrap();
        fs::write(dir_png_first.path().join("app.svg"), b"<svg/>").unwrap();

        let dir_svg_first = tempdir().unwrap();
        fs::write(dir_svg_first.path().join("app.svg"), b"<svg/>").unwrap();
        fs::write(dir_svg_first.path().join("app.png"), b"PNG").unwrap();

        for dir in [dir_png_first, dir_svg_first] {
            let index = build_dir_index(&[dir.path().to_path_buf()]);
            let (_, map) = &index[0];
            let path =
                map.get("app").expect("app should be indexed regardless of order");
            assert_eq!(
                path.extension().and_then(|e| e.to_str()),
                Some("svg"),
                "svg should win regardless of insertion order"
            );
        }
    }

    #[test]
    fn build_dir_index_keeps_png_when_only_png_exists() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("app.png"), b"PNG").unwrap();

        let index = build_dir_index(&[dir.path().to_path_buf()]);
        let (_, map) = &index[0];
        let path = map.get("app").unwrap();
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("png"));
    }

    #[test]
    fn build_dir_index_skips_subdirectories() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("hicolor")).unwrap();
        fs::write(dir.path().join("app.svg"), b"<svg/>").unwrap();

        let index = build_dir_index(&[dir.path().to_path_buf()]);
        let (_, map) = &index[0];
        assert!(map.contains_key("app"));
        assert!(!map.contains_key("hicolor"), "subdirs should be skipped");
    }

    #[test]
    fn build_dir_index_handles_reverse_dns_stems() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("com.visualstudio.code.svg"), b"").unwrap();

        let index = build_dir_index(&[dir.path().to_path_buf()]);
        let (_, map) = &index[0];
        assert!(map.contains_key("com.visualstudio.code"));
    }

    // ── lookup_in_index ──────────────────────────────────────────────────────────

    #[test]
    fn lookup_in_index_returns_first_match() {
        let mut map1 = HashMap::new();
        map1.insert("app".to_string(), PathBuf::from("/d1/app.svg"));
        let mut map2 = HashMap::new();
        map2.insert("app".to_string(), PathBuf::from("/d2/app.svg"));

        let index = vec![(PathBuf::from("/d1"), map1), (PathBuf::from("/d2"), map2)];

        assert_eq!(
            lookup_in_index(&["app"], &index),
            Some(PathBuf::from("/d1/app.svg")),
        );
    }

    #[test]
    fn lookup_in_index_returns_none_for_empty_index() {
        let index: Vec<(PathBuf, HashMap<String, PathBuf>)> = vec![];
        assert_eq!(lookup_in_index(&["app"], &index), None);
    }

    #[test]
    fn lookup_in_index_returns_none_for_unknown_name() {
        let map: HashMap<String, PathBuf> = HashMap::new();
        let index = vec![(PathBuf::from("/d"), map)];
        assert_eq!(lookup_in_index(&["nonexistent"], &index), None);
    }

    #[test]
    fn lookup_in_index_falls_through_to_next_dir() {
        let mut map1 = HashMap::new();
        map1.insert("other".to_string(), PathBuf::from("/d1/other.svg"));
        let mut map2 = HashMap::new();
        map2.insert("app".to_string(), PathBuf::from("/d2/app.svg"));

        let index = vec![(PathBuf::from("/d1"), map1), (PathBuf::from("/d2"), map2)];

        assert_eq!(
            lookup_in_index(&["app"], &index),
            Some(PathBuf::from("/d2/app.svg")),
        );
    }

    #[test]
    fn lookup_in_index_dir_priority_over_name_priority() {
        // Matches the original loop order: dir1.name2 wins over dir2.name1
        // because we exhaust dir1 before moving on.
        let mut map1 = HashMap::new();
        map1.insert("second".to_string(), PathBuf::from("/d1/second.svg"));
        let mut map2 = HashMap::new();
        map2.insert("first".to_string(), PathBuf::from("/d2/first.svg"));

        let index = vec![(PathBuf::from("/d1"), map1), (PathBuf::from("/d2"), map2)];

        assert_eq!(
            lookup_in_index(&["first", "second"], &index),
            Some(PathBuf::from("/d1/second.svg")),
            "d1’s second wins over d2’s first — dir is the outer loop"
        );
    }

    #[test]
    fn lookup_in_index_tries_aliases_in_order() {
        let mut map = HashMap::new();
        map.insert("alias".to_string(), PathBuf::from("/d/alias.svg"));
        let index = vec![(PathBuf::from("/d"), map)];

        assert_eq!(
            lookup_in_index(&["primary", "alias"], &index),
            Some(PathBuf::from("/d/alias.svg")),
        );
    }

    // ── icon_search_dirs ─────────────────────────────────────────────────────────

    #[test]
    fn icon_search_dirs_excludes_parent_of_hicolor() {
        // ~/.local/share/icons only contains subdirs (hicolor, …), never flat
        // icon files. Searching it would always miss.
        let dirs = super::icon_search_dirs();
        assert!(
            !dirs
                .iter()
                .any(|p| p.to_string_lossy().ends_with("/.local/share/icons")),
            "parent of hicolor should not be in the probe list: {dirs:?}"
        );
    }

    #[test]
    fn icon_search_dirs_excludes_intermediate_64_raster() {
        // 64x64 is between 48 and 96 and rarely the only size an icon ships
        // at; we keep 48 (common fallback) and 96 (default icon_size) only.
        let dirs = super::icon_search_dirs();
        assert!(
            !dirs.iter().any(|p| p.to_string_lossy().contains("64x64")),
            "64x64 dirs should not be in the probe list: {dirs:?}"
        );
    }

    #[test]
    fn icon_search_dirs_includes_scalable_and_96_and_48() {
        let dirs = super::icon_search_dirs();
        for needle in ["scalable", "96x96", "48x48"] {
            assert!(
                dirs.iter().any(|p| p.to_string_lossy().contains(needle)),
                "expected {needle:?} in probe list: {dirs:?}"
            );
        }
    }

    #[test]
    fn icon_search_dirs_includes_pixmaps() {
        let dirs = super::icon_search_dirs();
        assert!(
            dirs.iter().any(|p| p.to_string_lossy() == "/usr/share/pixmaps"),
            "pixmaps should remain: {dirs:?}"
        );
    }

    #[test]
    fn icon_search_dirs_user_before_system() {
        // User-side overrides must be probed before system-side ones so that
        // ~/.local wins over /usr/share when both have the same stem.
        let dirs = super::icon_search_dirs();
        let home = std::env::var("HOME").unwrap_or_default();
        let first_user = dirs
            .iter()
            .position(|p| p.starts_with(&home))
            .expect("at least one user-side dir should be present");
        let first_system = dirs
            .iter()
            .position(|p| p.to_string_lossy().starts_with("/usr/"))
            .expect("at least one system-side dir should be present");
        assert!(
            first_user < first_system,
            "user dirs must come before system dirs: {dirs:?}"
        );
    }
}
