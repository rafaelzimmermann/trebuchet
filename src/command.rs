#[derive(Debug, Clone, PartialEq)]
pub enum SlashCommand {
    Ai,
    App,
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
                "ai" => SlashCommand::Ai,
                "app" => SlashCommand::App,
                s => SlashCommand::Unknown(s.to_string()),
            },
            args,
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentEvent {
    Handled,
    Exit,
    CommandInvoked(SlashCommand, String),
}
