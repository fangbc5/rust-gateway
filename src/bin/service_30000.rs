use axum::{routing::get, Router, Json, extract::Path};
use serde_json::{json, Value};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let app = Router::new()
        .route("/user", get(|| async { "User service - root" }))
        .route("/user/*", get(|| async { "User service - wildcard" }))
        .route("/user/profile", get(|| async { "User profile" }))
        .route("/user/settings", get(|| async { "User settings" }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:30000").await?;
    tracing::info!("User service listening on http://{}", listener.local_addr()?);

    axum::serve(listener, app).await?;
    Ok(())
} 