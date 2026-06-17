mod migrations;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::Connection;

use crate::app_error::AppError;

#[derive(Clone, Debug)]
pub struct Db {
    path: Arc<PathBuf>,
}

impl Db {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let path = path.as_ref().to_path_buf();
        ensure_parent_directory(&path)?;
        let db = Self {
            path: Arc::new(path),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn open(&self) -> Result<Connection, AppError> {
        let connection = Connection::open(self.path.as_path())?;
        connection.execute_batch(
            r#"
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
"#,
        )?;
        Ok(connection)
    }

    pub fn migrate(&self) -> Result<(), AppError> {
        let mut connection = self.open()?;
        migrations::run_migrations(&mut connection)
    }
}

fn ensure_parent_directory(path: &Path) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    Ok(())
}

#[cfg(test)]
pub fn test_database_path(test_name: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!("vantumpos-{test_name}-{unique}.sqlite3"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_initializes_and_migrates_empty_database() {
        let path = test_database_path("new_initializes_and_migrates_empty_database");

        let db = Db::new(&path).expect("database should initialize");
        let connection = db.open().expect("database should open");
        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('settings', 'users', 'products', 'sales')",
                [],
                |row| row.get(0),
            )
            .expect("table count should query");

        assert_eq!(table_count, 4);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn migrate_is_idempotent_and_records_initial_migration_once() {
        let path = test_database_path("migrate_is_idempotent_and_records_initial_migration_once");

        let db = Db::new(&path).expect("database should initialize");
        db.migrate().expect("second migration should succeed");

        let connection = db.open().expect("database should open");
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .expect("migration count should query");

        assert_eq!(migration_count, 1);

        let _ = std::fs::remove_file(path);
    }
}
