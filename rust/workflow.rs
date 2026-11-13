//! Development roles and deterministic source checks. This module never runs
//! generated code and never represents static checks as runtime verification.
use crate::{
    domain::{Message, Project, bounded_text, validate_files},
    prompts,
    providers::Connection,
    service::Service,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Declaration order is workflow order and is used to sort reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AgentRole {
    Planner,
    Developer,
    Reviewer,
    Tester,
    Deployer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Completed,
    NeedsAttention,
    NotRun,
    AwaitingConfirmation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentRun {
    pub role: AgentRole,
    pub status: AgentStatus,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    Passed,
    Warning,
    Failed,
    NotRun,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowReport {
    pub agents: Vec<AgentRun>,
    pub checks: Vec<CheckResult>,
    pub runtime_verified: bool,
    pub deployment_ready: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DevelopmentRun {
    pub project: Project,
    pub report: WorkflowReport,
}

/// Run three actual model roles and independent deterministic source checks.
/// The original requirement is preserved in storage; the plan is context for
/// generation, not a source of test results or deployment approval.
pub async fn develop(
    service: &Service,
    prompt: String,
    connection: Connection,
) -> Result<DevelopmentRun> {
    ensure!(
        !prompt.trim().is_empty() && prompt.len() <= 20_000,
        "Prompt requires 1–20000 characters"
    );
    let plan = service
        .providers
        .complete(
            &connection,
            &[Message {
                role: "user".into(),
                content: prompts::planner(&prompt),
            }],
        )
        .await?;
    // Keep the combined request within the same bound as a direct prompt.
    let available = 20_000usize.saturating_sub(prompt.len());
    let context = bounded_text(&plan, available.min(4000));
    // The plan guides generation; only the user's original prompt is saved,
    // so a single revision is written per build.
    let project = service
        .build_with_context(prompt, context, connection.clone())
        .await?;
    let mut report = test_project(&project);
    let mut review_input = String::from(prompts::REVIEWER);
    for file in &project.files {
        review_input.push_str(&format!(
            "\n--- {} (source excerpt) ---\n{}\n",
            file.path,
            bounded_text(&file.content, 18_000)
        ));
    }
    let advisory = service
        .providers
        .complete(
            &connection,
            &[Message {
                role: "user".into(),
                content: review_input,
            }],
        )
        .await;
    if let Some(reviewer) = report
        .agents
        .iter_mut()
        .find(|agent| agent.role == AgentRole::Reviewer)
    {
        match advisory {
            Ok(advisory) => {
                reviewer.status = AgentStatus::Completed;
                reviewer.summary = format!(
                    "AI advisory review of bounded source excerpts; no code was executed.\n{}",
                    bounded_text(&advisory, 8000)
                );
            }
            Err(error) => {
                reviewer.status = AgentStatus::NeedsAttention;
                reviewer.summary = format!(
                    "Project source was saved, but the advisory reviewer did not complete: {}. Static checks remain available; retry the review before publishing.",
                    bounded_text(&error.to_string(), 300)
                );
            }
        }
    }
    report.agents.insert(
        0,
        AgentRun {
            role: AgentRole::Planner,
            status: AgentStatus::Completed,
            summary: bounded_text(&plan, 4000).into(),
        },
    );
    report.agents.insert(1, AgentRun { role: AgentRole::Developer, status: AgentStatus::Completed, summary: format!("Generated {} source files using the configured provider. No generated code was executed.", project.files.len()) });
    Ok(DevelopmentRun { project, report })
}

fn check(name: &str, status: CheckStatus, detail: impl Into<String>) -> CheckResult {
    CheckResult {
        name: name.into(),
        status,
        detail: detail.into(),
    }
}

/// Platform-owned source checks. Passing these does not establish behavior,
/// accessibility conformance, complete security, or deployability.
pub fn test_project(project: &Project) -> WorkflowReport {
    let validation = validate_files(&project.files);
    let mut checks = vec![match validation {
        Ok(()) => check(
            "Source bundle",
            CheckStatus::Passed,
            "Allowed paths, unique names, file count and source-size bounds passed.",
        ),
        Err(error) => check("Source bundle", CheckStatus::Failed, error.to_string()),
    }];
    let html = project
        .files
        .iter()
        .find(|file| file.path == "index.html")
        .map(|file| file.content.as_str())
        .unwrap_or_default();
    checks.push(if html.trim().is_empty() {
        check(
            "HTML entry point",
            CheckStatus::Failed,
            "index.html is absent or empty.",
        )
    } else {
        check(
            "HTML entry point",
            CheckStatus::Passed,
            "A nonempty HTML entry point exists; markup and behavior were not executed.",
        )
    });
    let html_lower = html.to_ascii_lowercase();
    let external_reference = [
        "src=\"http",
        "src='http",
        "href=\"http",
        "href='http",
        "src=\"//",
        "src='//",
    ]
    .iter()
    .any(|pattern| html_lower.contains(pattern));
    checks.push(if external_reference {
        check("External resource review", CheckStatus::Warning, "HTML contains external URLs. Review them before serving or publishing the output.")
    } else {
        check("External resource review", CheckStatus::Passed, "No obvious external HTML resource attributes were found. This is a lexical check, not a security guarantee.")
    });
    let runtime_access = project
        .files
        .iter()
        .filter(|file| {
            file.path.ends_with(".js")
                || file.path.ends_with(".mjs")
                || file.path.ends_with(".html")
        })
        .any(|file| {
            [
                "eval(",
                "new Function(",
                "child_process",
                "process.env",
                "document.cookie",
            ]
            .iter()
            .any(|pattern| file.content.contains(pattern))
        });
    checks.push(if runtime_access {
        check("Sensitive code review", CheckStatus::Warning, "Source contains dynamic execution, runtime access, or cookie access markers requiring manual review.")
    } else {
        check("Sensitive code review", CheckStatus::Passed, "No configured sensitive-code markers were found. This does not certify the code as safe.")
    });
    checks.push(check(
        "Behavior tests",
        CheckStatus::NotRun,
        "No generated-code sandbox is configured. No behavior tests were executed.",
    ));
    checks.push(check("Cloud deployment", CheckStatus::NotRun, "No cloud deployment adapter is configured. Export is available for manual review; it does not publish the application."));
    let attention = checks
        .iter()
        .any(|item| matches!(item.status, CheckStatus::Failed | CheckStatus::Warning));
    WorkflowReport {
        agents: vec![
            AgentRun { role: AgentRole::Reviewer, status: AgentStatus::NotRun, summary: "No AI advisory review was performed by this static check operation.".into() },
            AgentRun { role: AgentRole::Tester, status: if attention { AgentStatus::NeedsAttention } else { AgentStatus::Completed }, summary: "Deterministic source checks completed. Runtime tests have not run.".into() },
            AgentRun { role: AgentRole::Deployer, status: AgentStatus::AwaitingConfirmation, summary: "Source export or configured cloud publishing requires explicit confirmation. Application behavior remains unverified.".into() },
        ],
        checks,
        runtime_verified: false,
        deployment_ready: false,
    }
}

/// Export only into a fresh child directory; never execute or publish source.
/// UUID naming plus create_dir avoids replacing existing user files.
pub async fn export_project(project: &Project, root: PathBuf, confirmed: bool) -> Result<PathBuf> {
    ensure!(confirmed, "Confirm source export before writing files");
    validate_files(&project.files)?;
    tokio::fs::create_dir_all(&root).await?;
    let destination = root.join(format!("{}-{}", project.id, uuid::Uuid::new_v4()));
    tokio::fs::create_dir(&destination).await?;
    for file in &project.files {
        tokio::fs::write(destination.join(&file.path), &file.content).await?;
    }
    tokio::fs::write(
        destination.join("tesserun-review.json"),
        serde_json::to_vec_pretty(&test_project(project))?,
    )
    .await?;
    let manifest = serde_json::json!({
        "project_id": project.id,
        "kind": "source-export-only",
        "files": project.files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>(),
        "runtime_verified": false,
        "published": false,
    });
    tokio::fs::write(
        destination.join("tesserun-export.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )
    .await?;
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SourceFile;
    async fn development_fixture(reviewer_failure: bool) -> DevelopmentRun {
        use axum::{Json, Router, http::StatusCode, routing::post};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let fixture = Router::new().route("/api/chat", post(move || {
            let calls = calls.clone();
            async move {
                let request = calls.fetch_add(1, Ordering::SeqCst);
                if reviewer_failure && request >= 2 {
                    return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({"error":"fixture-private-provider-detail"})));
                }
                let content = match request {
                    0 => "Plan: build a readable static page with a clear heading.".to_owned(),
                    1 => serde_json::json!({"files":[{"path":"index.html","content":"<h1>Fixture page</h1>"}]}).to_string(),
                    _ => "Advisory: add keyboard interaction tests before publishing.".to_owned(),
                };
                (StatusCode::OK, Json(serde_json::json!({"message":{"content":content},"done":true})))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, fixture).await.unwrap();
        });
        let root =
            std::env::temp_dir().join(format!("tesserun-agents-test-{}", uuid::Uuid::new_v4()));
        let service = Service::new(root.clone()).unwrap();
        let connection = Connection {
            model: "fixture".into(),
            endpoint: format!("http://{address}"),
            ..Default::default()
        };
        let run = develop(&service, "Build a page".into(), connection)
            .await
            .unwrap();
        // Three role requests; a failing reviewer is retried twice.
        let expected = if reviewer_failure { 5 } else { 3 };
        assert_eq!(count.load(Ordering::SeqCst), expected);
        assert_eq!(run.project.prompt, "Build a page");
        let persisted = service.store.list().await.unwrap();
        assert!(persisted.iter().any(|project| project.id == run.project.id
            && project.files[0].content == "<h1>Fixture page</h1>"));
        assert!(!run.report.runtime_verified);
        assert!(!run.report.deployment_ready);
        server.abort();
        tokio::fs::remove_dir_all(root).await.unwrap();
        run
    }
    #[tokio::test]
    async fn development_uses_three_real_role_requests_and_keeps_checks_independent() {
        let run = development_fixture(false).await;
        assert!(
            run.report
                .agents
                .iter()
                .any(|agent| agent.role == AgentRole::Reviewer
                    && agent.summary.contains("Advisory:"))
        );
    }
    #[tokio::test]
    async fn reviewer_http_failure_preserves_generated_project_and_source_checks() {
        let run = development_fixture(true).await;
        assert_eq!(run.project.files[0].content, "<h1>Fixture page</h1>");
        let reviewer = run
            .report
            .agents
            .iter()
            .find(|agent| agent.role == AgentRole::Reviewer)
            .unwrap();
        assert_eq!(reviewer.status, AgentStatus::NeedsAttention);
        assert!(reviewer.summary.contains("429"));
        assert!(!reviewer.summary.contains("fixture-private-provider-detail"));
        assert!(
            run.report
                .checks
                .iter()
                .any(|check| check.name == "Source bundle" && check.status == CheckStatus::Passed)
        );
    }
    fn project(content: &str) -> Project {
        Project {
            id: uuid::Uuid::new_v4(),
            prompt: "A page".into(),
            files: vec![SourceFile {
                path: "index.html".into(),
                content: content.into(),
            }],
            verification: "unverified".into(),
            build: None,
        }
    }
    #[test]
    fn static_success_never_claims_runtime_verification_or_deployment_readiness() {
        let report = test_project(&project("<h1>Hello</h1>"));
        assert!(!report.runtime_verified);
        assert!(!report.deployment_ready);
        assert!(
            report
                .checks
                .iter()
                .any(|item| item.name == "Behavior tests" && item.status == CheckStatus::NotRun)
        );
    }
    #[test]
    fn flags_external_references_and_dynamic_execution() {
        let report = test_project(&project(
            "<script src='https://example.com/app.js'></script><script>eval('x')</script>",
        ));
        assert!(
            report
                .checks
                .iter()
                .filter(|item| item.status == CheckStatus::Warning)
                .count()
                >= 2
        );
    }
    #[tokio::test]
    async fn export_requires_confirmation_and_cannot_escape_its_new_directory() {
        let root =
            std::env::temp_dir().join(format!("tesserun-export-test-{}", uuid::Uuid::new_v4()));
        let valid = project("<h1>Export</h1>");
        assert!(export_project(&valid, root.clone(), false).await.is_err());
        assert!(!root.exists());
        let mut invalid = valid.clone();
        invalid.files[0].path = "../escape".into();
        assert!(export_project(&invalid, root.clone(), true).await.is_err());
        let destination = export_project(&valid, root.clone(), true).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(destination.join("index.html"))
                .await
                .unwrap(),
            "<h1>Export</h1>"
        );
        assert!(destination.join("tesserun-review.json").exists());
        tokio::fs::remove_dir_all(root).await.unwrap();
    }
}
