use serde::{Deserialize, Serialize};
use specta::Type;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("task not found: {0}")]
    TaskNotFound(u32),
    #[error("cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

impl Error {
    fn kind(&self) -> &'static str {
        match self {
            Error::TaskNotFound(_) => "task_not_found",
            Error::Cancelled => "cancelled",
            Error::Io(_) => "io",
            Error::Other(_) => "error",
        }
    }
}

/// UI-facing error: `{ kind, message }`. Commands return `Result<T, ErrorDto>`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ErrorDto {
    pub kind: String,
    pub message: String,
}

impl From<Error> for ErrorDto {
    fn from(e: Error) -> Self {
        Self {
            kind: e.kind().into(),
            message: e.to_string(),
        }
    }
}
