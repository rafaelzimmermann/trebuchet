use iced::{widget::markdown, time, Element, Subscription, Task};
use std::time::Duration;
use crate::ai_client::{self, AiRequest};
use crate::app::Message;
use crate::config::Config;
use crate::ui::ai_panel;

#[derive(Debug, Clone, PartialEq)]
pub enum AiStatus {
    Idle,
    Loading { tick: u8 },
    Done(String),
    Error(String),
}

pub struct AiState {
    pub status: AiStatus,
    pub prompt: String,
    pub copy_feedback: bool,
    pub response_items: Vec<markdown::Item>,
}

impl AiState {
    pub fn new() -> Self {
        Self {
            status: AiStatus::Idle,
            prompt: String::new(),
            copy_feedback: false,
            response_items: Vec::new(),
        }
    }

    pub fn start_query(&mut self, prompt: String, config: &Config) -> Task<Message> {
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
        Task::perform(ai_client::query(req), Message::AiResponse)
    }

    pub fn update(&mut self, msg: Message, config: &Config) -> Task<Message> {
        match msg {
            Message::AiResponse(result) => {
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
            Message::AiRetry => {
                let prompt = self.prompt.clone();
                if prompt.is_empty() {
                    return Task::none();
                }
                return self.start_query(prompt, config);
            }
            Message::AiCopyResponse => {
                if let AiStatus::Done(text) = &self.status {
                    let _ = std::process::Command::new("wl-copy").arg(text).spawn();
                    self.copy_feedback = true;
                    return Task::perform(
                        async { tokio::time::sleep(Duration::from_secs(2)).await },
                        |_| Message::AiCopied,
                    );
                }
            }
            Message::AiCopied => {
                self.copy_feedback = false;
            }
            Message::AiLoadingTick => {
                if let AiStatus::Loading { tick } = &mut self.status {
                    *tick = (*tick + 1) % 3;
                }
            }
            Message::LinkClicked(url) => {
                let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
            }
            _ => {}
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        ai_panel(&self.status, &self.prompt, self.copy_feedback, &self.response_items)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        if matches!(self.status, AiStatus::Loading { .. }) {
            time::every(Duration::from_millis(400)).map(|_| Message::AiLoadingTick)
        } else {
            Subscription::none()
        }
    }
}
