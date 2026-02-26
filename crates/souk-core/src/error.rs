use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SoukError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Semver error: {0}")]
    Semver(#[from] semver::Error),

    #[error("{0}")]
    Other(String),
}
