use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid state: {0}")]
    InvalidState(String),
}

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub details: Option<String>,
}

impl CommandError {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        details: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details,
        }
    }
}

impl From<AppError> for CommandError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Database(error) => Self::new(
                "database_error",
                "Greška pri radu sa lokalnom bazom podataka.",
                Some(error.to_string()),
            ),
            AppError::Io(error) => Self::new(
                "file_error",
                "Greška pri pristupu lokalnim fajlovima.",
                Some(error.to_string()),
            ),
            AppError::InvalidState(details) => Self::new(
                "invalid_state",
                "Aplikacija nije u ispravnom stanju za ovu operaciju.",
                Some(details),
            ),
        }
    }
}
