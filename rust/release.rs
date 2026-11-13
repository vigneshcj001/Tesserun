//! Explicitly opted-in publishing of static source to Vercel.
//! This adapter does not execute or verify generated code locally.
use crate::domain::{Project, validate_files};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Serialize;
use serde_json::{Value, json};
use std::time::Duration;

const API: &str = "https://api.vercel.com/v13/deployments";

#[derive(Clone, Debug, Serialize)]
pub struct Deployment {
    pub url: String,
    pub status: String,
    pub id: String,
}

struct Configuration {
    enabled: bool,
    token: String,
    project: Option<String>,
    team: Option<String>,
}

impl Configuration {
    fn environment() -> Self {
        let optional = |name| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        };
        Self {
            enabled: std::env::var("TESSERUN_ENABLE_VERCEL_DEPLOYMENTS").as_deref() == Ok("true"),
            token: optional("VERCEL_TOKEN").unwrap_or_default(),
            project: optional("VERCEL_PROJECT_ID"),
            team: optional("VERCEL_TEAM_ID"),
        }
    }

    fn validate(&self, confirmed: bool) -> Result<()> {
        ensure!(
            self.enabled,
            "Set TESSERUN_ENABLE_VERCEL_DEPLOYMENTS=true to enable Vercel publishing"
        );
        ensure!(
            confirmed,
            "Confirm this production deployment before publishing"
        );
        ensure!(
            !self.token.trim().is_empty(),
            "Set VERCEL_TOKEN before publishing"
        );
        Ok(())
    }
}

/// Indicates server-side configuration, not successful credential validation.
pub fn configured() -> bool {
    let configuration = Configuration::environment();
    configuration.enabled && !configuration.token.trim().is_empty()
}

/// Creates a real production deployment after server opt-in and user confirmation.
/// A queued response is not reported as a live/verified application.
pub async fn deploy(project: Project, confirmed: bool) -> Result<Deployment> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    deploy_with(
        &client,
        &project,
        confirmed,
        &Configuration::environment(),
        API,
    )
    .await
}

async fn deploy_with(
    client: &reqwest::Client,
    project: &Project,
    confirmed: bool,
    configuration: &Configuration,
    endpoint: &str,
) -> Result<Deployment> {
    configuration.validate(confirmed)?;
    validate_files(&project.files)?;
    let files: Vec<_> = project.files.iter().filter(|file| file.path != "README.md").map(|file| {
        json!({"file": file.path, "data": STANDARD.encode(file.content.as_bytes()), "encoding": "base64"})
    }).collect();
    let mut payload = json!({
        "name": format!("tesserun-{}", project.id.simple()),
        "target": "production",
        "files": files,
        "projectSettings": {"framework": null, "buildCommand": null, "installCommand": null, "outputDirectory": "."},
        "meta": {"generatedBy": "tesserun-rust", "runtimeVerification": "not-performed"}
    });
    if let Some(id) = &configuration.project {
        payload["project"] = json!(id);
    }
    let mut url = reqwest::Url::parse(endpoint)?;
    url.query_pairs_mut()
        .append_pair("skipAutoDetectionConfirmation", "1");
    if let Some(team) = &configuration.team {
        url.query_pairs_mut().append_pair("teamId", team);
    }
    let mut response = client
        .post(url)
        .bearer_auth(configuration.token.trim())
        .json(&payload)
        .send()
        .await
        .map_err(|_| {
            anyhow::anyhow!("Vercel could not be reached. Check the connection and credentials.")
        })?;
    ensure!(
        response.status().is_success(),
        "Vercel returned HTTP {}. Check credentials, project access, and quota.",
        response.status()
    );
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| anyhow::anyhow!("Vercel response was interrupted"))?
    {
        ensure!(
            body.len() + chunk.len() <= 1024 * 1024,
            "Vercel response exceeds the supported size"
        );
        body.extend_from_slice(&chunk);
    }
    let response: Value = serde_json::from_slice(&body)
        .map_err(|_| anyhow::anyhow!("Vercel returned an invalid deployment response"))?;
    let id = response["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Vercel did not return a deployment ID"))?;
    let state = response["readyState"].as_str().unwrap_or("QUEUED");
    ensure!(
        !["ERROR", "CANCELED"].contains(&state),
        "Vercel reported that deployment failed or was cancelled"
    );
    let hostname = response["url"]
        .as_str()
        .filter(|url| !url.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Vercel did not return a deployment URL"))?;
    let parsed = reqwest::Url::parse(&format!("https://{hostname}"))?;
    ensure!(
        parsed.scheme() == "https"
            && parsed.host_str().is_some()
            && parsed.username().is_empty()
            && parsed.password().is_none(),
        "Vercel returned an invalid deployment URL"
    );
    Ok(Deployment {
        url: parsed.to_string(),
        status: if state == "READY" { "live" } else { "queued" }.into(),
        id: id.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SourceFile;
    use axum::{Json, Router, extract::State, http::HeaderMap, routing::post};
    use std::sync::{Arc, Mutex};

    fn project() -> Project {
        Project {
            id: uuid::Uuid::new_v4(),
            prompt: "Fixture".into(),
            files: vec![SourceFile {
                path: "index.html".into(),
                content: "<h1>Fixture</h1>".into(),
            }],
            verification: "unverified".into(),
            build: None,
        }
    }

    #[tokio::test]
    async fn publishing_preflight_rejects_without_network_or_confirmation() {
        let client = reqwest::Client::new();
        let mut config = Configuration {
            enabled: false,
            token: "fixture-private-key".into(),
            project: None,
            team: None,
        };
        assert!(
            deploy_with(&client, &project(), true, &config, "http://127.0.0.1:1")
                .await
                .unwrap_err()
                .to_string()
                .contains("enable")
        );
        config.enabled = true;
        assert!(
            deploy_with(&client, &project(), false, &config, "http://127.0.0.1:1")
                .await
                .unwrap_err()
                .to_string()
                .contains("Confirm")
        );
        config.token.clear();
        assert!(
            deploy_with(&client, &project(), true, &config, "http://127.0.0.1:1")
                .await
                .unwrap_err()
                .to_string()
                .contains("VERCEL_TOKEN")
        );
    }

    #[tokio::test]
    async fn creates_an_inline_deployment_without_serializing_the_token() {
        type Captured = Arc<Mutex<Option<Value>>>;
        async fn create(
            State(captured): State<Captured>,
            headers: HeaderMap,
            Json(body): Json<Value>,
        ) -> Json<Value> {
            assert_eq!(headers["authorization"], "Bearer fixture-private-key");
            *captured.lock().unwrap() = Some(body);
            Json(json!({"id":"dpl_fixture", "url":"fixture.vercel.app", "readyState":"QUEUED"}))
        }
        let captured: Captured = Arc::new(Mutex::new(None));
        let app = Router::new()
            .route("/v13/deployments", post(create))
            .with_state(captured.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/v13/deployments", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let config = Configuration {
            enabled: true,
            token: "fixture-private-key".into(),
            project: Some("prj_fixture".into()),
            team: Some("team_fixture".into()),
        };
        let deployment = deploy_with(
            &reqwest::Client::new(),
            &project(),
            true,
            &config,
            &endpoint,
        )
        .await
        .unwrap();
        assert_eq!(deployment.status, "queued");
        assert_eq!(deployment.url, "https://fixture.vercel.app/");
        let body = captured.lock().unwrap().take().unwrap();
        assert_eq!(body["project"], "prj_fixture");
        assert_eq!(body["target"], "production");
        assert_eq!(
            body["files"][0]["data"],
            STANDARD.encode("<h1>Fixture</h1>")
        );
        assert!(!body.to_string().contains("fixture-private-key"));
        task.abort();
    }
}
