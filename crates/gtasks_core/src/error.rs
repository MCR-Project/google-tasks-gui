use thiserror::Error;

#[derive(Error, Debug)]
pub enum GTasksError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("HTTP API error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Keyring error: {0}")]
    Keyring(#[from] keyring::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Task join error: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Task list not found: {0}")]
    TaskListNotFound(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Error: {0}")]
    Other(String),
}

impl From<&str> for GTasksError {
    fn from(msg: &str) -> Self {
        GTasksError::Other(msg.to_string())
    }
}

impl From<String> for GTasksError {
    fn from(msg: String) -> Self {
        GTasksError::Other(msg)
    }
}

pub type Result<T> = std::result::Result<T, GTasksError>;
