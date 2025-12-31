use thiserror::Error;

#[derive(Error, Debug)]
pub enum DynamicConfigError {
    #[error("failed to fetch config: {0}")]
    FetchError(String),

    #[error("failed to parse config JSON: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("config source returned empty content")]
    EmptyContent,

    #[error("configuration has not been loaded yet")]
    NotLoaded,

    #[error("invalid config location: {0}")]
    InvalidLocation(String),
}

pub type Result<T> = std::result::Result<T, DynamicConfigError>;
