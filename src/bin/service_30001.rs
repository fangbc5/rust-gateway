use axum::{routing::get, Router, Json, extract::Path};
use serde_json::{json, Value};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let app = Router::new()
        .route("/api/user/:id", get(|Path(id): Path<String>| async move {
            Json(json!({
                "message": "API User service",
                "user_id": id,
                "service": "30001"
            }))
        }))
        .route("/api/user/:id/profile", get(|Path(id): Path<String>| async move {
            Json(json!({
                "message": "User profile",
                "user_id": id,
                "service": "30001"
            }))
        }))
        .route("/auth", get(|| async { "Auth service - root" }))
        .route("/auth/login", get(|| async { "Auth login" }))
        .route("/auth/logout", get(|| async { "Auth logout" }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:30001").await?;
    tracing::info!("API/Auth service listening on http://{}", listener.local_addr()?);

    axum::serve(listener, app).await?;
    Ok(())
} 