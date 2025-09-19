use axum::{routing::get, Router};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = Router::new().route("/helloworld", get(|| async { "hello world" }));

    let listener = TcpListener::bind("0.0.0.0:30001").await?;
    tracing::info!("mini server listening on http://{}", listener.local_addr()?);

    axum::serve(listener, app).await?;
    Ok(())
} 