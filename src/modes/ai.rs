use iced::{Element, Task};
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
}

impl AiState {
    pub fn new() -> Self {
        Self { status: AiStatus::Idle, prompt: String::new(), copy_feedback: false }
    }

    pub fn start_query(&mut self, prompt: String, config: &Config) -> Task<Message> {
        self.prompt = prompt.clone();
        self.status = AiStatus::Loading { tick: 0 };
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
                    Ok(text) => AiStatus::Done(text),
                    Err(err) => AiStatus::Error(err),
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
            _ => {}
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        ai_panel(&self.status, &self.prompt, self.copy_feedback)
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.status, AiStatus::Loading { .. })
    }
}
