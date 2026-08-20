use thiserror::Error;

#[derive(Debug, Error)]
pub enum AirPlayError {
    #[error("AirPlay discovery failed: {0}")]
    Discovery(String),

    #[error("Credential storage failed: {0}")]
    CredentialStore(String),

    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("Credential JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Binary plist is invalid: {0}")]
    Plist(#[from] plist::Error),

    #[error("RTSP protocol error: {0}")]
    Protocol(String),

    #[error("{0}")]
    Unsupported(&'static str),
}

pub type Result<T> = std::result::Result<T, AirPlayError>;
