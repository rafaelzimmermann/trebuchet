use crate::config::AiProvider;

pub struct AiRequest {
    pub prompt: String,
    pub provider: AiProvider,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

pub async fn query(req: AiRequest) -> Result<String, String> {
    match req.provider {
        AiProvider::OpenAi    => openai(req).await,
        AiProvider::Anthropic => anthropic(req).await,
        AiProvider::Gemini    => gemini(req).await,
        AiProvider::Ollama    => ollama(req).await,
    }
}

fn new_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|e| e.to_string())
}

fn http_error(status: reqwest::StatusCode) -> String {
    match status.as_u16() {
        401 | 403 => "Authentication failed — check your API key".to_string(),
        429       => "Rate limit exceeded — try again later".to_string(),
        n if n >= 500 => format!("Server error ({})", n),
        n             => format!("HTTP {}", n),
    }
}

// ── OpenAI ──────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct OpenAiResp { choices: Vec<OpenAiChoice> }
#[derive(serde::Deserialize)]
struct OpenAiChoice { message: OpenAiMsg }
#[derive(serde::Deserialize)]
struct OpenAiMsg { content: String }

async fn openai(req: AiRequest) -> Result<String, String> {
    let client = new_client()?;
    let key = req.api_key.as_deref().unwrap_or("");
    let model = req.model.as_deref().unwrap_or("gpt-4o");
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": req.prompt}]
    });
    let base = req.base_url.as_deref().unwrap_or("https://api.openai.com");
    let url = format!("{}/v1/chat/completions", base.trim_end_matches('/'));
    let resp = client
        .post(url)
        .bearer_auth(key)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(http_error(resp.status()));
    }
    let parsed: OpenAiResp = resp.json().await.map_err(|e| e.to_string())?;
    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "No response from OpenAI".to_string())
}

// ── Anthropic ───────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct AnthropicResp { content: Vec<AnthropicContent> }
#[derive(serde::Deserialize)]
struct AnthropicContent { text: String }

async fn anthropic(req: AiRequest) -> Result<String, String> {
    let client = new_client()?;
    let key = req.api_key.as_deref().unwrap_or("");
    let model = req.model.as_deref().unwrap_or("claude-opus-4-6");
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": req.prompt}]
    });
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(http_error(resp.status()));
    }
    let parsed: AnthropicResp = resp.json().await.map_err(|e| e.to_string())?;
    parsed
        .content
        .into_iter()
        .next()
        .map(|c| c.text)
        .ok_or_else(|| "No response from Anthropic".to_string())
}

// ── Gemini ───────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct GeminiResp { candidates: Vec<GeminiCandidate> }
#[derive(serde::Deserialize)]
struct GeminiCandidate { content: GeminiContent }
#[derive(serde::Deserialize)]
struct GeminiContent { parts: Vec<GeminiPart> }
#[derive(serde::Deserialize)]
struct GeminiPart { text: String }

async fn gemini(req: AiRequest) -> Result<String, String> {
    let client = new_client()?;
    let key = req.api_key.as_deref().unwrap_or("");
    let model = req.model.as_deref().unwrap_or("gemini-2.0-flash");
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, key
    );
    let body = serde_json::json!({
        "contents": [{"parts": [{"text": req.prompt}]}]
    });
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(http_error(resp.status()));
    }
    let parsed: GeminiResp = resp.json().await.map_err(|e| e.to_string())?;
    parsed
        .candidates
        .into_iter()
        .next()
        .and_then(|c| c.content.parts.into_iter().next())
        .map(|p| p.text)
        .ok_or_else(|| "No response from Gemini".to_string())
}

// ── Ollama ───────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct OllamaResp { message: OllamaMsg }
#[derive(serde::Deserialize)]
struct OllamaMsg { content: String }

async fn ollama(req: AiRequest) -> Result<String, String> {
    let client = new_client()?;
    let base = req.base_url.as_deref().unwrap_or("http://localhost:11434");
    let model = req.model.as_deref().unwrap_or("llama3.2");
    let url = format!("{}/api/chat", base.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": req.prompt}],
        "stream": false
    });
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(http_error(resp.status()));
    }
    let parsed: OllamaResp = resp.json().await.map_err(|e| e.to_string())?;
    Ok(parsed.message.content)
}
