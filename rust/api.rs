use crate::{domain::Message, providers::Connection, service::Service};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
struct ApiState {
    service: Service,
    token: Arc<String>,
}
#[derive(Deserialize)]
struct BuildInput {
    prompt: String,
    connection: Connection,
}
#[derive(Deserialize)]
struct ChatInput {
    connection: Connection,
    messages: Vec<Message>,
}
fn failure(error: anyhow::Error) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error":{"message":error.to_string()}})),
    )
        .into_response()
}
async fn authorize(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    let supplied = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let expected = format!("Bearer {}", state.token);
    // Fixed-length comparison; never accept an absent token or a browser Origin.
    let same = supplied.len() == expected.len()
        && supplied
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |a, (b, c)| a | (b ^ c))
            == 0;
    if !same || request.headers().contains_key("origin") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":{"message":"Native API authorization required"}})),
        )
            .into_response();
    }
    next.run(request).await
}
pub fn router(service: Service, token: String) -> anyhow::Result<Router> {
    anyhow::ensure!(
        token.len() >= 32,
        "TESSERUN_API_TOKEN must contain at least 32 characters"
    );
    let state = ApiState {
        service,
        token: Arc::new(token),
    };
    let protected = Router::new()
        .route("/api/builds", get(list).post(build))
        .route("/api/chat", post(chat))
        .route_layer(middleware::from_fn_with_state(state.clone(), authorize));
    Ok(Router::new()
        .route(
            "/api/health",
            get(|| async { Json(json!({"ok":true,"service":"tesserun-rust"})) }),
        )
        .merge(protected)
        .layer(DefaultBodyLimit::max(512 * 1024))
        .with_state(state))
}
async fn list(State(state): State<ApiState>) -> Response {
    match state.service.store.list().await {
        Ok(projects) => Json(json!({"jobs":projects})).into_response(),
        Err(e) => failure(e),
    }
}
async fn build(State(state): State<ApiState>, Json(input): Json<BuildInput>) -> Response {
    match state.service.build(input.prompt, input.connection).await {
        Ok(project) => Json(json!({"job":project})).into_response(),
        Err(e) => failure(e),
    }
}
async fn chat(State(state): State<ApiState>, Json(input): Json<ChatInput>) -> Response {
    match state
        .service
        .providers
        .complete(&input.connection, &input.messages)
        .await
    {
        Ok(content) => Json(json!({"content":content})).into_response(),
        Err(e) => failure(e),
    }
}
