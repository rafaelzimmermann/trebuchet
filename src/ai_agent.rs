use iced::{widget::markdown, time, Element, Subscription, Task};
use std::time::Duration;

use crate::ai_client::{self, AiRequest};
use crate::command::{ComponentEvent, SlashCommand};
use crate::component::{Component, NavDirection};
use crate::config::Config;
use crate::launcher::AppEntry;
use crate::ui::{ai_panel, search_bar, SearchIcon, ShakeState};

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
        let req = AiRequest {
            prompt,
            provider: config.ai_provider.clone(),
            api_key: config.ai_api_key.clone(),
            model: config.ai_model.clone(),
            base_url: config.ai_base_url.clone(),
        };
        Task::perform(ai_client::query(req), Msg::Response)
    }
}

impl Component for AIAgent {
    type Msg = Msg;

    fn handle_char(
        &mut self,
        c: String,
        _apps: &[AppEntry],
        _config: &Config,
    ) -> (Task<Msg>, ComponentEvent) {
        self.query.push_str(&c);

        if let Some((SlashCommand::App, args)) = SlashCommand::detect(&self.query) {
            return (Task::none(), ComponentEvent::CommandInvoked(SlashCommand::App, args));
        }

        (Task::none(), ComponentEvent::Handled)
    }

    fn handle_backspace(
        &mut self,
        _apps: &[AppEntry],
        _config: &Config,
    ) -> (Task<Msg>, ComponentEvent) {
        self.query.pop();
        (Task::none(), ComponentEvent::Handled)
    }

    fn handle_submit(
        &mut self,
        _apps: &[AppEntry],
        config: &Config,
    ) -> (Task<Msg>, ComponentEvent) {
        let prompt = self.query.trim().to_string();
        if prompt.is_empty() {
            self.shake = ShakeState::trigger();
            return (Task::none(), ComponentEvent::Handled);
        }
        let task = self.start_query(prompt, config);
        (task, ComponentEvent::Handled)
    }

    fn handle_escape(&mut self) -> ComponentEvent {
        self.query.clear();
        ComponentEvent::CommandInvoked(SlashCommand::App, String::new())
    }

    fn handle_nav(&mut self, _dir: NavDirection, _config: &Config) -> ComponentEvent {
        ComponentEvent::Handled
    }

    fn handle_page(&mut self, _delta: i32, _config: &Config) -> ComponentEvent {
        ComponentEvent::Handled
    }

    fn handle_go_to_page(&mut self, _p: usize, _config: &Config) -> ComponentEvent {
        ComponentEvent::Handled
    }

    fn update(&mut self, msg: Msg, _apps: &[AppEntry], config: &Config) -> Task<Msg> {
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
                    return Task::none();
                }
                return self.start_query(prompt, config);
            }
            Msg::CopyResponse => {
                if let AiStatus::Done(text) = &self.status {
                    let _ = std::process::Command::new("wl-copy").arg(text).spawn();
                    self.copy_feedback = true;
                    return Task::perform(
                        async { tokio::time::sleep(Duration::from_secs(2)).await },
                        |_| Msg::Copied,
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
        }
        Task::none()
    }

    fn view<'a>(&'a self, _apps: &'a [AppEntry], _config: &'a Config) -> Element<'a, Msg> {
        let content = iced::widget::column![
            search_bar(&self.query, &self.shake, SearchIcon::Robot, Msg::QueryChanged),
            ai_panel(
                &self.status,
                &self.prompt,
                self.copy_feedback,
                &self.response_items,
                Msg::CopyResponse,
                Msg::Retry,
                Msg::LinkClicked,
            ),
        ]
        .spacing(16)
        .padding(iced::Padding { top: 24.0, bottom: 24.0, left: 80.0, right: 80.0 })
        .width(iced::Length::Fill)
        .height(iced::Length::Fill);

        content.into()
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
