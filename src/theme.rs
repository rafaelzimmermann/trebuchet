use iced::Color;

/// UI colour palette for trebuchet.
///
/// Theme files live in `~/.config/trebuchet/themes/<name>.conf`.
/// Each line is `key = #RRGGBB` or `key = #RRGGBBAA`.
/// Switch themes at runtime with `/theme <name>` in the launcher.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    // ── Window ─────────────────────────────────────────────────────────────
    /// Main window overlay — the dark rounded background.
    pub background: Color,

    // ── Search bar ─────────────────────────────────────────────────────────
    /// Search bar pill background.
    pub search_background: Color,
    /// Search bar pill border.
    pub search_border: Color,
    /// Typed text in the search bar.
    pub search_text: Color,
    /// Placeholder text ("Search apps…", "Ask anything…").
    pub search_placeholder: Color,
    /// Text selection highlight.
    pub search_selection: Color,

    // ── App grid ───────────────────────────────────────────────────────────
    /// App name label text.
    pub app_label: Color,
    /// Background of the currently highlighted app.
    pub app_selected: Color,

    // ── Pagination dots ────────────────────────────────────────────────────
    /// Current-page indicator dot.
    pub dot_active: Color,
    /// Other-page indicator dots.
    pub dot_inactive: Color,

    // ── AI panel ───────────────────────────────────────────────────────────
    /// Idle/hint text and muted UI labels.
    pub ai_idle: Color,
    /// Error message text.
    pub ai_error: Color,
    /// AI response text-area background.
    pub ai_panel: Color,
    /// Inline code highlight background in markdown.
    pub ai_code_background: Color,
    /// Inline code text colour in markdown.
    pub ai_code_text: Color,
    /// Hyperlink colour in markdown responses.
    pub ai_link: Color,

    // ── Command result / terminal ──────────────────────────────────────────
    /// Terminal output-area background.
    pub terminal_background: Color,
    /// Prompt line colour (`$ /command`).
    pub terminal_prompt: Color,
    /// Command output text.
    pub terminal_output: Color,

    // ── Shared elements ────────────────────────────────────────────────────
    /// Background for icon buttons (copy, retry) and the model picker.
    pub button_background: Color,
    /// "Copied to clipboard" confirmation text.
    pub copy_feedback: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background:          hex("#14141eea"),
            search_background:   hex("#ffffff1e"),
            search_border:       hex("#ffffff38"),
            search_text:         Color::WHITE,
            search_placeholder:  hex("#9999b2"),
            search_selection:    hex("#6680e673"),
            app_label:           Color::WHITE,
            app_selected:        hex("#ffffff26"),
            dot_active:          Color::WHITE,
            dot_inactive:        hex("#ffffff59"),
            ai_idle:             hex("#999999"),
            ai_error:            hex("#ff6666"),
            ai_panel:            hex("#0d0d17"),
            ai_code_background:  hex("#2e2e42"),
            ai_code_text:        hex("#f2c780"),
            ai_link:             hex("#73bfff"),
            terminal_background: hex("#0a0a0f"),
            terminal_prompt:     hex("#66d966"),
            terminal_output:     hex("#e0e0e0"),
            button_background:   hex("#ffffff1a"),
            copy_feedback:       hex("#80e699"),
        }
    }
}

impl Theme {
    pub fn apply_key(&mut self, key: &str, value: &str) {
        let Some(color) = parse_color(value) else { return };
        match key {
            "background"          => self.background          = color,
            "search_background"   => self.search_background   = color,
            "search_border"       => self.search_border       = color,
            "search_text"         => self.search_text         = color,
            "search_placeholder"  => self.search_placeholder  = color,
            "search_selection"    => self.search_selection     = color,
            "app_label"           => self.app_label           = color,
            "app_selected"        => self.app_selected        = color,
            "dot_active"          => self.dot_active          = color,
            "dot_inactive"        => self.dot_inactive        = color,
            "ai_idle"             => self.ai_idle             = color,
            "ai_error"            => self.ai_error            = color,
            "ai_panel"            => self.ai_panel            = color,
            "ai_code_background"  => self.ai_code_background  = color,
            "ai_code_text"        => self.ai_code_text        = color,
            "ai_link"             => self.ai_link             = color,
            "terminal_background" => self.terminal_background = color,
            "terminal_prompt"     => self.terminal_prompt     = color,
            "terminal_output"     => self.terminal_output     = color,
            "button_background"   => self.button_background   = color,
            "copy_feedback"       => self.copy_feedback       = color,
            _ => {}
        }
    }
}

impl Theme {
    /// Load a theme from a `.conf` file. Returns `None` if the file can't be read.
    /// Unknown keys are silently ignored; missing keys keep their default values.
    pub fn from_file(path: &std::path::Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        let mut theme = Theme::default();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let Some((key, val)) = line.split_once('=') else { continue };
            theme.apply_key(key.trim(), val.trim().trim_matches('"'));
        }
        Some(theme)
    }
}

/// Parse `#RRGGBB` or `#RRGGBBAA` hex string into an iced Color.
/// Each channel is divided by 255 to produce a 0.0–1.0 linear value.
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#');
    if s.len() < 6 { return None; }
    let r = u8::from_str_radix(&s[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&s[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&s[4..6], 16).ok()? as f32 / 255.0;
    let a = if s.len() >= 8 {
        u8::from_str_radix(&s[6..8], 16).ok()? as f32 / 255.0
    } else {
        1.0
    };
    Some(Color { r, g, b, a })
}

/// Hex colour literal used in Default — panics only if a constant is wrong.
fn hex(s: &str) -> Color {
    parse_color(s).expect("invalid hex in Theme::default()")
}
