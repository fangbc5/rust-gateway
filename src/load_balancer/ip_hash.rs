use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::collections::hash_map::DefaultHasher;
use arc_swap::ArcSwap;
use std::sync::Arc;
use crate::load_balancer::LoadBalancer;

/// 负载均衡器状态（不可变对象）
#[derive(Debug)]
struct BalancerState {
    hash_ring: BTreeMap<u64, String>,
    upstreams: Vec<String>,
    virtual_nodes: usize,
}

impl BalancerState {
    fn build(upstreams: Vec<String>, virtual_nodes: usize) -> Self {
        let mut hash_ring = BTreeMap::new();

        for upstream in &upstreams {
            for i in 0..virtual_nodes {
                let key = format!("{}#{}", upstream, i);
                let hash = Self::hash(&key);
                hash_ring.insert(hash, upstream.clone());
            }
        }

        Self {
            hash_ring,
            upstreams,
            virtual_nodes,
        }
    }

    fn hash(key: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    fn find_upstream(&self, hash: u64) -> Option<String> {
        if self.hash_ring.is_empty() {
            return None;
        }

        if let Some((_, upstream)) = self.hash_ring.range(hash..).next() {
            return Some(upstream.clone());
        }

        // 环回到第一个节点
        self.hash_ring.iter().next().map(|(_, v)| v.clone())
    }
}

/// 无锁 IP 哈希负载均衡器
#[derive(Debug)]
pub struct IpHashBalancer {
    state: ArcSwap<BalancerState>,
}

impl IpHashBalancer {
    pub fn new(upstreams: Vec<String>) -> Self {
        let state = BalancerState::build(upstreams, 150); // 每个节点 150 个虚拟节点
        Self {
            state: ArcSwap::from_pointee(state),
        }
    }

    /// 根据客户端 IP 选择 upstream
    pub fn select(&self, client_ip: Option<&SocketAddr>) -> Option<String> {
        let state = self.state.load();
        let ip_str = match client_ip {
            Some(addr) => addr.ip().to_string(),
            None => "127.0.0.1".to_string(),
        };
        let hash = BalancerState::hash(&ip_str);
        state.find_upstream(hash)
    }

    /// 更新所有 upstreams
    pub fn update_upstreams(&self, new_upstreams: Vec<String>) {
        let new_state = BalancerState::build(new_upstreams, self.state.load().virtual_nodes);
        self.state.store(Arc::new(new_state));
    }

    /// 添加一个 upstream
    pub fn add_upstream(&self, upstream: String) {
        let mut new_list = self.state.load().upstreams.clone();
        if !new_list.contains(&upstream) {
            new_list.push(upstream);
            self.update_upstreams(new_list);
        }
    }

    /// 删除一个 upstream
    pub fn remove_upstream(&self, upstream: &str) {
        let new_list: Vec<String> = self.state.load().upstreams
            .iter()
            .filter(|u| u.as_str() != upstream)
            .cloned()
            .collect();
        self.update_upstreams(new_list);
    }

    /// 获取当前 upstreams
    pub fn get_upstreams(&self) -> Vec<String> {
        self.state.load().upstreams.clone()
    }
}

impl LoadBalancer for IpHashBalancer {
    fn select(&self, client_ip: Option<&SocketAddr>) -> Option<String> {
        self.select(client_ip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consistent_hashing() {
        let balancer = IpHashBalancer::new(vec![
            "http://localhost:30000".to_string(),
            "http://localhost:30001".to_string(),
            "http://localhost:30002".to_string(),
        ]);

        // 同一个IP应该总是选择同一个upstream
        let ip1 = std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let result1 = balancer.select(Some(&ip1));
        let result2 = balancer.select(Some(&ip1));
        assert_eq!(result1, result2);

        // 不同IP可能选择不同的upstream
        let ip2 = std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 2)), 8080);
        let result3 = balancer.select(Some(&ip2));
        // 结果可能相同也可能不同，但不应该panic
        assert!(result3.is_some());
    }

    #[test]
    fn test_dynamic_update() {
        let balancer = IpHashBalancer::new(vec![
            "http://localhost:30000".to_string(),
        ]);

        let ip = std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let _original = balancer.select(Some(&ip));

        balancer.update_upstreams(vec![
            "http://localhost:30001".to_string(),
            "http://localhost:30002".to_string(),
        ]);

        let updated = balancer.select(Some(&ip));
        // 更新后应该仍然能选择到upstream
        assert!(updated.is_some());
    }
} 