mod config;
mod response;
use crate::domain::Message;
use anyhow::{Result, bail, ensure};
pub use config::{Connection, default_endpoint};
use serde_json::json;
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
    pub async fn complete(&self, c: &Connection, messages: &[Message]) -> Result<String> {
        ensure!(
            !c.model.trim().is_empty() && c.model.len() <= 200,
            "Choose a model ID"
        );
        ensure!(
            !messages.is_empty() && messages.len() <= 200,
            "Conversation requires 1–200 messages"
        );
        ensure!(
            messages
                .iter()
                .all(|m| ["user", "assistant"].contains(&m.role.as_str())),
            "Invalid message role"
        );
        ensure!(
            messages.iter().map(|m| m.content.len()).sum::<usize>() <= 300_000,
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
        let (url, body) = match c.provider.as_str() {
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
                let endpoint = reqwest::Url::parse(&c.endpoint)?;
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
                    format!("{}/api/chat", c.endpoint.trim_end_matches('/')),
                    json!({"model":c.model,"messages":messages,"stream":false}),
                )
            }
            _ => unreachable!(),
        };
        let mut request = self.client.post(url).json(&body);
        request = match c.provider.as_str() {
            "anthropic" => request
                .header("x-api-key", &key)
                .header("anthropic-version", "2023-06-01"),
            "gemini" => request.header("x-goog-api-key", &key),
            "ollama" => request,
            _ => request.bearer_auth(&key),
        };
        let mut response = request
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Provider connection failed"))?;
        ensure!(
            response.status().is_success(),
            "Provider returned HTTP {}",
            response.status()
        );
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
