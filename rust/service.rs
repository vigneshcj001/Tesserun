use crate::{
    domain::{Message, Project, SourceFile, validate_files},
    providers::{Connection, Providers},
    storage::Store,
};
use anyhow::{Result, bail, ensure};

#[derive(Clone)]
pub struct Service {
    pub store: Store,
    pub providers: Providers,
}
impl Service {
    pub fn new(root: std::path::PathBuf) -> Result<Self> {
        Ok(Self {
            store: Store::new(root),
            providers: Providers::new()?,
        })
    }
    pub async fn build(&self, prompt: String, connection: Connection) -> Result<Project> {
        self.build_with_context(prompt, "", connection).await
    }
    /// Generates and saves a project. `context` (for example an implementation
    /// plan) guides the model but is not persisted; only `prompt` is saved.
    pub async fn build_with_context(
        &self,
        prompt: String,
        context: &str,
        connection: Connection,
    ) -> Result<Project> {
        ensure!(
            !prompt.trim().is_empty() && prompt.len() <= 20_000,
            "Prompt requires 1–20000 characters"
        );
        let context = if context.trim().is_empty() {
            String::new()
        } else {
            format!("\n\nImplementation plan (context only):\n{context}")
        };
        let instruction = format!(
            "Create a standalone static application for this request: {prompt}{context}\nReturn ONLY JSON: {{\"files\":[{{\"path\":\"index.html\",\"content\":\"...\"}},{{\"path\":\"styles.css\",\"content\":\"...\"}},{{\"path\":\"app.js\",\"content\":\"...\"}}]}}. No external dependencies or network calls. Only index.html, styles.css, app.js, logic.mjs and README.md paths are supported."
        );
        let output = self
            .providers
            .complete(
                &connection,
                &[Message {
                    role: "user".into(),
                    content: instruction,
                }],
            )
            .await?;
        let files = parse_generated_files(&output)?;
        validate_files(&files)?;
        let project = Project {
            id: uuid::Uuid::new_v4(),
            prompt,
            files,
            verification: "Source generated; behavior and security have not been verified".into(),
        };
        self.store.save(&project).await?;
        Ok(project)
    }
}

/// Accepts bare JSON, JSON inside a Markdown code fence, or JSON surrounded by
/// short prose, which models frequently add despite instructions.
fn parse_generated_files(output: &str) -> Result<Vec<SourceFile>> {
    #[derive(serde::Deserialize)]
    struct Generated {
        files: Vec<SourceFile>,
    }
    let trimmed = output.trim();
    let unfenced = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    let unfenced = unfenced
        .trim_end()
        .strip_suffix("```")
        .unwrap_or(unfenced)
        .trim();
    let object = unfenced
        .find('{')
        .zip(unfenced.rfind('}'))
        .filter(|&(start, end)| start < end)
        .map(|(start, end)| &unfenced[start..=end]);
    for candidate in [Some(unfenced), object].into_iter().flatten() {
        if let Ok(generated) = serde_json::from_str::<Generated>(candidate) {
            return Ok(generated.files);
        }
    }
    bail!("Provider did not return valid project JSON")
}

#[cfg(test)]
mod tests {
    use super::parse_generated_files;

    #[test]
    fn parses_bare_fenced_and_prose_wrapped_project_json() {
        let json = r#"{"files":[{"path":"index.html","content":"<h1>Hi</h1>"}]}"#;
        for output in [
            json.to_owned(),
            format!("```json\n{json}\n```"),
            format!("```JSON\n{json}\n```"),
            format!("Here is your project:\n{json}\nEnjoy!"),
        ] {
            let files = parse_generated_files(&output).unwrap();
            assert_eq!(files[0].content, "<h1>Hi</h1>");
        }
        assert!(parse_generated_files("I cannot help with that.").is_err());
    }
}
