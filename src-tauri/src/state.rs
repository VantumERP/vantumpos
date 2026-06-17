use std::path::PathBuf;

use tauri::Manager;

use crate::app_error::AppError;
use crate::db::Db;

#[derive(Clone, Debug)]
pub struct AppState {
    db: Db,
}

impl AppState {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &Db {
        &self.db
    }
}

pub fn resolve_database_path(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    let app_data_dir = app.path().app_data_dir().map_err(|error| {
        AppError::InvalidState(format!("Ne mogu da pronadjem folder aplikacije: {error}"))
    })?;

    std::fs::create_dir_all(&app_data_dir)?;
    Ok(app_data_dir.join("vantumpos.sqlite3"))
}
