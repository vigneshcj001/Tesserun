use crate::{
    domain::{Message, Project, SourceFile, validate_files},
    providers::{Connection, Providers},
    storage::Store,
};
use anyhow::{Result, ensure};

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
        ensure!(
            !prompt.trim().is_empty() && prompt.len() <= 20_000,
            "Prompt requires 1–20000 characters"
        );
        let instruction = format!(
            "Create a standalone static application for this request: {prompt}\nReturn ONLY JSON: {{\"files\":[{{\"path\":\"index.html\",\"content\":\"...\"}},{{\"path\":\"styles.css\",\"content\":\"...\"}},{{\"path\":\"app.js\",\"content\":\"...\"}}]}}. No external dependencies or network calls. Only index.html, styles.css, app.js, logic.mjs and README.md paths are supported."
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
        let trimmed = output
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        #[derive(serde::Deserialize)]
        struct Generated {
            files: Vec<SourceFile>,
        }
        let generated: Generated = serde_json::from_str(trimmed)
            .map_err(|_| anyhow::anyhow!("Provider did not return valid project JSON"))?;
        validate_files(&generated.files)?;
        let project = Project {
            id: uuid::Uuid::new_v4(),
            prompt,
            files: generated.files,
            verification: "Source generated; behavior and security have not been verified".into(),
        };
        self.store.save(&project).await?;
        Ok(project)
    }
}
