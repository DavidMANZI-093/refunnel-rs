use std::{net::SocketAddr, sync::Arc};

use tokio::net::UdpSocket;
use tracing::{debug, error, info, trace};

use crate::{
    core::{Blocklist, DnsPacket, cache::Cache},
    services::upstream::resolve,
    utils::Result,
};

pub struct Server {
    blocklist: Arc<Blocklist>,
    cache: Arc<Cache>,
    socket: Arc<UdpSocket>,
}

impl Server {
    pub async fn new(addr: &str, blocklist: Arc<Blocklist>, cache: Arc<Cache>) -> Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        let socket = Arc::new(socket);

        info!("DNS Sinkhole listening on udp://{}", addr);

        Ok(Self {
            blocklist,
            cache,
            socket,
        })
    }

    pub async fn run(&self) {
        let mut buf = [0u8; 4096];

        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((size, peer_addr)) => {
                    trace!("Received {} bytes from {}", size, peer_addr);

                    let payload = buf[..size].to_vec();

                    let blocklist = Arc::clone(&self.blocklist);
                    let cache = Arc::clone(&self.cache);
                    let socket = Arc::clone(&self.socket);

                    tokio::spawn(async move {
                        if let Err(e) =
                            Self::handle_request(payload, peer_addr, blocklist, cache, socket).await
                        {
                            error!("Error handling request from {}: {}", peer_addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to receive UDP packet: {}", e);
                }
            }
        }
    }

    async fn handle_request(
        payload: Vec<u8>,
        peer_addr: SocketAddr,
        blocklist: Arc<Blocklist>,
        cache: Arc<Cache>,
        socket: Arc<UdpSocket>,
    ) -> Result<()> {
        let message = DnsPacket::parse(&payload)?;

        let domain = match DnsPacket::extract_domain(&message) {
            Some(d) => d,
            None => {
                debug!("Received DNS query with no valid domain name.");
                return Ok(());
            }
        };

        if blocklist.is_blocked(&domain) {
            info!("BLOCKED: {}", domain);

            let response_bytes = DnsPacket::build_sinkhole(&message)?;

            socket.send_to(&response_bytes, peer_addr).await?;
            return Ok(());
        }

        trace!("ALLOWED: {} (Routing upstream...)", domain);

        match resolve(&message, &payload, &domain, cache).await {
            Ok(response_bytes) => {
                socket.send_to(&response_bytes, peer_addr).await?;
            }
            Err(e) => {
                error!("Failed to resolve {}: {}", domain, e);
            }
        }

        Ok(())
    }
}
