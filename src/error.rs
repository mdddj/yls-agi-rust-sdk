use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("url parse error: {0}")]
    Url(#[from] url::ParseError),
    #[error("stream parse error: {0}")]
    Stream(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("missing environment variable: {0}")]
    MissingEnvVar(&'static str),
    #[error("unsupported configuration: {0}")]
    UnsupportedConfig(String),
    #[error("task join error: {0}")]
    Join(#[from] tokio::task::JoinError),
}
