use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("File system error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{message}")]
    Validation {
        message: String,
        details: Option<serde_json::Value>,
    },

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    BackupFailed(String),

    #[error("{message}")]
    Business {
        code: &'static str,
        message: String,
        details: Option<serde_json::Value>,
    },

    #[error("{0}")]
    InvalidState(String),
}

impl AppError {
    pub fn validation(message: impl Into<String>, details: serde_json::Value) -> Self {
        Self::Validation {
            message: message.into(),
            details: Some(details),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn business(code: &'static str, message: impl Into<String>) -> Self {
        Self::Business {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn business_with_details(
        code: &'static str,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self::Business {
            code,
            message: message.into(),
            details: Some(details),
        }
    }

    #[cfg(test)]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(_) => "database_error",
            Self::Io(_) => "file_system_error",
            Self::Validation { .. } => "validation_error",
            Self::NotFound(_) => "not_found",
            Self::BackupFailed(_) => "backup_failed",
            Self::Business { code, .. } => code,
            Self::InvalidState(_) => "invalid_state",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }
}

impl From<AppError> for CommandError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Database(source) => {
                Self::new("database_error", format!("Greška baze podataka: {source}"))
            }
            AppError::Io(source) => Self::new(
                "file_system_error",
                format!("Greška fajl sistema: {source}"),
            ),
            AppError::Validation { message, details } => Self {
                code: "validation_error",
                message,
                details,
            },
            AppError::NotFound(message) => Self::new("not_found", message),
            AppError::BackupFailed(message) => Self::new("backup_failed", message),
            AppError::Business {
                code,
                message,
                details,
            } => Self {
                code,
                message,
                details,
            },
            AppError::InvalidState(message) => Self::new("invalid_state", message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CommandError;

    #[test]
    fn new_uses_static_code_and_json_details_shape() {
        let command_error = CommandError::new("invalid_state", "Operacija nije moguća.");

        let details: Option<serde_json::Value> = command_error.details;
        assert_eq!(command_error.code, "invalid_state");
        assert_eq!(command_error.message, "Operacija nije moguća.");
        assert_eq!(details, None);
    }
}
