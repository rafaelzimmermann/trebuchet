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
