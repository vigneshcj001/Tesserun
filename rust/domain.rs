use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
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
}
