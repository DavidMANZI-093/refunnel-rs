use std::sync::Arc;

use tracing::{error, info};

use crate::{
    core::{Blocklist, cache::Cache},
    services::Server,
};

mod core;
mod services;
mod utils;

const BIND_ADDRESS: &str = "0.0.0.0:53";
const HOSTS_FILE: &str = "hosts.txt";
const CACHE_CAPACITY: usize = 10_000;

#[tokio::main]
async fn main() -> utils::Result<()> {
    utils::logger::init();
    info!("Starting DNS Sinkhole...");

    info!("Loading blocklist from {}...", HOSTS_FILE);
    let blocklist = match Blocklist::from_file(HOSTS_FILE) {
        Ok(bl) => Arc::new(bl),
        Err(e) => {
            error!(
                "Failed to load blocklist: {}. Make sure the file exists!",
                e
            );
            std::process::exit(1);
        }
    };

    info!("Initializing edge cache with capacity {}", CACHE_CAPACITY);
    let cache = Arc::new(Cache::new(CACHE_CAPACITY));

    let server = Server::new(BIND_ADDRESS, blocklist, cache).await?;

    server.run().await;

    Ok(())
}
