use std::time::Instant;

use prometheus::{Encoder, TextEncoder, IntCounterVec, register_int_counter_vec, register_histogram_vec, HistogramVec};
use once_cell::sync::Lazy;
use axum::{extract::Request, http::StatusCode, middleware::Next, response::IntoResponse};

pub static HTTP_COUNTER: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "gateway_http_requests_total",
        "Total HTTP requests handled",
        &["method", "path", "status"]
    )
    .unwrap()
});

pub static HTTP_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "gateway_request_duration_seconds",
        "Request duration histogram",
        &["method", "path"]
    )
    .unwrap()
});

pub async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    (StatusCode::OK, [(axum::http::header::CONTENT_TYPE, encoder.format_type().to_string())], buffer)
}

// ===== Prometheus 中间件 =====
pub async fn prometheus_middleware(req: Request, next: Next) -> impl IntoResponse {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(req).await;
    let status = response.status().as_u16().to_string();

    if path != "/metrics" {
        HTTP_COUNTER.with_label_values(&[&method, &path, &status]).inc();
        HTTP_DURATION.with_label_values(&[&method, &path]).observe(start.elapsed().as_secs_f64());
    }

    response
}