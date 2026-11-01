#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let root = std::env::var_os("TESSERUN_RUST_DATA_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| ".tesserun-rust/projects".into());
    let token = std::env::var("TESSERUN_API_TOKEN").map_err(|_| {
        anyhow::anyhow!("Set TESSERUN_API_TOKEN to a random value of at least 32 characters")
    })?;
    let port: u16 = std::env::var("TESSERUN_RUST_PORT")
        .unwrap_or_else(|_| "8789".into())
        .parse()?;
    let app = tesserun::api::router(tesserun::service::Service::new(root)?, token)?;
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    println!("Tesserun Rust API: http://{}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
