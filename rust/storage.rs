use crate::domain::{Project, validate_files};
use anyhow::{Result, ensure};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Store {
    root: PathBuf,
    lock: Arc<Mutex<()>>,
}
impl Store {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            lock: Arc::new(Mutex::new(())),
        }
    }
    pub async fn save(&self, project: &Project) -> Result<()> {
        validate_files(&project.files)?;
        let _guard = self.lock.lock().await;
        tokio::fs::create_dir_all(&self.root).await?;
        // Immutable revisions avoid replacing an existing Windows file.
        let target = self
            .root
            .join(format!("{}-{}.json", project.id, uuid::Uuid::new_v4()));
        let temp = target.with_extension("tmp");
        tokio::fs::write(&temp, serde_json::to_vec_pretty(project)?).await?;
        tokio::fs::rename(temp, target).await?;
        Ok(())
    }
    pub async fn list(&self) -> Result<Vec<Project>> {
        tokio::fs::create_dir_all(&self.root).await?;
        let mut entries = tokio::fs::read_dir(&self.root).await?;
        let mut latest = std::collections::HashMap::new();
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if !entry.file_type().await?.is_file() {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some(id) = revision_id(name) else {
                continue;
            };
            let metadata = entry.metadata().await?;
            let rank = (metadata.modified()?, name.to_owned());
            match latest.entry(id) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert((rank, metadata.len()));
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    if rank > entry.get().0 {
                        entry.insert((rank, metadata.len()));
                    }
                }
            }
        }
        let mut revisions: Vec<_> = latest.into_iter().collect();
        revisions.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        let mut projects = Vec::with_capacity(revisions.len());
        for (id, ((_, name), size)) in revisions {
            ensure!(size <= 32 * 1024 * 1024, "Saved revision exceeds 32 MB");
            let bytes = tokio::fs::read(self.root.join(name)).await?;
            let project: Project = serde_json::from_slice(&bytes)?;
            ensure!(
                project.id == id,
                "Saved revision ID does not match its filename"
            );
            validate_files(&project.files)?;
            projects.push(project);
        }
        Ok(projects)
    }
}

fn revision_id(name: &str) -> Option<uuid::Uuid> {
    let stem = name.strip_suffix(".json")?;
    let project = stem.get(..36)?;
    let revision = stem.get(36..)?;
    uuid::Uuid::parse_str(revision.strip_prefix('-')?).ok()?;
    uuid::Uuid::parse_str(project).ok()
}
