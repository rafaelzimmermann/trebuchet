#[derive(Debug, Clone, PartialEq)]
pub enum SlashCommand {
    Ai,
    App,
    Config,
    Cmd,
    Unknown(String),
}

impl SlashCommand {
    /// Detects `^/[a-zA-Z]+\s` pattern. Returns (command, args_after_space).
    pub fn detect(query: &str) -> Option<(Self, String)> {
        if !query.starts_with('/') {
            return None;
        }
        let rest = &query[1..];
        let end = rest.find(char::is_whitespace)?;
        let args = rest[end + 1..].to_string();
        Some((
            match &rest[..end] {
                "ai"     => SlashCommand::Ai,
                "app"    => SlashCommand::App,
                "config" => SlashCommand::Config,
                "cmd"    => SlashCommand::Cmd,
                s        => SlashCommand::Unknown(s.to_string()),
            },
            args,
        ))
    }

    /// Returns a nav ComponentEvent if `query` triggers a navigation command.
    /// Works for space-triggered dispatch (query already ends with space) and
    /// Enter-triggered dispatch (call with `format!("{} ", query.trim())`).
    pub fn as_nav_event(query: &str) -> Option<ComponentEvent> {
        match Self::detect(query) {
            Some((Self::Ai, args))     => Some(ComponentEvent::CommandInvoked(Self::Ai, args)),
            Some((Self::App, args))    => Some(ComponentEvent::CommandInvoked(Self::App, args)),
            Some((Self::Config, args)) => Some(ComponentEvent::CommandInvoked(Self::Config, args)),
            Some((Self::Cmd, args))    => Some(ComponentEvent::CommandInvoked(Self::Cmd, args)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentEvent {
    Handled,
    Exit,
    CommandInvoked(SlashCommand, String), // command + remaining args
    ThemeChanged(String, crate::theme::Theme), // (name, theme) — settings emits this
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── SlashCommand::detect ──────────────────────────────────────────────────

    #[test]
    fn detect_ai_with_trailing_space() {
        assert_eq!(SlashCommand::detect("/ai "), Some((SlashCommand::Ai, String::new())));
    }

    #[test]
    fn detect_cmd_variant() {
        assert_eq!(SlashCommand::detect("/cmd "), Some((SlashCommand::Cmd, String::new())));
    }

    #[test]
    fn detect_config_variant() {
        assert_eq!(SlashCommand::detect("/config "), Some((SlashCommand::Config, String::new())));
    }

    #[test]
    fn detect_app_with_args() {
        assert_eq!(
            SlashCommand::detect("/app hello world"),
            Some((SlashCommand::App, "hello world".to_string())),
        );
    }

    #[test]
    fn detect_unknown_variant() {
        assert_eq!(
            SlashCommand::detect("/foo "),
            Some((SlashCommand::Unknown("foo".to_string()), String::new())),
        );
    }

    #[test]
    fn detect_no_leading_slash_returns_none() {
        assert_eq!(SlashCommand::detect("ai "), None);
    }

    #[test]
    fn detect_no_trailing_space_returns_none() {
        // No whitespace after the command word — should not fire.
        assert_eq!(SlashCommand::detect("/ai"), None);
    }

    #[test]
    fn detect_empty_returns_none() {
        assert_eq!(SlashCommand::detect(""), None);
    }

    // ── SlashCommand::as_nav_event ────────────────────────────────────────────

    #[test]
    fn as_nav_event_known_commands_return_some() {
        for (input, expected) in [
            ("/ai ", SlashCommand::Ai),
            ("/app ", SlashCommand::App),
            ("/config ", SlashCommand::Config),
            ("/cmd ", SlashCommand::Cmd),
        ] {
            let evt = SlashCommand::as_nav_event(input);
            assert!(
                matches!(&evt, Some(ComponentEvent::CommandInvoked(cmd, _)) if cmd == &expected),
                "expected Some(CommandInvoked({expected:?})) for {input:?}, got {evt:?}",
            );
        }
    }

    #[test]
    fn as_nav_event_unknown_returns_none() {
        assert_eq!(SlashCommand::as_nav_event("/foo "), None);
    }

    #[test]
    fn as_nav_event_no_slash_returns_none() {
        assert_eq!(SlashCommand::as_nav_event("ai "), None);
    }

    #[test]
    fn as_nav_event_bare_word_returns_none() {
        // No trailing space — detect() returns None, so as_nav_event returns None.
        assert_eq!(SlashCommand::as_nav_event("/ai"), None);
    }
}
