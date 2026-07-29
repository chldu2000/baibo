use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{
    domain::{
        terminal::{NewTerminalSession, TerminalId, TerminalSession, TerminalStatus},
        workspace::WorkspaceId,
    },
    services::terminal::TerminalError,
};

use super::Database;

const MAX_LOG_BYTES: i64 = 2 * 1024 * 1024;

#[derive(Clone)]
pub struct TerminalRepository {
    database: Database,
}

impl TerminalRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn list(&self, workspace_id: &WorkspaceId) -> Result<Vec<TerminalSession>, TerminalError> {
        let connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        let mut statement = connection
            .prepare(
                "SELECT id, workspace_id, title, shell, cwd, status, cols, rows,
                        created_at, started_at, ended_at, exit_code, termination_reason
                 FROM terminal_sessions
                 WHERE workspace_id = ?1
                 ORDER BY created_at DESC, id ASC",
            )
            .map_err(TerminalError::database)?;
        let result = statement
            .query_map(params![workspace_id.as_str()], map_session)
            .map_err(TerminalError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(TerminalError::database)?;
        Ok(result)
    }

    pub fn get_scoped(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
    ) -> Result<TerminalSession, TerminalError> {
        let connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        find_scoped(&connection, workspace_id, terminal_id)?
            .ok_or_else(|| TerminalError::not_found(terminal_id))
    }

    pub fn create(
        &self,
        mut terminal: NewTerminalSession,
    ) -> Result<TerminalSession, TerminalError> {
        let mut connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(TerminalError::database)?;
        if terminal.auto_title {
            let ordinal: i64 = transaction
                .query_row(
                    "SELECT COALESCE(MAX(CAST(SUBSTR(title, 7) AS INTEGER)), 0) + 1
                     FROM terminal_sessions
                     WHERE workspace_id = ?1 AND title GLOB 'Shell [0-9]*'",
                    params![terminal.workspace_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(TerminalError::database)?;
            terminal.title = format!("Shell {ordinal}");
        }
        transaction
            .execute(
                "INSERT INTO terminal_sessions (
                    id, workspace_id, title, shell, cwd, status, cols, rows, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'starting', ?6, ?7, ?8)",
                params![
                    terminal.id.as_str(),
                    terminal.workspace_id.as_str(),
                    terminal.title,
                    terminal.shell,
                    terminal.cwd,
                    terminal.cols,
                    terminal.rows,
                    terminal.now,
                ],
            )
            .map_err(TerminalError::database)?;
        let result = find_scoped(&transaction, &terminal.workspace_id, &terminal.id)?
            .ok_or_else(|| TerminalError::not_found(&terminal.id))?;
        transaction.commit().map_err(TerminalError::database)?;
        Ok(result)
    }

    pub fn mark_running(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
        now: i64,
    ) -> Result<TerminalSession, TerminalError> {
        self.update_status(
            workspace_id,
            terminal_id,
            TerminalStatus::Running,
            Some(now),
            None,
            None,
        )
    }

    pub fn finish(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
        status: TerminalStatus,
        exit_code: Option<i32>,
        reason: &str,
        now: i64,
    ) -> Result<TerminalSession, TerminalError> {
        self.update_status(
            workspace_id,
            terminal_id,
            status,
            None,
            Some((now, exit_code)),
            Some(reason),
        )
    }

    fn update_status(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
        status: TerminalStatus,
        started_at: Option<i64>,
        ended: Option<(i64, Option<i32>)>,
        reason: Option<&str>,
    ) -> Result<TerminalSession, TerminalError> {
        let mut connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(TerminalError::database)?;
        let changed = transaction
            .execute(
                "UPDATE terminal_sessions
                 SET status = ?1,
                     started_at = COALESCE(?2, started_at),
                     ended_at = COALESCE(?3, ended_at),
                     exit_code = CASE WHEN ?3 IS NULL THEN exit_code ELSE ?4 END,
                     termination_reason = COALESCE(?5, termination_reason)
                 WHERE id = ?6 AND workspace_id = ?7",
                params![
                    status.as_str(),
                    started_at,
                    ended.map(|value| value.0),
                    ended.and_then(|value| value.1),
                    reason,
                    terminal_id.as_str(),
                    workspace_id.as_str(),
                ],
            )
            .map_err(TerminalError::database)?;
        ensure_changed(changed, terminal_id)?;
        let result = find_scoped(&transaction, workspace_id, terminal_id)?
            .ok_or_else(|| TerminalError::not_found(terminal_id))?;
        transaction.commit().map_err(TerminalError::database)?;
        Ok(result)
    }

    pub fn resize(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalSession, TerminalError> {
        let connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        let changed = connection
            .execute(
                "UPDATE terminal_sessions SET cols = ?1, rows = ?2
                 WHERE id = ?3 AND workspace_id = ?4",
                params![cols, rows, terminal_id.as_str(), workspace_id.as_str()],
            )
            .map_err(TerminalError::database)?;
        ensure_changed(changed, terminal_id)?;
        find_scoped(&connection, workspace_id, terminal_id)?
            .ok_or_else(|| TerminalError::not_found(terminal_id))
    }

    pub fn append_log(
        &self,
        terminal_id: &TerminalId,
        data: &[u8],
        now: i64,
    ) -> Result<(), TerminalError> {
        if data.is_empty() {
            return Ok(());
        }
        let mut connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(TerminalError::database)?;
        let sequence: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) + 1
                 FROM terminal_log_chunks WHERE terminal_id = ?1",
                params![terminal_id.as_str()],
                |row| row.get(0),
            )
            .map_err(TerminalError::database)?;
        transaction
            .execute(
                "INSERT INTO terminal_log_chunks (
                    terminal_id, sequence, data, byte_length, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    terminal_id.as_str(),
                    sequence,
                    data,
                    i64::try_from(data.len()).unwrap_or(i64::MAX),
                    now,
                ],
            )
            .map_err(TerminalError::database)?;
        trim_log_to_limit(&transaction, terminal_id)?;
        transaction.commit().map_err(TerminalError::database)?;
        Ok(())
    }

    pub fn read_log(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
    ) -> Result<Vec<Vec<u8>>, TerminalError> {
        let connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        if find_scoped(&connection, workspace_id, terminal_id)?.is_none() {
            return Err(TerminalError::not_found(terminal_id));
        }
        let mut statement = connection
            .prepare(
                "SELECT data FROM terminal_log_chunks
                 WHERE terminal_id = ?1 ORDER BY sequence ASC",
            )
            .map_err(TerminalError::database)?;
        let result = statement
            .query_map(params![terminal_id.as_str()], |row| row.get(0))
            .map_err(TerminalError::database)?
            .collect::<Result<Vec<Vec<u8>>, _>>()
            .map_err(TerminalError::database)?;
        Ok(result)
    }

    pub fn delete(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
    ) -> Result<(), TerminalError> {
        let connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        let session = find_scoped(&connection, workspace_id, terminal_id)?
            .ok_or_else(|| TerminalError::not_found(terminal_id))?;
        if matches!(
            session.status,
            TerminalStatus::Starting | TerminalStatus::Running
        ) {
            return Err(TerminalError::StillRunning);
        }
        let changed = connection
            .execute(
                "DELETE FROM terminal_sessions WHERE id = ?1 AND workspace_id = ?2",
                params![terminal_id.as_str(), workspace_id.as_str()],
            )
            .map_err(TerminalError::database)?;
        ensure_changed(changed, terminal_id)
    }

    pub fn has_live(&self, workspace_id: &WorkspaceId) -> Result<bool, TerminalError> {
        let connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM terminal_sessions
                    WHERE workspace_id = ?1 AND status IN ('starting', 'running')
                 )",
                params![workspace_id.as_str()],
                |row| row.get(0),
            )
            .map_err(TerminalError::database)
    }

    pub fn recover_interrupted(&self, now: i64) -> Result<usize, TerminalError> {
        let connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        connection
            .execute(
                "UPDATE terminal_sessions
                 SET status = 'interrupted', ended_at = ?1,
                     termination_reason = 'app_restart'
                 WHERE status IN ('starting', 'running')",
                params![now],
            )
            .map_err(TerminalError::database)
    }
}

fn trim_log_to_limit(
    connection: &Connection,
    terminal_id: &TerminalId,
) -> Result<(), TerminalError> {
    loop {
        let total: i64 = connection
            .query_row(
                "SELECT COALESCE(SUM(byte_length), 0)
                 FROM terminal_log_chunks WHERE terminal_id = ?1",
                params![terminal_id.as_str()],
                |row| row.get(0),
            )
            .map_err(TerminalError::database)?;
        if total <= MAX_LOG_BYTES {
            return Ok(());
        }
        let changed = connection
            .execute(
                "DELETE FROM terminal_log_chunks
                 WHERE terminal_id = ?1 AND sequence = (
                    SELECT MIN(sequence) FROM terminal_log_chunks WHERE terminal_id = ?1
                 )",
                params![terminal_id.as_str()],
            )
            .map_err(TerminalError::database)?;
        if changed == 0 {
            return Ok(());
        }
    }
}

fn find_scoped(
    connection: &Connection,
    workspace_id: &WorkspaceId,
    terminal_id: &TerminalId,
) -> Result<Option<TerminalSession>, TerminalError> {
    connection
        .query_row(
            "SELECT id, workspace_id, title, shell, cwd, status, cols, rows,
                    created_at, started_at, ended_at, exit_code, termination_reason
             FROM terminal_sessions
             WHERE id = ?1 AND workspace_id = ?2",
            params![terminal_id.as_str(), workspace_id.as_str()],
            map_session,
        )
        .optional()
        .map_err(TerminalError::database)
}

fn map_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<TerminalSession> {
    let status: String = row.get(5)?;
    let status = TerminalStatus::try_from(status.as_str()).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            5,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
        )
    })?;
    Ok(TerminalSession {
        id: TerminalId::from(row.get::<_, String>(0)?),
        workspace_id: WorkspaceId::from(row.get::<_, String>(1)?),
        title: row.get(2)?,
        shell: row.get(3)?,
        cwd: row.get(4)?,
        status,
        cols: row.get(6)?,
        rows: row.get(7)?,
        created_at: row.get(8)?,
        started_at: row.get(9)?,
        ended_at: row.get(10)?,
        exit_code: row.get(11)?,
        termination_reason: row.get(12)?,
    })
}

fn ensure_changed(changed: usize, terminal_id: &TerminalId) -> Result<(), TerminalError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(TerminalError::not_found(terminal_id))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use crate::{
        domain::terminal::{NewTerminalSession, TerminalId, TerminalStatus},
        persistence::{Database, WorkspaceRepository},
        services::workspace::WorkspaceService,
    };

    use super::{TerminalRepository, MAX_LOG_BYTES};

    struct Context {
        _temp: TempDir,
        repository: TerminalRepository,
        workspace_id: crate::domain::workspace::WorkspaceId,
        workspace_path: std::path::PathBuf,
    }

    impl Context {
        fn new() -> Self {
            let temp = TempDir::new().expect("temp");
            let app_data = temp.path().join("app-data").join("baibo");
            let database = Database::open(&app_data.join("baibo.sqlite3")).expect("database");
            let workspace_path = temp.path().join("workspace");
            fs::create_dir(&workspace_path).expect("workspace");
            let workspace_service =
                WorkspaceService::new(WorkspaceRepository::new(database.clone()), app_data);
            let snapshot = workspace_service
                .register_path(&workspace_path)
                .expect("register workspace");
            Self {
                _temp: temp,
                repository: TerminalRepository::new(database),
                workspace_id: snapshot.active_workspace_id.expect("active"),
                workspace_path,
            }
        }

        fn create(&self, name: &str) -> crate::domain::terminal::TerminalSession {
            self.repository
                .create(NewTerminalSession {
                    id: TerminalId::new(),
                    workspace_id: self.workspace_id.clone(),
                    title: name.into(),
                    auto_title: true,
                    shell: "/bin/zsh".into(),
                    cwd: self.workspace_path.to_string_lossy().into_owned(),
                    cols: 80,
                    rows: 24,
                    now: 1,
                })
                .expect("create terminal")
        }
    }

    #[test]
    fn creates_scoped_sessions_and_rejects_cross_workspace_ids() {
        let context = Context::new();
        let terminal = context.create("ignored");
        let other_workspace = crate::domain::workspace::WorkspaceId::new();

        assert_eq!(terminal.title, "Shell 1");
        assert_eq!(
            context
                .repository
                .get_scoped(&other_workspace, &terminal.id)
                .expect_err("cross workspace")
                .code(),
            "terminal_not_found"
        );
    }

    #[test]
    fn recovers_live_sessions_without_relaunching() {
        let context = Context::new();
        let terminal = context.create("shell");
        context
            .repository
            .mark_running(&context.workspace_id, &terminal.id, 2)
            .expect("running");

        let recovered = context.repository.recover_interrupted(3).expect("recover");
        let session = context
            .repository
            .get_scoped(&context.workspace_id, &terminal.id)
            .expect("session");

        assert_eq!(recovered, 1);
        assert_eq!(session.status, TerminalStatus::Interrupted);
        assert_eq!(session.ended_at, Some(3));
    }

    #[test]
    fn retains_log_order_and_enforces_the_byte_limit() {
        let context = Context::new();
        let terminal = context.create("shell");
        let chunk = vec![b'x'; 16 * 1024];
        for index in 0..140 {
            let mut data = chunk.clone();
            data[0] = u8::try_from(index % 255).expect("byte");
            context
                .repository
                .append_log(&terminal.id, &data, index)
                .expect("append");
        }
        let chunks = context
            .repository
            .read_log(&context.workspace_id, &terminal.id)
            .expect("read");
        let retained: usize = chunks.iter().map(Vec::len).sum();

        assert!(retained <= usize::try_from(MAX_LOG_BYTES).expect("limit"));
        assert_eq!(chunks.last().expect("last")[0], 139);
        assert!(chunks.first().expect("first")[0] > 0);
    }

    #[test]
    fn refuses_to_delete_live_sessions_and_cascades_logs_for_finished_ones() {
        let context = Context::new();
        let terminal = context.create("shell");
        context
            .repository
            .append_log(&terminal.id, b"kept workspace file", 1)
            .expect("log");
        assert_eq!(
            context
                .repository
                .delete(&context.workspace_id, &terminal.id)
                .expect_err("live")
                .code(),
            "terminal_still_running"
        );
        context
            .repository
            .finish(
                &context.workspace_id,
                &terminal.id,
                TerminalStatus::Stopped,
                None,
                "test",
                2,
            )
            .expect("finish");
        context
            .repository
            .delete(&context.workspace_id, &terminal.id)
            .expect("delete");

        assert!(context.workspace_path.exists());
        assert_eq!(
            context
                .repository
                .get_scoped(&context.workspace_id, &terminal.id)
                .expect_err("deleted")
                .code(),
            "terminal_not_found"
        );
    }
}
