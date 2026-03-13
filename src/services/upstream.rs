use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use hickory_proto::{
    op::Message,
    rr::{RData, RecordType},
};
use tokio::{net::UdpSocket, time::timeout};
use tracing::{debug, trace, warn};

use crate::{
    core::{DnsPacket, cache::Cache},
    utils::{AppError, Result},
};

const UPSTREAM_DNS: &str = "1.1.1.1:53"; // Cloudflare's primary DNS
const TIMEOUT_SECS: u64 = 2;

pub async fn resolve(
    request: &Message,
    raw_query: &[u8],
    domain: &str,
    cache: Arc<Cache>,
) -> Result<Vec<u8>> {
    if let Some(cached_ip) = cache.get(domain) {
        let query_type = request
            .queries()
            .first()
            .map(|q| q.query_type())
            .unwrap_or(hickory_proto::rr::RecordType::A);

        let is_match = matches!(
            (query_type, cached_ip),
            (RecordType::A, IpAddr::V4(_)) | (RecordType::AAAA, IpAddr::V6(_))
        );

        if is_match {
            trace!("Cache HIT for {} -> {}", domain, cached_ip);

            if let Ok(response_bytes) = DnsPacket::build_cached_response(request, cached_ip) {
                return Ok(response_bytes);
            }
        } else {
            debug!(
                "Cache type miss: {} asked for {:?} but cache holds {}",
                domain, query_type, cached_ip
            );
        }
    }

    trace!(
        "Cache MISS for {} - querying upstream {}",
        domain, UPSTREAM_DNS
    );

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let upstream_addr: SocketAddr = UPSTREAM_DNS.parse().expect("Invalid upstream IP");

    socket.send_to(raw_query, upstream_addr).await?;

    let mut buf = [0u8; 4096];

    let result = timeout(
        Duration::from_secs(TIMEOUT_SECS),
        socket.recv_from(&mut buf),
    )
    .await;

    match result {
        Ok(Ok((size, _))) => {
            let response_bytes = buf[..size].to_vec();

            if let Ok(response_msg) = DnsPacket::parse(&response_bytes) {
                cache_upstream_response(domain, &response_msg, &cache);
            }

            Ok(response_bytes)
        }
        Ok(Err(e)) => {
            warn!("Network error receiving from upstream: {}", e);
            Err(AppError::Io(e))
        }
        Err(_) => {
            warn!("Upstream query timied out for {}", domain);
            Err(AppError::Blocklist("Upstream timeout".to_string()))
        }
    }
}

fn cache_upstream_response(domain: &str, response: &Message, cache: &Arc<Cache>) {
    for record in response.answers() {
        let ttl = record.ttl() as u64;

        let data = record.data();

        let ip_addr = match data {
            RData::A(ipv4) => Some(IpAddr::V4((*ipv4).into())),
            RData::AAAA(ipv6) => Some(IpAddr::V6((*ipv6).into())),
            _ => None,
        };

        if let Some(ip) = ip_addr {
            debug!("Caching {} -> {} for {} seconds", domain, ip, ttl);
            cache.insert(domain.to_string(), ip, ttl);

            // break after first valid IP.
            // round-robin DNS might return multiple
            break;
        }
    }
}
