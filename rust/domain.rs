use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Request limits enforced by the provider adapter.
pub const MAX_MESSAGES: usize = 200;
pub const MAX_CONVERSATION_BYTES: usize = 300_000;

/// The most recent messages that fit within the given limits, beginning with a
/// user turn, so a long session-only chat keeps working instead of exceeding
/// provider request limits forever.
pub fn recent_conversation(
    messages: &[Message],
    max_messages: usize,
    max_bytes: usize,
) -> Vec<Message> {
    let mut start = messages.len();
    let mut bytes = 0;
    while start > 0 && messages.len() - start < max_messages {
        let next = bytes + messages[start - 1].content.len();
        if next > max_bytes {
            break;
        }
        bytes = next;
        start -= 1;
    }
    while start < messages.len() && messages[start].role != "user" {
        start += 1;
    }
    if start == messages.len() {
        // Nothing fits; send the latest turn so the provider reports why.
        return messages.last().cloned().into_iter().collect();
    }
    messages[start..].to_vec()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: uuid::Uuid,
    pub prompt: String,
    pub files: Vec<SourceFile>,
    pub verification: String,
}

pub fn validate_files(files: &[SourceFile]) -> Result<()> {
    ensure!(
        !files.is_empty() && files.len() <= 32,
        "A project requires 1–32 files"
    );
    let mut names = std::collections::HashSet::new();
    let mut bytes = 0;
    for file in files {
        ensure!(
            [
                "index.html",
                "styles.css",
                "app.js",
                "logic.mjs",
                "README.md"
            ]
            .contains(&file.path.as_str()),
            "Unsupported source path"
        );
        ensure!(names.insert(file.path.as_str()), "Duplicate source path");
        bytes += file.content.len();
    }
    ensure!(bytes <= 4 * 1024 * 1024, "Source exceeds 4 MB");
    ensure!(names.contains("index.html"), "Missing index.html");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_escape_and_duplicates() {
        assert!(
            validate_files(&[SourceFile {
                path: "../secret".into(),
                content: String::new()
            }])
            .is_err()
        );
        assert!(
            validate_files(&[
                SourceFile {
                    path: "index.html".into(),
                    content: String::new()
                },
                SourceFile {
                    path: "index.html".into(),
                    content: String::new()
                }
            ])
            .is_err()
        );
    }
    fn message(role: &str, content: &str) -> Message {
        Message {
            role: role.into(),
            content: content.into(),
        }
    }
    #[test]
    fn recent_conversation_keeps_latest_turns_and_starts_with_user() {
        let history = [
            message("user", "aaaa"),
            message("assistant", "bbbb"),
            message("user", "cccc"),
            message("assistant", "dddd"),
            message("user", "eeee"),
        ];
        assert_eq!(recent_conversation(&history, 200, 1000).len(), 5);
        // Four messages fit, but the window must not begin with an assistant turn.
        let window = recent_conversation(&history, 4, 1000);
        assert_eq!(window.len(), 3);
        assert_eq!(window[0].content, "cccc");
        let window = recent_conversation(&history, 200, 8);
        assert_eq!(window.len(), 1);
        assert_eq!(window[0].content, "eeee");
    }
}
