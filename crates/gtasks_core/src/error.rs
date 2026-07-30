use std::fmt;

#[derive(Debug)]
pub enum GTasksError {
    Database(rusqlite::Error),
    Http(reqwest::Error),
    Auth(String),
    Io(std::io::Error),
    TaskNotFound(String),
    Other(String),
}

impl fmt::Display for GTasksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GTasksError::Database(err) => write!(f, "Database error: {err}"),
            GTasksError::Http(err) => write!(f, "HTTP API error: {err}"),
            GTasksError::Auth(msg) => write!(f, "Authentication error: {msg}"),
            GTasksError::Io(err) => write!(f, "I/O error: {err}"),
            GTasksError::TaskNotFound(id) => write!(f, "Task not found: {id}"),
            GTasksError::Other(msg) => write!(f, "Error: {msg}"),
        }
    }
}

impl std::error::Error for GTasksError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GTasksError::Database(err) => Some(err),
            GTasksError::Http(err) => Some(err),
            GTasksError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for GTasksError {
    fn from(err: rusqlite::Error) -> Self {
        GTasksError::Database(err)
    }
}

impl From<reqwest::Error> for GTasksError {
    fn from(err: reqwest::Error) -> Self {
        GTasksError::Http(err)
    }
}

impl From<std::io::Error> for GTasksError {
    fn from(err: std::io::Error) -> Self {
        GTasksError::Io(err)
    }
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
