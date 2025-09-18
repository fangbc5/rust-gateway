use serde::Deserialize;
use std::time::Duration;

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

pub fn load_settings() -> Result<Settings, config::ConfigError> {
    let c = config::Config::builder()
        .add_source(config::File::with_name("config").required(false))
        .add_source(config::Environment::default());
    // also load .env
    dotenvy::dotenv().ok();
    let c = c.build()?;
    c.try_deserialize::<Settings>()
}
