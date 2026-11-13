use crate::{
    domain::{BuildInfo, Message, Project, SourceFile, bounded_text, validate_files},
    prompts,
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
    /// plan) guides the model and is recorded in the build metadata; the
    /// project's prompt is only the user's brief.
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
        let started = std::time::Instant::now();
        let request = Message {
            role: "user".into(),
            content: prompts::developer(&prompt, context),
        };
        let output = self
            .providers
            .complete_json(&connection, std::slice::from_ref(&request))
            .await?;
        let (files, repaired) = match usable_files(&output) {
            Ok(files) => (files, false),
            Err(error) => {
                // One repair request: show the model its reply and the reason
                // it was rejected. The reply is bounded to stay within limits.
                let conversation = [
                    request,
                    Message {
                        role: "assistant".into(),
                        content: bounded_text(&output, 100_000).into(),
                    },
                    Message {
                        role: "user".into(),
                        content: prompts::repair(&error.to_string()),
                    },
                ];
                let output = self
                    .providers
                    .complete_json(&connection, &conversation)
                    .await?;
                let files = usable_files(&output).map_err(|error| {
                    anyhow::anyhow!(
                        "Provider did not return a usable project after one repair attempt: {error}"
                    )
                })?;
                (files, true)
            }
        };
        let project = Project {
            id: uuid::Uuid::new_v4(),
            prompt,
            files,
            verification: "Source generated; behavior and security have not been verified".into(),
            build: Some(BuildInfo {
                provider: connection.provider.clone(),
                model: connection.model.clone(),
                prompt_version: prompts::PROMPT_VERSION.into(),
                plan: context.into(),
                created_unix: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|elapsed| elapsed.as_secs())
                    .unwrap_or_default(),
                duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                repaired,
            }),
        };
        self.store.save(&project).await?;
        Ok(project)
    }
}

/// Parses and validates a provider reply as a project file bundle.
fn usable_files(output: &str) -> Result<Vec<SourceFile>> {
    let files = parse_generated_files(output)?;
    validate_files(&files)?;
    Ok(files)
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
