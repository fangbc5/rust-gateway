use axum::{routing::get, Router, Json, extract::Path};
use serde_json::{json, Value};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let app = Router::new()
        .route("/", get(|| async { "Service 30000 - Root" }))
        .route("/user", get(|| async { "Service 30000 - User endpoint" }))
        .route("/user/*path", get(|| async { "Service 30000 - User wildcard" }))
        .route("/user/profile", get(|| async { "Service 30000 - User profile" }))
        .route("/user/settings", get(|| async { "Service 30000 - User settings" }))
        .route("/api/user/:id", get(handle_user_id))
        .route("/static/*path", get(|| async { "Service 30000 - Static files" }))
        .route("/files/:filename", get(|| async { "Service 30000 - File pattern" }))
        .route("/api/v:version/user/:id/posts/*path", get(handle_complex_path))
        .route("/auth", get(|| async { "Service 30000 - Auth endpoint" }))
        .route("/auth/*path", get(|| async { "Service 30000 - Auth wildcard" }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:30000").await?;
    tracing::info!("Service 30000 listening on http://{}", listener.local_addr()?);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_user_id(Path(id): Path<String>) -> Json<Value> {
    Json(json!({
        "service": "30000",
        "endpoint": "user_id",
        "id": id,
        "message": "Service 30000 - User ID endpoint"
    }))
}

async fn handle_user_id_numeric(Path(id): Path<String>) -> Json<Value> {
    Json(json!({
        "service": "30000",
        "endpoint": "user_id_numeric",
        "id": id,
        "message": "Service 30000 - User ID numeric endpoint"
    }))
}

async fn handle_complex_path(Path((version, id, path)): Path<(String, String, String)>) -> Json<Value> {
    Json(json!({
        "service": "30000",
        "endpoint": "complex_path",
        "version": version,
        "id": id,
        "path": path,
        "message": "Service 30000 - Complex path endpoint"
    }))
} 