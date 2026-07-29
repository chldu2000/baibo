use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

use crate::services::workspace::WorkspaceError;

const WORKSPACE_REGISTRY_MIGRATION: &str =
    include_str!("../../migrations/0001_workspace_registry.sql");
const TERMINAL_RUNTIME_MIGRATION: &str = include_str!("../../migrations/0002_terminal_runtime.sql");

#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, WorkspaceError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                WorkspaceError::database(format!("无法创建应用数据目录：{error}"))
            })?;
        }

        let mut connection = Connection::open(path).map_err(|error| {
            WorkspaceError::database(format!("无法打开工作空间数据库：{error}"))
        })?;
        configure(&connection)?;
        migrations()
            .to_latest(&mut connection)
            .map_err(|error| WorkspaceError::migration(error.to_string()))?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, Connection>, WorkspaceError> {
        self.connection
            .lock()
            .map_err(|_| WorkspaceError::database("工作空间数据库锁已损坏".into()))
    }
}

fn configure(connection: &Connection) -> Result<(), WorkspaceError> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| WorkspaceError::database(error.to_string()))?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )
        .map_err(|error| WorkspaceError::database(error.to_string()))
}

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(WORKSPACE_REGISTRY_MIGRATION),
        M::up(TERMINAL_RUNTIME_MIGRATION),
    ])
}

#[cfg(test)]
mod tests {
    use rusqlite::OptionalExtension;
    use tempfile::TempDir;

    use super::Database;

    #[test]
    fn migrates_a_new_database_and_reopens_idempotently() {
        let temp = TempDir::new().expect("temp directory");
        let path = temp.path().join("baibo.sqlite3");

        let database = Database::open(&path).expect("first open");
        drop(database);
        let database = Database::open(&path).expect("second open");
        let connection = database.lock().expect("database lock");

        let table: Option<String> = connection
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'workspaces'",
                [],
                |row| row.get(0),
            )
            .optional()
            .expect("schema query");
        let terminal_table: Option<String> = connection
            .query_row(
                "SELECT name FROM sqlite_master
                 WHERE type = 'table' AND name = 'terminal_sessions'",
                [],
                |row| row.get(0),
            )
            .optional()
            .expect("terminal schema query");
        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal mode");
        let foreign_keys: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign keys");

        assert_eq!(table.as_deref(), Some("workspaces"));
        assert_eq!(terminal_table.as_deref(), Some("terminal_sessions"));
        assert_eq!(journal_mode, "wal");
        assert_eq!(foreign_keys, 1);
    }

    #[test]
    fn does_not_replace_an_invalid_existing_database() {
        let temp = TempDir::new().expect("temp directory");
        let path = temp.path().join("baibo.sqlite3");
        std::fs::write(&path, b"not a sqlite database").expect("invalid database");

        let error = match Database::open(&path) {
            Ok(_) => panic!("invalid database must fail"),
            Err(error) => error,
        };
        let bytes = std::fs::read(&path).expect("database remains");

        assert_eq!(error.code(), "database_unavailable");
        assert_eq!(bytes, b"not a sqlite database");
    }
}
