use anyhow::{Result, ensure};
use serde_json::Value;
pub(super) fn decode(provider: &str, bytes: &[u8]) -> Result<String> {
    let value: Value = serde_json::from_slice(bytes)?;
    let completed = match provider {
        "openai" => value["status"] == "completed",
        "anthropic" => {
            value["stop_reason"] == "end_turn" || value["stop_reason"] == "stop_sequence"
        }
        "gemini" => value["candidates"][0]["finishReason"] == "STOP",
        "groq" => value["choices"][0]["finish_reason"] == "stop",
        _ => value["done"] == true && value["done_reason"] != "length",
    };
    ensure!(
        completed,
        "Provider response was incomplete or reached its output limit"
    );
    let text = match provider {
        "openai" => value["output"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|item| item["content"].as_array().into_iter().flatten())
            .filter_map(|v| v["text"].as_str())
            .collect::<String>(),
        "anthropic" => value["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v["text"].as_str())
            .collect::<String>(),
        "gemini" => value["candidates"][0]["content"]["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|part| part["thought"] != true)
            .filter_map(|v| v["text"].as_str())
            .collect::<String>(),
        "groq" => value["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        _ => value["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
    };
    ensure!(!text.is_empty(), "Provider returned no text");
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn does_not_display_gemini_thought_parts() {
        let payload = serde_json::json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"thought":true,"text":"internal"},{"text":"Visible answer"}]}}]});
        assert_eq!(
            decode("gemini", &serde_json::to_vec(&payload).unwrap()).unwrap(),
            "Visible answer"
        );
    }
    #[test]
    fn rejects_incomplete_provider_output_even_when_it_contains_text() {
        let payload =
            serde_json::json!({"status":"incomplete","output":[{"content":[{"text":"partial"}]}]});
        assert!(decode("openai", &serde_json::to_vec(&payload).unwrap()).is_err());
    }
}
