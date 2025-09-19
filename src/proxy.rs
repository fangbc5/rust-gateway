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

    // 从扩展获取路由前缀规则
    let route_rules = req
        .extensions()
        .get::<Vec<crate::config::RouteRule>>()
        .cloned();

    // 用于匹配与转发的路径（去除 /proxy 前缀）
    let full_path = req.uri().path();
    let match_path = full_path.strip_prefix("/proxy").unwrap_or(full_path);
    let query_suffix = req.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();

    let (upstream, forward_path, path_variables) = if let Some(s) = &settings {
        if let Some(rules) = route_rules.as_ref() {
            // 查找最佳匹配的路由规则
            if let Some(best_match) = find_best_match(rules, match_path) {
                let path_variables = best_match.extract_variables(match_path);
                
                // 计算转发路径
                let forward_path = if best_match.prefix.contains('{') || best_match.prefix.contains('*') || best_match.prefix.contains('?') {
                    // 如果prefix包含模式匹配字符，去掉匹配的部分
                    reconstruct_forward_path(match_path, &best_match.prefix, &path_variables)
                } else {
                    // 传统前缀匹配
                    match_path.strip_prefix(&best_match.prefix).unwrap_or(match_path).to_string()
                };
                
                (best_match.upstream.clone(), forward_path, path_variables)
            } else {
                // 未命中任何规则时回退到默认上游
                (s.upstream_default.clone(), match_path.to_string(), std::collections::HashMap::new())
            }
        } else {
            (s.upstream_default.clone(), match_path.to_string(), std::collections::HashMap::new())
        }
    } else {
        ("http://httpbin.org".to_string(), match_path.to_string(), std::collections::HashMap::new())
    };

    let client = Client::new();
    let uri = format!("{}{}{}", upstream, forward_path, query_suffix);

    info!("路径匹配: {} -> {} (转发到: {})", match_path, forward_path, upstream);
    if !path_variables.is_empty() {
        info!("提取的路径变量: {:?}", path_variables);
    }

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

// 查找最佳匹配的路由规则
fn find_best_match<'a>(rules: &'a [crate::config::RouteRule], path: &str) -> Option<&'a crate::config::RouteRule> {
    let mut best_match: Option<&'a crate::config::RouteRule> = None;
    let mut best_score = 0;

    for rule in rules {
        if rule.matches(path) {
            // 计算匹配分数：模式匹配 > 前缀匹配，更具体的模式优先
            let score = if rule.prefix.contains('{') || rule.prefix.contains('*') || rule.prefix.contains('?') {
                1000 + rule.prefix.len() as i32
            } else {
                rule.prefix.len() as i32
            };
            
            if score > best_score {
                best_score = score;
                best_match = Some(rule);
            }
        }
    }

    best_match
}

// 重构转发路径
fn reconstruct_forward_path(
    original_path: &str,
    prefix: &str,
    variables: &std::collections::HashMap<String, String>,
) -> String {
    // 如果原始路径以prefix开头，去掉prefix部分
    if original_path.starts_with(prefix) {
        original_path.strip_prefix(prefix).unwrap_or(original_path).to_string()
    } else {
        original_path.to_string()
    }
}

