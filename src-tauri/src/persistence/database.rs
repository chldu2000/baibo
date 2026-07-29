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
const DURABLE_SESSIONS_MIGRATION: &str = include_str!("../../migrations/0003_durable_sessions.sql");

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
        M::up(DURABLE_SESSIONS_MIGRATION),
    ])
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection, OptionalExtension};
    use tempfile::TempDir;

    use super::{configure, migrations, Database};

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
        let agent_table: Option<String> = connection
            .query_row(
                "SELECT name FROM sqlite_master
                 WHERE type = 'table' AND name = 'agent_sessions'",
                [],
                |row| row.get(0),
            )
            .optional()
            .expect("agent schema query");
        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal mode");
        let foreign_keys: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign keys");

        assert_eq!(table.as_deref(), Some("workspaces"));
        assert_eq!(terminal_table.as_deref(), Some("terminal_sessions"));
        assert_eq!(agent_table.as_deref(), Some("agent_sessions"));
        assert_eq!(journal_mode, "wal");
        assert_eq!(foreign_keys, 1);
    }

    #[test]
    fn upgrades_v2_records_without_guessing_agent_identity() {
        let temp = TempDir::new().expect("temp directory");
        let path = temp.path().join("baibo.sqlite3");
        let mut connection = Connection::open(&path).expect("connection");
        configure(&connection).expect("configure");
        migrations()
            .to_version(&mut connection, 2)
            .expect("migrate to v2");
        connection
            .execute(
                "INSERT INTO workspaces (
                    id, name, canonical_path, repository_root, git_repository,
                    created_at, last_opened_at
                 ) VALUES ('workspace', 'Workspace', '/tmp/workspace', NULL, 0, 1, 1)",
                [],
            )
            .expect("workspace");
        for (id, title, shell) in [
            ("shell", "Shell 1", "/bin/zsh"),
            ("legacy", "Codex 1", "/usr/local/bin/codex"),
        ] {
            connection
                .execute(
                    "INSERT INTO terminal_sessions (
                        id, workspace_id, title, shell, cwd, status, cols, rows, created_at
                     ) VALUES (?1, 'workspace', ?2, ?3, '/tmp/workspace',
                               'exited', 80, 24, 2)",
                    params![id, title, shell],
                )
                .expect("terminal");
        }
        connection
            .execute(
                "INSERT INTO terminal_log_chunks (
                    terminal_id, sequence, data, byte_length, created_at
                 ) VALUES ('legacy', 4, x'78', 1, 3)",
                [],
            )
            .expect("log");
        drop(connection);

        let database = Database::open(&path).expect("upgrade to v3");
        let connection = database.lock().expect("database lock");
        let shell_kind: String = connection
            .query_row(
                "SELECT session_kind FROM terminal_sessions WHERE id = 'shell'",
                [],
                |row| row.get(0),
            )
            .expect("shell kind");
        let legacy_kind: String = connection
            .query_row(
                "SELECT session_kind FROM terminal_sessions WHERE id = 'legacy'",
                [],
                |row| row.get(0),
            )
            .expect("legacy kind");
        let agent_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM agent_sessions", [], |row| row.get(0))
            .expect("agent count");
        let log_index: (i64, i64, String) = connection
            .query_row(
                "SELECT next_sequence, retained_bytes, coverage
                 FROM terminal_log_index WHERE terminal_id = 'legacy'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("log index");

        assert_eq!(shell_kind, "shell");
        assert_eq!(legacy_kind, "legacy");
        assert_eq!(agent_count, 0);
        assert_eq!(log_index, (5, 1, "unknown".into()));
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
