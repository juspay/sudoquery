// 1. Derive Error and use #[error] macros for Display formatting
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    // Automatically convert standard IO errors into your custom error type
    #[error("Disk I/O error occurred")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error occured")]
    Json(#[from] serde_json::Error),

    #[error("Kafka error occurred")]
    Kafka(#[from] rdkafka::error::KafkaError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Superposition provider error: {0}")]
    Superposition(#[from] superposition_provider::SuperpositionError),

    #[error("Missing or malformed authorization credentials")]
    MissingCredentials,

    #[error("Invalid bearer token")]
    InvalidToken,

    /// `collector_keys` could not be read (the authenticated endpoint's
    /// database is down or misconfigured).
    #[error("Collector keys unavailable: {0}")]
    KeyStore(String),

    /// The authenticated endpoint has no key store configured
    /// (`COLLECTOR_KEYS_DATABASE_URL` unset).
    #[error("The authenticated endpoint is not configured")]
    AuthNotConfigured,

    /// An event's `org_id` / `workspace_id` differ from the request's scope
    /// (its headers, or its key's).
    #[error("{0}")]
    ScopeMismatch(String),
}

// 2. Define your clean Result alias
pub type Result<T> = std::result::Result<T, AppError>;
