use axum::{
    body::Body,
    extract::Request,
    http::Response,
    middleware::Next,
};
use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::Arc;
use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    state::{keyed::DefaultKeyedStateStore, InMemoryState, NotKeyed},
};
use crate::config::Settings;

pub struct RateLimits {
    pub per_ip: RateLimiter<IpAddr, DefaultKeyedStateStore<IpAddr>, DefaultClock>,
    pub global: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
}

pub fn init_rate_limits(settings: &Settings) -> Arc<RateLimits> {
    let client_qps_nz = NonZeroU32::new(settings.client_qps).unwrap_or(NonZeroU32::new(1).unwrap());
    let global_qps_nz = NonZeroU32::new(settings.global_qps).unwrap_or(NonZeroU32::new(1).unwrap());
    let per_ip = RateLimiter::keyed(Quota::per_second(client_qps_nz));
    let global = RateLimiter::direct(Quota::per_second(global_qps_nz));
    Arc::new(RateLimits { per_ip, global })
}

pub async fn rate_limit_layer(req: Request, next: Next) -> Response<Body> {
    let limits = req
        .extensions()
        .get::<Arc<RateLimits>>()
        .cloned();

    if let Some(limits) = limits {
        if limits.global.check().is_err() {
            return Response::builder()
                .status(429)
                .body(Body::from("Too Many Requests (global)"))
                .unwrap();
        }

        let client_ip = req
            .extensions()
            .get::<axum::extract::ConnectInfo<IpAddr>>()
            .map(|ci| ci.0)
            .unwrap_or_else(|| "127.0.0.1".parse().unwrap());

        if limits.per_ip.check_key(&client_ip).is_err() {
            return Response::builder()
                .status(429)
                .body(Body::from("Too Many Requests (client)"))
                .unwrap();
        }
    }

    next.run(req).await
} 