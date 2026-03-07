use iced::{
    event::Status,
    keyboard::{self, key::Named, Key},
    widget::markdown,
    time,
    Element, Event, Length, Subscription, Task,
};
use std::time::Duration;

use super::ai_client::{self, AiRequest};
use super::command::{ComponentEvent, SlashCommand};
use super::component::Component;
use crate::config::Config;
use crate::launcher::AppEntry;
use crate::ui::{ai_panel, search_bar, SearchIcon, ShakeState, PANEL_PADDING};

#[derive(Debug, Clone, PartialEq)]
pub enum AiStatus {
    Idle,
    Loading { tick: u8 },
    Done(String),
    Error(String),
}

pub struct AIAgent {
    /// Prompt text shown in the search bar (no "/ai " prefix).
    query: String,
    /// Last submitted prompt, retained for Retry.
    prompt: String,
    pub status: AiStatus,
    copy_feedback: bool,
    response_items: Vec<markdown::Item>,
    shake: ShakeState,
    /// Index into `config.ai_models` for the currently selected model.
    selected_model: usize,
}

#[derive(Debug, Clone)]
pub enum Msg {
    QueryChanged(String),
    Response(Result<String, String>),
    Retry,
    CopyResponse,
    Copied,
    LoadingTick,
    ShakeTick,
    LinkClicked(String),
    ModelSelected(String),
}

impl AIAgent {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            prompt: String::new(),
            status: AiStatus::Idle,
            copy_feedback: false,
            response_items: Vec::new(),
            shake: ShakeState::default(),
            selected_model: 0,
        }
    }

    /// Reset for entering AI mode. `initial_query` is the args portion (after "/ai ").
    pub fn reset(&mut self, initial_query: String) {
        self.query = initial_query.trim().to_string();
        self.prompt = String::new();
        self.status = AiStatus::Idle;
        self.response_items.clear();
        self.copy_feedback = false;
        self.shake = ShakeState::default();
    }

    fn start_query(&mut self, prompt: String, config: &Config) -> Task<Msg> {
        self.prompt = prompt.clone();
        self.status = AiStatus::Loading { tick: 0 };
        self.response_items.clear();
        let m = config.ai_models.get(self.selected_model);
        let req = AiRequest {
            prompt,
            provider: m.map(|m| m.provider.clone()).unwrap_or_default(),
            api_key:  m.and_then(|m| m.api_key.clone()),
            model:    m.and_then(|m| m.model.clone()),
            base_url: m.and_then(|m| m.base_url.clone()),
        };
        Task::perform(ai_client::query(req), Msg::Response)
    }
}

impl AIAgent {
    fn do_char(&mut self, c: String) -> (Task<Msg>, ComponentEvent) {
        self.query.push_str(&c);
        if let Some(evt) = SlashCommand::as_nav_event(&self.query) {
            self.query.clear();
            return (Task::none(), evt);
        }
        (Task::none(), ComponentEvent::Handled)
    }

    fn do_backspace(&mut self) -> (Task<Msg>, ComponentEvent) {
        self.query.pop();
        (Task::none(), ComponentEvent::Handled)
    }

    fn do_submit(&mut self, config: &Config) -> (Task<Msg>, ComponentEvent) {
        let prompt = self.query.trim().to_string();
        if prompt.is_empty() {
            self.shake = ShakeState::trigger();
            return (Task::none(), ComponentEvent::Handled);
        }
        let task = self.start_query(prompt, config);
        (task, ComponentEvent::Handled)
    }
}

impl Component for AIAgent {
    type Msg = Msg;

    fn handle_event(
        &mut self,
        event: &Event,
        status: Status,
        _apps: &[AppEntry],
        config: &Config,
    ) -> (Task<Msg>, ComponentEvent) {
        let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, text, .. }) = event
        else {
            return (Task::none(), ComponentEvent::Handled);
        };
        match key {
            Key::Named(Named::Enter) => {
                if let Some(evt) = SlashCommand::as_nav_event(&format!("{} ", self.query.trim())) {
                    self.query.clear();
                    return (Task::none(), evt);
                }
                self.do_submit(config)
            }
            Key::Named(Named::Escape) => {
                self.query.clear();
                (Task::none(), ComponentEvent::CommandInvoked(SlashCommand::App, String::new()))
            }
            Key::Named(Named::Backspace) if status == Status::Ignored => self.do_backspace(),
            Key::Named(Named::Space) if status == Status::Ignored => {
                self.do_char(" ".to_string())
            }
            Key::Character(_)
                if status == Status::Ignored
                    && !modifiers.control()
                    && !modifiers.alt()
                    && !modifiers.logo() =>
            {
                if let Some(t) = text.as_ref() {
                    self.do_char(t.to_string())
                } else {
                    (Task::none(), ComponentEvent::Handled)
                }
            }
            _ => (Task::none(), ComponentEvent::Handled),
        }
    }

    fn update(&mut self, msg: Msg, _apps: &[AppEntry], config: &Config) -> (Task<Msg>, ComponentEvent) {
        match msg {
            Msg::QueryChanged(s) => {
                self.query = s;
            }
            Msg::Response(result) => {
                self.status = match result {
                    Ok(text) => {
                        self.response_items = markdown::parse(&text).collect();
                        AiStatus::Done(text)
                    }
                    Err(err) => {
                        self.response_items.clear();
                        AiStatus::Error(err)
                    }
                };
            }
            Msg::Retry => {
                let prompt = self.prompt.clone();
                if prompt.is_empty() {
                    return (Task::none(), ComponentEvent::Handled);
                }
                return (self.start_query(prompt, config), ComponentEvent::Handled);
            }
            Msg::CopyResponse => {
                if let AiStatus::Done(text) = &self.status {
                    let _ = std::process::Command::new("wl-copy").arg(text).spawn();
                    self.copy_feedback = true;
                    return (
                        Task::perform(
                            async { tokio::time::sleep(Duration::from_secs(2)).await },
                            |_| Msg::Copied,
                        ),
                        ComponentEvent::Handled,
                    );
                }
            }
            Msg::Copied => {
                self.copy_feedback = false;
            }
            Msg::LoadingTick => {
                if let AiStatus::Loading { tick } = &mut self.status {
                    *tick = (*tick + 1) % 3;
                }
            }
            Msg::ShakeTick => {
                self.shake.advance();
            }
            Msg::LinkClicked(url) => {
                let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
            }
            Msg::ModelSelected(label) => {
                if let Some(idx) = config.ai_models.iter().position(|m| m.label == label) {
                    self.selected_model = idx;
                }
            }
        }
        (Task::none(), ComponentEvent::Handled)
    }

    fn view<'a>(&'a self, _apps: &'a [AppEntry], config: &'a Config) -> Element<'a, Msg> {
        let model_labels: Vec<String> = config.ai_models.iter().map(|m| m.label.clone()).collect();
        let selected_label = model_labels.get(self.selected_model).cloned();

        iced::widget::container(
            iced::widget::column![
                search_bar(&self.query, &self.shake, SearchIcon::Robot, &config.theme, Msg::QueryChanged),
                ai_panel(
                    &self.status,
                    &self.prompt,
                    self.copy_feedback,
                    &self.response_items,
                    model_labels,
                    selected_label,
                    &config.theme,
                    Msg::CopyResponse,
                    Msg::Retry,
                    Msg::LinkClicked,
                    Msg::ModelSelected,
                ),
            ]
            .spacing(16)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .padding(PANEL_PADDING)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn subscription(&self) -> Subscription<Msg> {
        let loading = if matches!(self.status, AiStatus::Loading { .. }) {
            time::every(Duration::from_millis(400)).map(|_| Msg::LoadingTick)
        } else {
            Subscription::none()
        };
        let shake = if self.shake.active {
            time::every(Duration::from_millis(67)).map(|_| Msg::ShakeTick)
        } else {
            Subscription::none()
        };
        Subscription::batch([loading, shake])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::command::ComponentEvent;
    use crate::config::Config;
    use crate::launcher::AppEntry;

    // Regression: Copy/Retry when there is no response must not exit the launcher.
    // Buttons rendered without on_press leak Status::Ignored which app.rs maps to
    // Message::Close. These handlers must return Handled in all non-Done states.

    #[test]
    fn copy_when_idle_returns_handled() {
        let mut a = AIAgent::new(); // status starts Idle
        let apps: Vec<AppEntry> = vec![];
        let (_, evt) = a.update(Msg::CopyResponse, &apps, &Config::default());
        assert_eq!(evt, ComponentEvent::Handled);
    }

    #[test]
    fn copy_when_loading_returns_handled() {
        let mut a = AIAgent::new();
        a.status = AiStatus::Loading { tick: 0 };
        let apps: Vec<AppEntry> = vec![];
        let (_, evt) = a.update(Msg::CopyResponse, &apps, &Config::default());
        assert_eq!(evt, ComponentEvent::Handled);
    }

    #[test]
    fn retry_when_idle_returns_handled() {
        let mut a = AIAgent::new(); // prompt is empty
        let apps: Vec<AppEntry> = vec![];
        let (_, evt) = a.update(Msg::Retry, &apps, &Config::default());
        assert_eq!(evt, ComponentEvent::Handled);
    }
}
