use axum::{routing::get, Router, Json, extract::Path};
use serde_json::{json, Value};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let app = Router::new()
        .route("/", get(|| async { "Service 30001 - Root" }))
        .route("/user", get(|| async { "Service 30001 - User endpoint" }))
        .route("/user/*path", get(|| async { "Service 30001 - User wildcard" }))
        .route("/user/profile", get(|| async { "Service 30001 - User profile" }))
        .route("/user/settings", get(|| async { "Service 30001 - User settings" }))
        .route("/api/user/:id", get(handle_user_id))
        .route("/static/*path", get(|| async { "Service 30001 - Static files" }))
        .route("/files/:filename", get(|| async { "Service 30001 - File pattern" }))
        .route("/api/v:version/user/:id/posts/*path", get(handle_complex_path))
        .route("/auth", get(|| async { "Service 30001 - Auth endpoint" }))
        .route("/auth/*path", get(|| async { "Service 30001 - Auth wildcard" }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:30001").await?;
    tracing::info!("Service 30001 listening on http://{}", listener.local_addr()?);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_user_id(Path(id): Path<String>) -> Json<Value> {
    Json(json!({
        "service": "30001",
        "endpoint": "user_id",
        "id": id,
        "message": "Service 30001 - User ID endpoint"
    }))
}

async fn handle_user_id_numeric(Path(id): Path<String>) -> Json<Value> {
    Json(json!({
        "service": "30001",
        "endpoint": "user_id_numeric",
        "id": id,
        "message": "Service 30001 - User ID numeric endpoint"
    }))
}

async fn handle_complex_path(Path((version, id, path)): Path<(String, String, String)>) -> Json<Value> {
    Json(json!({
        "service": "30001",
        "endpoint": "complex_path",
        "version": version,
        "id": id,
        "path": path,
        "message": "Service 30001 - Complex path endpoint"
    }))
}