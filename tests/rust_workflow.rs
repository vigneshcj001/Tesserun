use axum::{Json, Router, routing::post};
use serde_json::json;
use tesserun::{
    domain::{Message, Project, SourceFile},
    providers::Connection,
    service::Service,
};

#[tokio::test]
async fn project_list_only_parses_the_latest_revision() {
    let root = std::env::temp_dir().join(format!("tesserun-history-test-{}", uuid::Uuid::new_v4()));
    let store = tesserun::storage::Store::new(root.clone());
    let mut project = Project {
        id: uuid::Uuid::new_v4(),
        prompt: "History".into(),
        files: vec![SourceFile {
            path: "index.html".into(),
            content: "Old".into(),
        }],
        verification: "unverified".into(),
    };
    store.save(&project).await.unwrap();
    let old = tokio::fs::read_dir(&root)
        .await
        .unwrap()
        .next_entry()
        .await
        .unwrap()
        .unwrap()
        .path();
    tokio::fs::write(&old, "invalid historical JSON")
        .await
        .unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH)
        .unwrap();
    project.files[0].content = "Newest revision".into();
    store.save(&project).await.unwrap();
    // Unrelated JSON files are not revision records and must not be opened.
    tokio::fs::write(root.join("unrelated.json"), "not a project")
        .await
        .unwrap();
    let loaded = store.list().await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].files[0].content, "Newest revision");
    assert_eq!(
        tokio::fs::read_to_string(old).await.unwrap(),
        "invalid historical JSON"
    );
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn generated_project_round_trips_and_does_not_persist_credentials() {
    let source = json!({"files":[{"path":"index.html","content":"<h1>Fixture</h1>"}]}).to_string();
    let upstream = Router::new().route(
        "/api/chat",
        post(move || {
            let source = source.clone();
            async move { Json(json!({"message":{"content":source},"done":true})) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, upstream).await.unwrap();
    });
    let root = std::env::temp_dir().join(format!("tesserun-rust-test-{}", uuid::Uuid::new_v4()));
    let service = Service::new(root.clone()).unwrap();
    let connection = Connection {
        provider: "ollama".into(),
        model: "fixture".into(),
        endpoint: format!("http://{address}"),
        api_key: "never-persist-this".into(),
    };
    let project = service
        .build("A test page".into(), connection)
        .await
        .unwrap();
    assert_eq!(project.files[0].content, "<h1>Fixture</h1>");
    let mut edited = project.clone();
    edited.files[0].content = "<h1>Edited</h1>".into();
    service.store.save(&edited).await.unwrap();
    let loaded = service.store.list().await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].files[0].content, "<h1>Edited</h1>");
    let mut files = tokio::fs::read_dir(&root).await.unwrap();
    while let Some(entry) = files.next_entry().await.unwrap() {
        assert!(
            !tokio::fs::read_to_string(entry.path())
                .await
                .unwrap()
                .contains("never-persist-this")
        );
    }
    task.abort();
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn refuses_unsupported_paths_and_unauthenticated_api_calls() {
    let root = std::env::temp_dir().join(format!("tesserun-rust-test-{}", uuid::Uuid::new_v4()));
    let service = Service::new(root.clone()).unwrap();
    let project = Project {
        id: uuid::Uuid::new_v4(),
        prompt: "test".into(),
        files: vec![SourceFile {
            path: "../escape".into(),
            content: String::new(),
        }],
        verification: "unverified".into(),
    };
    assert!(service.store.save(&project).await.is_err());
    let app = tesserun::api::router(service, "a".repeat(32)).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = reqwest::Client::new();
    assert_eq!(
        client
            .get(format!("http://{address}/api/builds"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .get(format!("http://{address}/api/health"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    task.abort();
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test]
async fn rejects_remote_ollama_without_making_an_external_request() {
    let providers = tesserun::providers::Providers::new().unwrap();
    let connection = Connection {
        endpoint: "http://example.com".into(),
        model: "fixture".into(),
        ..Default::default()
    };
    assert!(
        providers
            .complete(
                &connection,
                &[Message {
                    role: "user".into(),
                    content: "hello".into()
                }]
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_damaged_revision_does_not_hide_other_projects() {
    let root = std::env::temp_dir().join(format!("tesserun-damaged-test-{}", uuid::Uuid::new_v4()));
    let store = tesserun::storage::Store::new(root.clone());
    let healthy = Project {
        id: uuid::Uuid::new_v4(),
        prompt: "Healthy".into(),
        files: vec![SourceFile {
            path: "index.html".into(),
            content: "<h1>Healthy</h1>".into(),
        }],
        verification: "unverified".into(),
    };
    store.save(&healthy).await.unwrap();
    let damaged = root.join(format!(
        "{}-{}.json",
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4()
    ));
    tokio::fs::write(&damaged, "not valid JSON").await.unwrap();
    let loaded = store.list().await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, healthy.id);
    tokio::fs::remove_dir_all(root).await.unwrap();
}
