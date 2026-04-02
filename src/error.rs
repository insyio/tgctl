use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("Validation error: {0}")]
    Validation(String),
}

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Failed to read state file: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse state file: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Chat not found: {0}")]
    ChatNotFound(String),

    #[error("Insufficient permissions: {0}")]
    PermissionDenied(String),

    #[error("API error: {0}")]
    Invocation(#[from] grammers_client::InvocationError),
}
