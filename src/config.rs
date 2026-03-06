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
pub struct AiModelConfig {
    pub provider: AiProvider,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    /// Display label shown in the model picker — always `"provider:model"` or just `"provider"`.
    pub label: String,
}

impl AiModelConfig {
    fn build(
        provider: AiProvider,
        api_key: Option<String>,
        model: Option<String>,
        base_url: Option<String>,
    ) -> Self {
        let p = match &provider {
            AiProvider::OpenAi    => "openai",
            AiProvider::Anthropic => "anthropic",
            AiProvider::Gemini    => "gemini",
            AiProvider::Ollama    => "ollama",
        };
        let label = match &model {
            Some(m) => format!("{p}:{m}"),
            None    => p.to_string(),
        };
        Self { provider, api_key, model, base_url, label }
    }
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
    /// Ordered list of AI model configs. First entry is the default.
    pub ai_models: Vec<AiModelConfig>,
    pub commands: Vec<CustomCommand>,
}

impl Config {
    pub fn load() -> Self {
        let mut cfg = Self::parse(Self::default(), DEFAULT_CONF);
        if let Some(content) = std::env::var("HOME")
            .ok()
            .map(|h| std::path::PathBuf::from(h).join(".config/trebuchet/trebuchet.conf"))
            .and_then(|p| std::fs::read_to_string(p).ok())
        {
            cfg = Self::parse(cfg, &content);
        }
        cfg
    }

    fn parse(mut base: Self, content: &str) -> Self {
        // ── [[command]] block state ───────────────────────────────────────────
        let mut cur_prefix = String::new();
        let mut cur_command = String::new();
        let mut cur_display = false;
        let mut in_cmd_block = false;

        // ── [[ai_model]] block state ──────────────────────────────────────────
        let mut cur_ai_provider = AiProvider::default();
        let mut cur_ai_api_key: Option<String> = None;
        let mut cur_ai_model: Option<String> = None;
        let mut cur_ai_base_url: Option<String> = None;
        let mut in_ai_block = false;

        // ── Legacy flat AI keys (backward compat) ─────────────────────────────
        let mut legacy_provider: Option<AiProvider> = None;
        let mut legacy_api_key: Option<String> = None;
        let mut legacy_model: Option<String> = None;
        let mut legacy_base_url: Option<String> = None;
        let mut has_legacy_ai = false;

        let finalize_cmd = |base: &mut Config,
                            prefix: &mut String,
                            command: &mut String,
                            display: &mut bool| {
            if !prefix.is_empty() && !command.is_empty() {
                base.commands.push(CustomCommand {
                    prefix: std::mem::take(prefix),
                    command: std::mem::take(command),
                    display_result: std::mem::take(display),
                });
            }
        };

        // Expand a comma-separated model string into one AiModelConfig per model.
        // If no model is specified, one entry is created using the provider default.
        let finalize_ai = |base: &mut Config,
                           provider: &mut AiProvider,
                           api_key: &mut Option<String>,
                           model: &mut Option<String>,
                           base_url: &mut Option<String>| {
            let prov = std::mem::replace(provider, AiProvider::default());
            let key  = std::mem::take(api_key);
            let url  = std::mem::take(base_url);
            let models: Vec<Option<String>> = match std::mem::take(model) {
                None => vec![None],
                Some(s) => {
                    let v: Vec<_> = s.split(',').map(|m| m.trim()).filter(|m| !m.is_empty())
                        .map(|m| Some(m.to_string())).collect();
                    if v.is_empty() { vec![None] } else { v }
                }
            };
            for m in models {
                base.ai_models.push(AiModelConfig::build(prov.clone(), key.clone(), m, url.clone()));
            }
        };

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line == "[[command]]" {
                if in_ai_block {
                    finalize_ai(&mut base, &mut cur_ai_provider, &mut cur_ai_api_key,
                                &mut cur_ai_model, &mut cur_ai_base_url);
                    in_ai_block = false;
                }
                if in_cmd_block {
                    finalize_cmd(&mut base, &mut cur_prefix, &mut cur_command, &mut cur_display);
                }
                in_cmd_block = true;
                continue;
            }
            if line == "[[ai_model]]" {
                if in_cmd_block {
                    finalize_cmd(&mut base, &mut cur_prefix, &mut cur_command, &mut cur_display);
                    in_cmd_block = false;
                }
                if in_ai_block {
                    finalize_ai(&mut base, &mut cur_ai_provider, &mut cur_ai_api_key,
                                &mut cur_ai_model, &mut cur_ai_base_url);
                }
                in_ai_block = true;
                continue;
            }

            let Some((key, val)) = line.split_once('=') else { continue };
            let key = key.trim();
            let val = val.trim().trim_matches('"');

            if in_cmd_block {
                match key {
                    "prefix"         => { cur_prefix  = val.to_string(); continue; }
                    "command"        => { cur_command  = val.to_string(); continue; }
                    "display_result" => { cur_display  = val == "true";  continue; }
                    _ => {}
                }
            }
            if in_ai_block {
                match key {
                    "provider" => {
                        cur_ai_provider = match val {
                            "anthropic" => AiProvider::Anthropic,
                            "gemini"    => AiProvider::Gemini,
                            "ollama"    => AiProvider::Ollama,
                            _           => AiProvider::OpenAi,
                        };
                        continue;
                    }
                    "api_key"  => { cur_ai_api_key  = Some(val.to_string()); continue; }
                    "model"    => { cur_ai_model     = Some(val.to_string()); continue; }
                    "base_url" => { if !val.is_empty() { cur_ai_base_url = Some(val.to_string()); } continue; }
                    _ => {}
                }
            }

            match key {
                "columns"   => { if let Ok(v) = val.parse() { base.columns   = v; } }
                "rows"      => { if let Ok(v) = val.parse() { base.rows      = v; } }
                "icon_size" => { if let Ok(v) = val.parse() { base.icon_size = v; } }
                // Legacy single-config AI keys — kept for backward compatibility.
                "ai_provider" => {
                    legacy_provider = Some(match val {
                        "anthropic" => AiProvider::Anthropic,
                        "gemini"    => AiProvider::Gemini,
                        "ollama"    => AiProvider::Ollama,
                        _           => AiProvider::OpenAi,
                    });
                    has_legacy_ai = true;
                }
                "ai_api_key"  => { legacy_api_key  = Some(val.to_string()); has_legacy_ai = true; }
                "ai_model"    => { legacy_model     = Some(val.to_string()); has_legacy_ai = true; }
                "ai_base_url" => { if !val.is_empty() { legacy_base_url = Some(val.to_string()); has_legacy_ai = true; } }
                _ => {}
            }
        }

        if in_cmd_block {
            finalize_cmd(&mut base, &mut cur_prefix, &mut cur_command, &mut cur_display);
        }
        if in_ai_block {
            finalize_ai(&mut base, &mut cur_ai_provider, &mut cur_ai_api_key,
                        &mut cur_ai_model, &mut cur_ai_base_url);
        }

        // Legacy flat keys — expand comma-separated models, insert at front as defaults.
        if has_legacy_ai {
            let prov = legacy_provider.unwrap_or_default();
            let models: Vec<Option<String>> = match legacy_model {
                None => vec![None],
                Some(s) => {
                    let v: Vec<_> = s.split(',').map(|m| m.trim()).filter(|m| !m.is_empty())
                        .map(|m| Some(m.to_string())).collect();
                    if v.is_empty() { vec![None] } else { v }
                }
            };
            let insert_at = base.ai_models.len();
            for m in models {
                base.ai_models.push(AiModelConfig::build(
                    prov.clone(), legacy_api_key.clone(), m, legacy_base_url.clone(),
                ));
            }
            // Rotate the newly added entries to the front.
            let total = base.ai_models.len();
            base.ai_models.rotate_right(total - insert_at);
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
            ai_models: Vec::new(),
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
        let embedded = Config::parse(defaults(), DEFAULT_CONF);
        let cfg = Config::parse(embedded, "columns = 5");
        assert_eq!(cfg.columns, 5);
        assert_eq!(cfg.rows, 5);
        assert_eq!(cfg.icon_size, 96);
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

    // ── [[ai_model]] blocks ───────────────────────────────────────────────────

    #[test]
    fn ai_model_block_parsed() {
        let content = "[[ai_model]]\nprovider = anthropic\napi_key = sk-ant\nmodel = claude-sonnet-4-6\n";
        let cfg = Config::parse(defaults(), content);
        assert_eq!(cfg.ai_models.len(), 1);
        assert_eq!(cfg.ai_models[0].provider, AiProvider::Anthropic);
        assert_eq!(cfg.ai_models[0].api_key.as_deref(), Some("sk-ant"));
        assert_eq!(cfg.ai_models[0].model.as_deref(), Some("claude-sonnet-4-6"));
        assert_eq!(cfg.ai_models[0].label, "anthropic:claude-sonnet-4-6");
    }

    #[test]
    fn ai_model_comma_list_expands() {
        let content = "[[ai_model]]\nprovider = anthropic\napi_key = sk-ant\nmodel = claude-sonnet-4-6, claude-opus-4-6\n";
        let cfg = Config::parse(defaults(), content);
        assert_eq!(cfg.ai_models.len(), 2);
        assert_eq!(cfg.ai_models[0].model.as_deref(), Some("claude-sonnet-4-6"));
        assert_eq!(cfg.ai_models[0].label, "anthropic:claude-sonnet-4-6");
        assert_eq!(cfg.ai_models[1].model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(cfg.ai_models[1].label, "anthropic:claude-opus-4-6");
        // Both share the same api_key
        assert_eq!(cfg.ai_models[0].api_key, cfg.ai_models[1].api_key);
    }

    #[test]
    fn multiple_ai_model_blocks() {
        let content = "[[ai_model]]\nprovider = openai\nmodel = gpt-4o\n\n[[ai_model]]\nprovider = anthropic\nmodel = claude-sonnet-4-6\n";
        let cfg = Config::parse(defaults(), content);
        assert_eq!(cfg.ai_models.len(), 2);
        assert_eq!(cfg.ai_models[0].provider, AiProvider::OpenAi);
        assert_eq!(cfg.ai_models[1].provider, AiProvider::Anthropic);
    }

    #[test]
    fn legacy_flat_keys_create_first_entry() {
        let content = "ai_provider = anthropic\nai_api_key = sk-ant\nai_model = claude-sonnet-4-6\n";
        let cfg = Config::parse(defaults(), content);
        assert_eq!(cfg.ai_models.len(), 1);
        assert_eq!(cfg.ai_models[0].provider, AiProvider::Anthropic);
        assert_eq!(cfg.ai_models[0].label, "anthropic:claude-sonnet-4-6");
    }

    #[test]
    fn legacy_flat_keys_comma_list_expands() {
        let content = "ai_provider = openai\nai_api_key = sk\nai_model = gpt-4o, gpt-4-turbo\n";
        let cfg = Config::parse(defaults(), content);
        assert_eq!(cfg.ai_models.len(), 2);
        assert_eq!(cfg.ai_models[0].label, "openai:gpt-4o");
        assert_eq!(cfg.ai_models[1].label, "openai:gpt-4-turbo");
    }

    #[test]
    fn legacy_keys_inserted_before_ai_model_blocks() {
        let content = "ai_provider = openai\nai_model = gpt-4o\n\n[[ai_model]]\nprovider = anthropic\nmodel = claude-sonnet-4-6\n";
        let cfg = Config::parse(defaults(), content);
        assert_eq!(cfg.ai_models.len(), 2);
        assert_eq!(cfg.ai_models[0].provider, AiProvider::OpenAi);
        assert_eq!(cfg.ai_models[1].provider, AiProvider::Anthropic);
    }
}
