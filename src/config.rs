use serde::Deserialize;
use std::time::Duration;
use crate::path_matcher::RoutePattern;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct RouteRule {
    // 支持单个或多个前缀
    #[serde(with = "prefix_deserializer")]
    pub prefix: Vec<String>,
    // 支持单个或多个上游
    #[serde(with = "upstream_deserializer")]
    pub upstream: Vec<String>,
    // 负载均衡策略，默认为轮询
    #[serde(default = "default_strategy")]
    pub strategy: String,
}

// 默认负载均衡策略
fn default_strategy() -> String {
    "robin".to_string()
}

// 自定义反序列化器，支持字符串和数组两种格式
mod prefix_deserializer {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrVec {
            String(String),
            Vec(Vec<String>),
        }

        match StringOrVec::deserialize(deserializer)? {
            StringOrVec::String(s) => Ok(vec![s]),
            StringOrVec::Vec(v) => Ok(v),
        }
    }
}

mod upstream_deserializer {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrVec {
            String(String),
            Vec(Vec<String>),
        }

        match StringOrVec::deserialize(deserializer)? {
            StringOrVec::String(s) => Ok(vec![s]),
            StringOrVec::Vec(v) => Ok(v),
        }
    }
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
        // 检查任意一个前缀是否匹配
        for prefix in &self.prefix {
            if self.matches_prefix(prefix, path) {
                return true;
            }
        }
        false
    }

    fn matches_prefix(&self, prefix: &str, path: &str) -> bool {
        // 检查是否包含模式匹配字符
        if prefix.contains('{') || prefix.contains('*') || prefix.contains('?') {
            // 使用模式匹配
            match RoutePattern::from_pattern(prefix) {
                Ok(route_pattern) => route_pattern.matches(path),
                Err(_) => {
                    // 如果模式编译失败，回退到简单前缀匹配
                    path.starts_with(prefix)
                }
            }
        } else {
            // 传统前缀匹配：精确匹配或前缀匹配
            path == prefix || path.starts_with(&format!("{}/", prefix))
        }
    }

    pub fn extract_variables(&self, path: &str) -> HashMap<String, String> {
        // 找到匹配的前缀并提取变量
        for prefix in &self.prefix {
            if self.matches_prefix(prefix, path) {
                match RoutePattern::from_pattern(prefix) {
                    Ok(route_pattern) => return route_pattern.match_path(path).unwrap_or_default(),
                    Err(_) => return HashMap::new(),
                }
            }
        }
        HashMap::new()
    }

    // 校验配置
    pub fn validate(&self) -> Result<(), String> {
        if self.prefix.is_empty() {
            return Err("prefix不能为空".to_string());
        }
        for (i, p) in self.prefix.iter().enumerate() {
            if p.trim().is_empty() {
                return Err(format!("prefix[{}]不能为空", i));
            }
        }
        if self.upstream.is_empty() {
            return Err("upstream不能为空".to_string());
        }
        for (i, u) in self.upstream.iter().enumerate() {
            if u.trim().is_empty() {
                return Err(format!("upstream[{}]不能为空", i));
            }
        }
        
        // 校验负载均衡策略
        match self.strategy.as_str() {
            "robin" | "random" | "iphash" => Ok(()),
            _ => Err(format!("不支持的负载均衡策略: {}", self.strategy)),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_rule_matching() {
        let routes = vec![
            RouteRule { 
                prefix: vec!["/user".to_string(), "/users".to_string()], 
                upstream: vec!["http://localhost:30000".to_string()],
                strategy: "robin".to_string(),
            },
            RouteRule { 
                prefix: vec!["/api/user/{id}".to_string()], 
                upstream: vec!["http://localhost:30001".to_string(), "http://localhost:30002".to_string()],
                strategy: "random".to_string(),
            },
        ];

        let test_cases = vec![
            ("/user", true, "30000"),
            ("/users", true, "30000"),
            ("/api/user/123", true, "30001或30002"),
        ];

        for (path, should_match, expected_upstream) in test_cases {
            let mut matched = false;
            for route in &routes {
                if route.matches(path) {
                    if route.upstream.len() == 1 {
                        assert_eq!(route.upstream[0], format!("http://localhost:{}", expected_upstream));
                    }
                    matched = true;
                    break;
                }
            }
            assert!(matched, "路径 {} 应该匹配某个路由", path);
        }
    }

    #[test]
    fn test_route_rule_validation() {
        let valid_route = RouteRule {
            prefix: vec!["/user".to_string()],
            upstream: vec!["http://localhost:30000".to_string()],
            strategy: "robin".to_string(),
        };
        assert!(valid_route.validate().is_ok());

        let invalid_prefix = RouteRule {
            prefix: vec![],
            upstream: vec!["http://localhost:30000".to_string()],
            strategy: "robin".to_string(),
        };
        assert!(invalid_prefix.validate().is_err());

        let invalid_upstream = RouteRule {
            prefix: vec!["/user".to_string()],
            upstream: vec![],
            strategy: "robin".to_string(),
        };
        assert!(invalid_upstream.validate().is_err());

        let invalid_strategy = RouteRule {
            prefix: vec!["/user".to_string()],
            upstream: vec!["http://localhost:30000".to_string()],
            strategy: "unknown".to_string(),
        };
        assert!(invalid_strategy.validate().is_err());
    }
}
