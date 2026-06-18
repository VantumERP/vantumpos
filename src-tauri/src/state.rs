use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::Manager;

use crate::app_error::AppError;
use crate::db::Db;

#[derive(Clone, Debug)]
pub struct AppState {
    db: Db,
    session_user_id: Arc<Mutex<Option<i64>>>,
}

impl AppState {
    pub fn new(db: Db) -> Self {
        Self {
            db,
            session_user_id: Arc::new(Mutex::new(None)),
        }
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub fn session_user_id(&self) -> Result<Option<i64>, AppError> {
        self.session_user_id
            .lock()
            .map(|guard| *guard)
            .map_err(|_| AppError::InvalidState("Sesija nije dostupna.".to_string()))
    }

    pub fn set_session_user_id(&self, user_id: i64) -> Result<(), AppError> {
        let mut guard = self
            .session_user_id
            .lock()
            .map_err(|_| AppError::InvalidState("Sesija nije dostupna.".to_string()))?;
        *guard = Some(user_id);
        Ok(())
    }

    pub fn clear_session(&self) -> Result<(), AppError> {
        let mut guard = self
            .session_user_id
            .lock()
            .map_err(|_| AppError::InvalidState("Sesija nije dostupna.".to_string()))?;
        *guard = None;
        Ok(())
    }
}

pub fn resolve_database_path(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    let app_data_dir = app.path().app_data_dir().map_err(|error| {
        AppError::InvalidState(format!("Ne mogu da pronadjem folder aplikacije: {error}"))
    })?;

    std::fs::create_dir_all(&app_data_dir)?;
    Ok(app_data_dir.join("vantumpos.sqlite3"))
}
