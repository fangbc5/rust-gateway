use serde::Deserialize;
use std::time::Duration;
use crate::path_matcher::RoutePattern;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct RouteRule {
    pub prefix: String,
    pub upstream: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub gateway_bind: String,
    pub jwt_decoding_key: String,
    pub upstream_default: String,
    pub global_qps: u32,
    pub client_qps: u32,
    pub request_timeout_secs: Option<u64>,
}

impl Settings {
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout_secs.unwrap_or(10))
    }
}

// 增强的路径匹配器
impl RouteRule {
    pub fn matches(&self, path: &str) -> bool {
        // 直接使用prefix作为pattern进行匹配
        match RoutePattern::from_pattern(&self.prefix) {
            Ok(route_pattern) => route_pattern.matches(path),
            Err(_) => {
                // 如果模式编译失败，回退到简单前缀匹配
                path.starts_with(&self.prefix)
            }
        }
    }

    pub fn extract_variables(&self, path: &str) -> HashMap<String, String> {
        // 直接使用prefix作为pattern提取变量
        match RoutePattern::from_pattern(&self.prefix) {
            Ok(route_pattern) => route_pattern.match_path(path).unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    }

    // 校验配置
    pub fn validate(&self) -> Result<(), String> {
        if self.prefix.trim().is_empty() {
            return Err("prefix不能为空".to_string());
        }
        if self.upstream.trim().is_empty() {
            return Err("upstream不能为空".to_string());
        }
        Ok(())
    }
}

pub fn load_settings() -> Result<Settings, config::ConfigError> {
    let c = config::Config::builder()
        .add_source(config::File::with_name("config").required(false))
        .add_source(config::Environment::default());
    // also load .env
    dotenvy::dotenv().ok();
    let c = c.build()?;
    c.try_deserialize::<Settings>()
}

#[derive(Debug, Deserialize)]
struct RoutesFile { routes: Vec<RouteRule> }

pub fn load_route_rules() -> Result<Vec<RouteRule>, config::ConfigError> {
    // 固定使用 TOML 文件格式，文件名 routes.toml
    let c = config::Config::builder()
        .add_source(config::File::new("routes", config::FileFormat::Toml))
        .build()?;
    let rf: RoutesFile = c.try_deserialize()?;
    
    // 校验所有路由规则
    for (i, rule) in rf.routes.iter().enumerate() {
        if let Err(err) = rule.validate() {
            return Err(config::ConfigError::Message(format!(
                "路由规则 #{} 配置错误: {}", i + 1, err
            )));
        }
    }
    
    Ok(rf.routes)
}
