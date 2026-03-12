use std::{
    net::IpAddr,
    num::NonZeroUsize,
    sync::RwLock,
    time::{Duration, Instant},
};

use lru::LruCache;
use tracing::{debug, trace};

#[derive(Debug, Clone)]
pub struct CachedRecord {
    pub ip: IpAddr,
    pub expires_at: Instant,
}

pub struct Cache {
    inner: RwLock<LruCache<String, CachedRecord>>,
}

impl Cache {
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).expect("Cache capacity must be non-zero");
        Self {
            inner: RwLock::new(LruCache::new(cap)),
        }
    }

    pub fn get(&self, domain: &str) -> Option<IpAddr> {
        let mut cache = self.inner.write().expect("RwLock poisoned");

        if let Some(record) = cache.get(domain) {
            if Instant::now() < record.expires_at {
                trace!("Cache hit for {}", domain);
                return Some(record.ip);
            } else {
                debug!("Cache expired for {}", domain);
                return None;
            }
        }

        None
    }

    pub fn insert(&self, domain: String, ip: IpAddr, ttl_seconds: u64) {
        let expires_at = Instant::now() + Duration::from_secs(ttl_seconds);
        let record = CachedRecord { ip, expires_at };

        let mut cache = self.inner.write().expect("RwLock poisoned");
        cache.put(domain, record);
    }
}
