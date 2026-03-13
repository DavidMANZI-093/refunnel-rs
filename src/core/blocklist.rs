use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use ahash::AHashSet;
use tracing::info;

use crate::utils::{AppError, Result};

pub struct Blocklist {
    domains: AHashSet<String>,
}

impl Blocklist {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(&path).map_err(AppError::Io)?;

        let reader = BufReader::new(file);
        let mut domains = AHashSet::new();

        for line in reader.lines() {
            let line = line.map_err(AppError::Io)?;
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some(domain) = trimmed.split_whitespace().last()
                && domain != "localhost"
            {
                domains.insert(domain.to_lowercase());
            }
        }

        info!(
            "Blocklist initialized. Loaded {} domains into memory.",
            domains.len()
        );

        Ok(Self { domains })
    }

    pub fn is_blocked(&self, domain: &str) -> bool {
        self.domains.contains(domain)
    }
}
