use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// Top-level error type for the Scutar engine. Backends and engine layers wrap
/// their own errors into these variants so the CLI sees a uniform shape.
#[derive(Debug, Error)]
pub enum Error {
    #[error("backend error: {0}")]
    Backend(String),

    #[error("object not found: {0}")]
    NotFound(String),

    #[error("invalid configuration: {0}")]
    Config(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(String),

    #[error("{0}")]
    Other(String),
}
