use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("File system error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    InvalidState(String),
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
                Self::new("database_error", format!("Greska baze podataka: {source}"))
            }
            AppError::Io(source) => Self::new(
                "file_system_error",
                format!("Greska fajl sistema: {source}"),
            ),
            AppError::InvalidState(message) => Self::new("invalid_state", message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CommandError;

    #[test]
    fn new_uses_static_code_and_json_details_shape() {
        let command_error = CommandError::new("invalid_state", "Operacija nije moguca.");

        let details: Option<serde_json::Value> = command_error.details;
        assert_eq!(command_error.code, "invalid_state");
        assert_eq!(command_error.message, "Operacija nije moguca.");
        assert_eq!(details, None);
    }
}
