const DEFAULT_CONF: &str = include_str!("../assets/trebuchet.conf");

#[derive(Debug, Clone, PartialEq, Default)]
pub enum AiProvider {
    #[default]
    OpenAi,
    Anthropic,
    Gemini,
    Ollama,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CustomCommand {
    pub prefix: String,
    pub command: String,
    pub display_result: bool,
}

pub struct Config {
    pub columns: usize,
    pub rows: usize,
    pub icon_size: u32,
    pub ai_provider: AiProvider,
    pub ai_api_key: Option<String>,
    pub ai_model: Option<String>,
    pub ai_base_url: Option<String>,
    pub commands: Vec<CustomCommand>,
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
    /// `[[command]]` blocks are accumulated into `base.commands`.
    /// Unknown keys and unparseable values are silently ignored.
    fn parse(mut base: Self, content: &str) -> Self {
        // State for the [[command]] block currently being assembled.
        let mut cur_prefix = String::new();
        let mut cur_command = String::new();
        let mut cur_display = false;
        let mut in_cmd_block = false;

        let finalize = |base: &mut Config, prefix: &mut String, command: &mut String, display: &mut bool| {
            if !prefix.is_empty() && !command.is_empty() {
                base.commands.push(CustomCommand {
                    prefix: std::mem::take(prefix),
                    command: std::mem::take(command),
                    display_result: std::mem::take(display),
                });
            }
        };

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line == "[[command]]" {
                if in_cmd_block {
                    finalize(&mut base, &mut cur_prefix, &mut cur_command, &mut cur_display);
                }
                in_cmd_block = true;
                continue;
            }
            let Some((key, val)) = line.split_once('=') else { continue };
            let key = key.trim();
            let val = val.trim().trim_matches('"');

            let handled_as_cmd_key = in_cmd_block && match key {
                "prefix"         => { cur_prefix  = val.to_string(); true }
                "command"        => { cur_command  = val.to_string(); true }
                "display_result" => { cur_display  = val == "true";  true }
                _ => false,
            };
            if !handled_as_cmd_key {
                match key {
                    "columns"   => { if let Ok(v) = val.parse() { base.columns   = v; } }
                    "rows"      => { if let Ok(v) = val.parse() { base.rows      = v; } }
                    "icon_size" => { if let Ok(v) = val.parse() { base.icon_size = v; } }
                    "ai_provider" => base.ai_provider = match val {
                        "anthropic" => AiProvider::Anthropic,
                        "gemini"    => AiProvider::Gemini,
                        "ollama"    => AiProvider::Ollama,
                        _           => AiProvider::OpenAi,
                    },
                    "ai_api_key"  => base.ai_api_key  = Some(val.to_string()),
                    "ai_model"    => base.ai_model    = Some(val.to_string()),
                    "ai_base_url" => if !val.is_empty() { base.ai_base_url = Some(val.to_string()); },
                    _ => {}
                }
            }
        }

        if in_cmd_block {
            finalize(&mut base, &mut cur_prefix, &mut cur_command, &mut cur_display);
        }

        base
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            columns: 7,
            rows: 5,
            icon_size: 96,
            ai_provider: AiProvider::default(),
            ai_api_key: None,
            ai_model: None,
            ai_base_url: None,
            commands: Vec::new(),
        }
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
        let base = Config { columns: 4, rows: 3, icon_size: 48, ..Config::default() };
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

    // ── [[command]] blocks ────────────────────────────────────────────────────

    #[test]
    fn command_block_parsed() {
        let cfg = Config::parse(defaults(), "[[command]]\nprefix = /hi\ncommand = echo hi\n");
        assert_eq!(cfg.commands.len(), 1);
        assert_eq!(cfg.commands[0].prefix, "/hi");
        assert_eq!(cfg.commands[0].command, "echo hi");
        assert!(!cfg.commands[0].display_result);
    }

    #[test]
    fn command_display_result_true() {
        let cfg = Config::parse(defaults(), "[[command]]\nprefix = /up\ncommand = uptime\ndisplay_result = true\n");
        assert!(cfg.commands[0].display_result);
    }

    #[test]
    fn multiple_command_blocks() {
        let content = "[[command]]\nprefix = /a\ncommand = echo a\n\n[[command]]\nprefix = /b\ncommand = echo b\ndisplay_result = true\n";
        let cfg = Config::parse(defaults(), content);
        assert_eq!(cfg.commands.len(), 2);
        assert_eq!(cfg.commands[0].prefix, "/a");
        assert!(!cfg.commands[0].display_result);
        assert_eq!(cfg.commands[1].prefix, "/b");
        assert!(cfg.commands[1].display_result);
    }

    #[test]
    fn command_block_without_prefix_skipped() {
        let cfg = Config::parse(defaults(), "[[command]]\ncommand = echo hi\n");
        assert_eq!(cfg.commands.len(), 0);
    }

    #[test]
    fn command_block_without_command_skipped() {
        let cfg = Config::parse(defaults(), "[[command]]\nprefix = /hi\n");
        assert_eq!(cfg.commands.len(), 0);
    }

    #[test]
    fn command_blocks_accumulate_across_parse_layers() {
        let base = Config::parse(defaults(), "[[command]]\nprefix = /a\ncommand = echo a\n");
        let cfg  = Config::parse(base, "[[command]]\nprefix = /b\ncommand = echo b\n");
        assert_eq!(cfg.commands.len(), 2);
    }

    #[test]
    fn scalar_keys_before_and_after_command_block() {
        let content = "columns = 3\n\n[[command]]\nprefix = /hi\ncommand = echo hi\n\nrows = 2\n";
        let cfg = Config::parse(defaults(), content);
        assert_eq!(cfg.columns, 3);
        assert_eq!(cfg.rows, 2);
        assert_eq!(cfg.commands.len(), 1);
    }
}
