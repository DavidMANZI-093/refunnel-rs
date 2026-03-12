use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Network I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("DNS protocol error: {0}")]
    Dns(#[from] hickory_proto::ProtoError),

    #[error("Failed to load blocklist: {0}")]
    Blocklist(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
