mod config;
mod response;
use crate::domain::{MAX_CONVERSATION_BYTES, MAX_MESSAGES, Message};
use anyhow::{Result, bail, ensure};
pub use config::{Connection, default_endpoint};
use serde_json::json;
use std::time::Duration;
#[derive(Clone)]
pub struct Providers {
    client: reqwest::Client,
}
impl Providers {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }
    /// Requests a plain-text completion.
    pub async fn complete(&self, c: &Connection, messages: &[Message]) -> Result<String> {
        self.request(c, messages, false).await
    }
    /// Requests a JSON object, using the provider's native JSON mode where it
    /// is reliably available. Callers must still parse and validate the reply.
    pub async fn complete_json(&self, c: &Connection, messages: &[Message]) -> Result<String> {
        self.request(c, messages, true).await
    }
    async fn request(&self, c: &Connection, messages: &[Message], json: bool) -> Result<String> {
        ensure!(
            !c.model.trim().is_empty() && c.model.len() <= 200,
            "Choose a model ID"
        );
        ensure!(
            !messages.is_empty() && messages.len() <= MAX_MESSAGES,
            "Conversation requires 1–200 messages"
        );
        ensure!(
            messages
                .iter()
                .all(|m| ["user", "assistant"].contains(&m.role.as_str())),
            "Invalid message role"
        );
        ensure!(
            messages.iter().map(|m| m.content.len()).sum::<usize>() <= MAX_CONVERSATION_BYTES,
            "Conversation too large"
        );
        let key_name = match c.provider.as_str() {
            "openai" => "OPENAI_API_KEY",
            "anthropic" => "ANTHROPIC_API_KEY",
            "gemini" => "GEMINI_API_KEY",
            "groq" => "GROQ_API_KEY",
            "ollama" => "",
            _ => bail!("Unsupported provider"),
        };
        let key = if c.api_key.is_empty() {
            std::env::var(key_name).unwrap_or_default()
        } else {
            c.api_key.clone()
        };
        ensure!(
            c.provider == "ollama" || !key.is_empty(),
            "Provider API key is required"
        );
        let (url, mut body): (String, serde_json::Value) = match c.provider.as_str() {
            "openai" => (
                "https://api.openai.com/v1/responses".into(),
                json!({"model":c.model,"input":messages,"store":false,"max_output_tokens":8192}),
            ),
            "anthropic" => (
                "https://api.anthropic.com/v1/messages".into(),
                json!({"model":c.model,"messages":messages,"max_tokens":8192}),
            ),
            "groq" => (
                "https://api.groq.com/openai/v1/chat/completions".into(),
                json!({"model":c.model,"messages":messages,"max_completion_tokens":8192}),
            ),
            "gemini" => {
                ensure!(
                    c.model
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || "-_.".contains(ch)),
                    "Invalid Gemini model ID"
                );
                (
                    format!(
                        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
                        c.model
                    ),
                    json!({"contents": messages.iter().map(|m| json!({"role":if m.role == "assistant" {"model"} else {"user"},"parts":[{"text":m.content}]})).collect::<Vec<_>>()}),
                )
            }
            "ollama" => {
                let endpoint = reqwest::Url::parse(c.endpoint.trim())
                    .map_err(|_| anyhow::anyhow!("Enter a valid Ollama origin URL"))?;
                ensure!(
                    ["http", "https"].contains(&endpoint.scheme())
                        && [Some("localhost"), Some("127.0.0.1"), Some("[::1]")]
                            .contains(&endpoint.host_str())
                        && endpoint.path() == "/"
                        && endpoint.username().is_empty()
                        && endpoint.password().is_none()
                        && endpoint.query().is_none()
                        && endpoint.fragment().is_none(),
                    "Ollama must use a loopback origin"
                );
                (
                    // Build from the validated URL, not the raw user text.
                    endpoint.join("api/chat")?.to_string(),
                    json!({"model":c.model,"messages":messages,"stream":false}),
                )
            }
            _ => unreachable!(),
        };
        if json {
            match c.provider.as_str() {
                "openai" => body["text"] = json!({"format":{"type":"json_object"}}),
                "gemini" => {
                    body["generationConfig"] = json!({"responseMimeType":"application/json"})
                }
                "ollama" => body["format"] = json!("json"),
                // Anthropic has no JSON mode, and Groq's depends on the model;
                // both rely on the prompt plus local parsing and validation.
                _ => {}
            }
        }
        let mut attempt = 0;
        let mut response = loop {
            let mut request = self.client.post(url.as_str()).json(&body);
            request = match c.provider.as_str() {
                "anthropic" => request
                    .header("x-api-key", &key)
                    .header("anthropic-version", "2023-06-01"),
                "gemini" => request.header("x-goog-api-key", &key),
                "ollama" => request,
                _ => request.bearer_auth(&key),
            };
            match request.send().await {
                Ok(response) if response.status().is_success() => break response,
                Ok(response) if attempt < MAX_RETRIES && retryable(response.status()) => {
                    let wait = retry_after(&response).unwrap_or_else(|| backoff(attempt));
                    tokio::time::sleep(wait).await;
                }
                Ok(response) => bail!("Provider returned HTTP {}", response.status()),
                Err(error) if attempt < MAX_RETRIES && error.is_connect() => {
                    tokio::time::sleep(backoff(attempt)).await;
                }
                Err(_) => bail!("Provider connection failed"),
            }
            attempt += 1;
        };
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len() + chunk.len() <= 8 * 1024 * 1024,
                "Provider response too large"
            );
            bytes.extend_from_slice(&chunk);
        }
        response::decode(&c.provider, &bytes)
    }
}

/// Retries after the first attempt for rate limits, server errors, and
/// connection failures. Other failures are returned immediately.
const MAX_RETRIES: u32 = 2;

fn retryable(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

/// 500 ms, then 1 s.
fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(500 << attempt)
}

/// Honors a numeric `Retry-After` header, capped so the UI stays responsive.
fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    let seconds: u64 = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(Duration::from_secs(seconds.min(10)))
}
