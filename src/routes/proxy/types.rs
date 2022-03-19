use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::{sync::Mutex, time::Instant};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct ProxyURL {
    pub url: String,
    pub ttl: u64,
}

#[derive(Serialize)]
pub struct ProxyID {
    pub id: Uuid,
}

#[derive(Debug, Clone)]
pub struct Proxy {
    pub url: String,
    pub valid_until: Instant,
}

#[derive(Debug)]
pub struct Proxies(pub Mutex<HashMap<Uuid, Proxy>>);

impl Proxies {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}

impl Default for Proxies {
    fn default() -> Self {
        Self::new()
    }
}
