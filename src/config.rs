const DEFAULT_CONF: &str = include_str!("../assets/trebuchet.conf");

pub struct Config {
    pub columns: usize,
    pub rows: usize,
    pub icon_size: u32,
}

impl Config {
    pub fn load() -> Self {
        // Layer 1: hardcoded Rust defaults.
        // Layer 2: embedded assets/trebuchet.conf (overrides layer 1).
        let mut cfg = Self::parse(Self::default(), DEFAULT_CONF);

        // Layer 3: user file — only valid, present keys override layer 2.
        if let Some(content) = std::env::var("HOME")
            .ok()
            .map(|h| std::path::PathBuf::from(h).join(".config/trebuchet/trebuchet.conf"))
            .and_then(|p| std::fs::read_to_string(p).ok())
        {
            cfg = Self::parse(cfg, &content);
        }

        cfg
    }

    /// Apply key=value pairs from `content` onto `base`, returning the result.
    /// Unknown keys and unparseable values are silently ignored, preserving
    /// whatever `base` already has for those fields.
    fn parse(mut base: Self, content: &str) -> Self {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                let val = val.trim();
                match key.trim() {
                    "columns"   => { if let Ok(v) = val.parse() { base.columns   = v; } }
                    "rows"      => { if let Ok(v) = val.parse() { base.rows      = v; } }
                    "icon_size" => { if let Ok(v) = val.parse() { base.icon_size = v; } }
                    _ => {}
                }
            }
        }
        base
    }
}

impl Default for Config {
    fn default() -> Self {
        Self { columns: 7, rows: 5, icon_size: 96 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Config { Config::default() }

    #[test]
    fn embedded_conf_parses_cleanly() {
        let cfg = Config::parse(defaults(), DEFAULT_CONF);
        assert_eq!(cfg.columns, 7);
        assert_eq!(cfg.rows, 5);
        assert_eq!(cfg.icon_size, 96);
    }

    #[test]
    fn valid_keys_override_base() {
        let cfg = Config::parse(defaults(), "columns = 5\nrows = 3\nicon_size = 64");
        assert_eq!(cfg.columns, 5);
        assert_eq!(cfg.rows, 3);
        assert_eq!(cfg.icon_size, 64);
    }

    #[test]
    fn invalid_value_keeps_base() {
        let cfg = Config::parse(defaults(), "columns = not_a_number");
        assert_eq!(cfg.columns, 7);
    }

    #[test]
    fn missing_key_keeps_base() {
        let base = Config { columns: 4, rows: 3, icon_size: 48 };
        let cfg = Config::parse(base, "icon_size = 64");
        assert_eq!(cfg.columns, 4);
        assert_eq!(cfg.rows, 3);
        assert_eq!(cfg.icon_size, 64);
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let cfg = Config::parse(defaults(), "# comment\n\ncolumns = 4\n# another");
        assert_eq!(cfg.columns, 4);
        assert_eq!(cfg.rows, 5);
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let cfg = Config::parse(defaults(), "unknown = 99\ncolumns = 4");
        assert_eq!(cfg.columns, 4);
        assert_eq!(cfg.rows, 5);
    }

    #[test]
    fn whitespace_around_equals_is_trimmed() {
        let cfg = Config::parse(defaults(), "columns   =   3");
        assert_eq!(cfg.columns, 3);
    }

    #[test]
    fn partial_user_conf_falls_back_to_embedded_defaults() {
        // Simulate: embedded conf sets all three, user conf only sets one.
        let embedded = Config::parse(defaults(), DEFAULT_CONF);
        let cfg = Config::parse(embedded, "columns = 5");
        assert_eq!(cfg.columns, 5);
        assert_eq!(cfg.rows, 5);      // from embedded default
        assert_eq!(cfg.icon_size, 96); // from embedded default
    }
}
