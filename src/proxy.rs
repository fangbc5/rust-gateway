use axum::{
    body::{self, Body},
    extract::Request,
    http::Response,
    routing::any,
    Router,
    middleware,
};
use reqwest::Client;
use tracing::info;
use crate::config::Settings;
use crate::rate_limit::rate_limit_layer;

// 代理服务路由
pub fn router() -> Router {
    use crate::auth::JwtAuth;

    Router::new()
        .route("/*path", any(proxy_handler))
        .route_layer(middleware::from_extractor::<JwtAuth>())
        .layer(axum::middleware::from_fn(rate_limit_layer))
}



// 代理处理器
async fn proxy_handler(req: Request) -> Response<Body> {
    let settings = req
        .extensions()
        .get::<Settings>()
        .cloned();

    let upstream = settings
        .as_ref()
        .map(|s| s.upstream_default.as_str())
        .unwrap_or("http://httpbin.org");

    let client = Client::new();
    let uri = format!("{}{}", upstream, req.uri());

    info!("Proxying request -> {}", uri);

    // 转发请求
    let mut rb = client
        .request(req.method().clone(), &uri);

    if let Some(s) = &settings {
        rb = rb.timeout(s.request_timeout());
    }

    // 转发头
    for (name, value) in req.headers().iter() {
        if name == &axum::http::header::HOST {
            continue;
        }
        if let (Ok(n), Ok(v)) = (
            reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            rb = rb.header(n, v);
        }
    }

    // 转发 body
    let body_bytes = body::to_bytes(req.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    let resp = rb.body(body_bytes).send().await;

    match resp {
        Ok(r) => {
            let status = r.status();
            let headers = r.headers().clone();
            let bytes = r.bytes().await.unwrap_or_default();
            let mut builder = Response::builder().status(status);

            // 透传上游响应头
            for (name, value) in headers.iter() {
                if let (Some(n), Ok(v)) = (
                    axum::http::header::HeaderName::from_bytes(name.as_str().as_bytes()).ok(),
                    axum::http::header::HeaderValue::from_bytes(value.as_bytes()),
                ) {
                    // 不设置 hop-by-hop 头，可继续在此过滤
                    if n != axum::http::header::TRANSFER_ENCODING {
                        builder = builder.header(n, v);
                    }
                }
            }

            // 如果上游没提供 Content-Type，兜底一个
            if !builder.headers_ref().map(|h| h.contains_key(axum::http::header::CONTENT_TYPE)).unwrap_or(false) {
                builder = builder.header(axum::http::header::CONTENT_TYPE, "application/octet-stream");
            }

            builder.body(Body::from(bytes)).unwrap()
        }
        Err(err) => Response::builder()
            .status(500)
            .body(Body::from(format!("Proxy error: {}", err)))
            .unwrap(),
    }
}
