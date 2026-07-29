use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{
    domain::{
        agent::NewAgentSession,
        terminal::{
            LifecycleEventKind, NewTerminalSession, SessionKind, SessionLifecycleEvent, TerminalId,
            TerminalLogCoverage, TerminalLogIndex, TerminalSession, TerminalStatus,
        },
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
                        created_at, started_at, ended_at, exit_code, termination_reason,
                        session_kind
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
                    id, workspace_id, title, shell, cwd, status, cols, rows, created_at,
                    session_kind
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'starting', ?6, ?7, ?8, ?9)",
                params![
                    terminal.id.as_str(),
                    terminal.workspace_id.as_str(),
                    terminal.title,
                    terminal.shell,
                    terminal.cwd,
                    terminal.cols,
                    terminal.rows,
                    terminal.now,
                    terminal.session_kind.as_str(),
                ],
            )
            .map_err(TerminalError::database)?;
        insert_initial_records(&transaction, &terminal.id, terminal.now)?;
        let result = find_scoped(&transaction, &terminal.workspace_id, &terminal.id)?
            .ok_or_else(|| TerminalError::not_found(&terminal.id))?;
        transaction.commit().map_err(TerminalError::database)?;
        Ok(result)
    }

    pub fn create_agent(
        &self,
        mut terminal: NewTerminalSession,
        agent: &NewAgentSession,
    ) -> Result<TerminalSession, TerminalError> {
        if terminal.workspace_id != agent.workspace_id
            || terminal.session_kind != SessionKind::Agent
        {
            return Err(TerminalError::InvalidLaunchSpec);
        }

        let mut connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(TerminalError::database)?;
        let prefix = match agent.provider_id {
            crate::domain::provider::ProviderId::Codex => "Codex",
            crate::domain::provider::ProviderId::Pi => "Pi",
        };
        let pattern = format!("{prefix} [0-9]*");
        let offset = i64::try_from(prefix.len() + 2).unwrap_or(i64::MAX);
        let ordinal: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(CAST(SUBSTR(title, ?1) AS INTEGER)), 0) + 1
                 FROM terminal_sessions
                 WHERE workspace_id = ?2 AND title GLOB ?3",
                params![offset, terminal.workspace_id.as_str(), pattern],
                |row| row.get(0),
            )
            .map_err(TerminalError::database)?;
        terminal.title = format!("{prefix} {ordinal}");
        transaction
            .execute(
                "INSERT INTO terminal_sessions (
                    id, workspace_id, title, shell, cwd, status, cols, rows, created_at,
                    session_kind
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'starting', ?6, ?7, ?8, 'agent')",
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
        let argv = serde_json::to_string(&agent.launch_snapshot.argv)
            .map_err(|_| TerminalError::Database)?;
        transaction
            .execute(
                "INSERT INTO agent_sessions (
                    id, workspace_id, terminal_id, provider_id, provider_session_id,
                    launch_mode, isolation_mode, restarted_from_session_id,
                    executable_path, launch_argv_json, provider_version, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    agent.id.as_str(),
                    agent.workspace_id.as_str(),
                    terminal.id.as_str(),
                    agent.provider_id.as_str(),
                    agent.provider_session_id.as_deref(),
                    agent.launch_mode.as_str(),
                    agent.isolation_mode.as_str(),
                    agent
                        .restarted_from_session_id
                        .as_ref()
                        .map(|id| id.as_str()),
                    agent.launch_snapshot.executable_path.as_deref(),
                    argv,
                    agent.launch_snapshot.provider_version.as_deref(),
                    agent.created_at,
                ],
            )
            .map_err(TerminalError::database)?;
        insert_initial_records(&transaction, &terminal.id, terminal.now)?;
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
            StatusTransition {
                status: TerminalStatus::Running,
                started_at: Some(now),
                ended: None,
                reason: None,
                event_kind: LifecycleEventKind::Running,
                dedupe_key: "running",
                accept_running: false,
            },
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
        let kind = match status {
            TerminalStatus::Exited => LifecycleEventKind::Exited,
            TerminalStatus::Failed => LifecycleEventKind::Failed,
            TerminalStatus::Stopped => LifecycleEventKind::Stopped,
            TerminalStatus::Interrupted => LifecycleEventKind::Interrupted,
            TerminalStatus::Starting | TerminalStatus::Running => {
                return Err(TerminalError::InvalidTransition)
            }
        };
        self.update_status(
            workspace_id,
            terminal_id,
            StatusTransition {
                status,
                started_at: None,
                ended: Some((now, exit_code)),
                reason: Some(reason),
                event_kind: kind,
                dedupe_key: "final",
                accept_running: true,
            },
        )
    }

    fn update_status(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
        transition: StatusTransition<'_>,
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
                 WHERE id = ?6 AND workspace_id = ?7
                   AND (
                        status = 'starting'
                        OR (?8 = 1 AND status = 'running')
                   )",
                params![
                    transition.status.as_str(),
                    transition.started_at,
                    transition.ended.map(|value| value.0),
                    transition.ended.and_then(|value| value.1),
                    transition.reason,
                    terminal_id.as_str(),
                    workspace_id.as_str(),
                    transition.accept_running,
                ],
            )
            .map_err(TerminalError::database)?;
        if changed == 0 {
            return find_scoped(&transaction, workspace_id, terminal_id)?
                .ok_or_else(|| TerminalError::not_found(terminal_id));
        }
        insert_lifecycle_event(
            &transaction,
            terminal_id,
            transition.event_kind,
            transition.status,
            transition
                .ended
                .map(|value| value.0)
                .or(transition.started_at)
                .unwrap_or(0),
            transition.ended.and_then(|value| value.1),
            transition.reason,
            transition.dedupe_key,
        )?;
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
        self.append_log_internal(terminal_id, data, now, false)
    }

    pub fn append_truncation_marker(
        &self,
        terminal_id: &TerminalId,
        data: &[u8],
        now: i64,
    ) -> Result<(), TerminalError> {
        self.append_log_internal(terminal_id, data, now, true)
    }

    fn append_log_internal(
        &self,
        terminal_id: &TerminalId,
        data: &[u8],
        now: i64,
        marks_truncated: bool,
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
                "SELECT next_sequence FROM terminal_log_index WHERE terminal_id = ?1",
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
        transaction
            .execute(
                "UPDATE terminal_log_index
                 SET next_sequence = ?1,
                     coverage = CASE WHEN ?2 THEN 'truncated' ELSE coverage END,
                     updated_at = ?3
                 WHERE terminal_id = ?4",
                params![
                    sequence.saturating_add(1),
                    marks_truncated,
                    now,
                    terminal_id.as_str()
                ],
            )
            .map_err(TerminalError::database)?;
        let trimmed = trim_log_to_limit(&transaction, terminal_id)?;
        refresh_log_index(&transaction, terminal_id, now, marks_truncated || trimmed)?;
        transaction.commit().map_err(TerminalError::database)?;
        Ok(())
    }

    #[cfg(test)]
    pub fn lifecycle_events(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
    ) -> Result<Vec<SessionLifecycleEvent>, TerminalError> {
        let connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        if find_scoped(&connection, workspace_id, terminal_id)?.is_none() {
            return Err(TerminalError::not_found(terminal_id));
        }
        let mut statement = connection
            .prepare(
                "SELECT terminal_id, sequence, kind, status, occurred_at, exit_code, reason
                 FROM terminal_lifecycle_events
                 WHERE terminal_id = ?1 ORDER BY sequence ASC",
            )
            .map_err(TerminalError::database)?;
        let result = statement
            .query_map(params![terminal_id.as_str()], map_lifecycle_event)
            .map_err(TerminalError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(TerminalError::database)?;
        Ok(result)
    }

    #[cfg(test)]
    pub fn log_index(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
    ) -> Result<TerminalLogIndex, TerminalError> {
        let connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        if find_scoped(&connection, workspace_id, terminal_id)?.is_none() {
            return Err(TerminalError::not_found(terminal_id));
        }
        connection
            .query_row(
                "SELECT terminal_id, first_sequence, last_sequence, chunk_count,
                        retained_bytes, coverage, updated_at
                 FROM terminal_log_index WHERE terminal_id = ?1",
                params![terminal_id.as_str()],
                map_log_index,
            )
            .map_err(TerminalError::database)
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
        let mut connection = self
            .database
            .lock()
            .map_err(TerminalError::from_workspace)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(TerminalError::database)?;
        let terminal_ids = {
            let mut statement = transaction
                .prepare(
                    "SELECT id FROM terminal_sessions
                     WHERE status IN ('starting', 'running') ORDER BY id ASC",
                )
                .map_err(TerminalError::database)?;
            let result = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(TerminalError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(TerminalError::database)?;
            result
        };
        for id in &terminal_ids {
            let terminal_id = TerminalId::from(id.clone());
            transaction
                .execute(
                    "UPDATE terminal_sessions
                     SET status = 'interrupted', ended_at = ?1,
                         termination_reason = 'app_restart'
                     WHERE id = ?2 AND status IN ('starting', 'running')",
                    params![now, id],
                )
                .map_err(TerminalError::database)?;
            insert_lifecycle_event(
                &transaction,
                &terminal_id,
                LifecycleEventKind::Interrupted,
                TerminalStatus::Interrupted,
                now,
                None,
                Some("app_restart"),
                "final",
            )?;
        }
        transaction.commit().map_err(TerminalError::database)?;
        Ok(terminal_ids.len())
    }
}

struct StatusTransition<'a> {
    status: TerminalStatus,
    started_at: Option<i64>,
    ended: Option<(i64, Option<i32>)>,
    reason: Option<&'a str>,
    event_kind: LifecycleEventKind,
    dedupe_key: &'a str,
    accept_running: bool,
}

fn trim_log_to_limit(
    connection: &Connection,
    terminal_id: &TerminalId,
) -> Result<bool, TerminalError> {
    let mut trimmed = false;
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
            return Ok(trimmed);
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
            return Ok(trimmed);
        }
        trimmed = true;
    }
}

fn insert_initial_records(
    connection: &Connection,
    terminal_id: &TerminalId,
    now: i64,
) -> Result<(), TerminalError> {
    connection
        .execute(
            "INSERT INTO terminal_log_index (
                terminal_id, next_sequence, first_sequence, last_sequence,
                chunk_count, retained_bytes, coverage, updated_at
             ) VALUES (?1, 1, NULL, NULL, 0, 0, 'complete', ?2)",
            params![terminal_id.as_str(), now],
        )
        .map_err(TerminalError::database)?;
    insert_lifecycle_event(
        connection,
        terminal_id,
        LifecycleEventKind::Created,
        TerminalStatus::Starting,
        now,
        None,
        None,
        "created",
    )
}

#[allow(clippy::too_many_arguments)]
fn insert_lifecycle_event(
    connection: &Connection,
    terminal_id: &TerminalId,
    kind: LifecycleEventKind,
    status: TerminalStatus,
    occurred_at: i64,
    exit_code: Option<i32>,
    reason: Option<&str>,
    dedupe_key: &str,
) -> Result<(), TerminalError> {
    let sequence: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1
             FROM terminal_lifecycle_events WHERE terminal_id = ?1",
            params![terminal_id.as_str()],
            |row| row.get(0),
        )
        .map_err(TerminalError::database)?;
    connection
        .execute(
            "INSERT OR IGNORE INTO terminal_lifecycle_events (
                terminal_id, sequence, kind, status, occurred_at, exit_code, reason, dedupe_key
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                terminal_id.as_str(),
                sequence,
                kind.as_str(),
                status.as_str(),
                occurred_at,
                exit_code,
                reason,
                dedupe_key,
            ],
        )
        .map_err(TerminalError::database)?;
    Ok(())
}

fn refresh_log_index(
    connection: &Connection,
    terminal_id: &TerminalId,
    now: i64,
    truncated: bool,
) -> Result<(), TerminalError> {
    connection
        .execute(
            "UPDATE terminal_log_index
             SET first_sequence = (
                    SELECT MIN(sequence) FROM terminal_log_chunks WHERE terminal_id = ?1
                 ),
                 last_sequence = (
                    SELECT MAX(sequence) FROM terminal_log_chunks WHERE terminal_id = ?1
                 ),
                 chunk_count = (
                    SELECT COUNT(*) FROM terminal_log_chunks WHERE terminal_id = ?1
                 ),
                 retained_bytes = (
                    SELECT COALESCE(SUM(byte_length), 0)
                    FROM terminal_log_chunks WHERE terminal_id = ?1
                 ),
                 coverage = CASE WHEN ?2 THEN 'truncated' ELSE coverage END,
                 updated_at = ?3
             WHERE terminal_id = ?1",
            params![terminal_id.as_str(), truncated, now],
        )
        .map_err(TerminalError::database)?;
    Ok(())
}

pub(super) fn find_scoped(
    connection: &Connection,
    workspace_id: &WorkspaceId,
    terminal_id: &TerminalId,
) -> Result<Option<TerminalSession>, TerminalError> {
    connection
        .query_row(
            "SELECT id, workspace_id, title, shell, cwd, status, cols, rows,
                    created_at, started_at, ended_at, exit_code, termination_reason,
                    session_kind
             FROM terminal_sessions
             WHERE id = ?1 AND workspace_id = ?2",
            params![terminal_id.as_str(), workspace_id.as_str()],
            map_session,
        )
        .optional()
        .map_err(TerminalError::database)
}

pub(super) fn map_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<TerminalSession> {
    map_session_at(row, 0)
}

pub(super) fn map_session_at(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<TerminalSession> {
    let status: String = row.get(offset + 5)?;
    let status = TerminalStatus::try_from(status.as_str()).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            offset + 5,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
        )
    })?;
    let session_kind: String = row.get(offset + 13)?;
    let session_kind = SessionKind::try_from(session_kind.as_str()).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            offset + 13,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
        )
    })?;
    Ok(TerminalSession {
        id: TerminalId::from(row.get::<_, String>(offset)?),
        workspace_id: WorkspaceId::from(row.get::<_, String>(offset + 1)?),
        title: row.get(offset + 2)?,
        shell: row.get(offset + 3)?,
        cwd: row.get(offset + 4)?,
        status,
        cols: row.get(offset + 6)?,
        rows: row.get(offset + 7)?,
        created_at: row.get(offset + 8)?,
        started_at: row.get(offset + 9)?,
        ended_at: row.get(offset + 10)?,
        exit_code: row.get(offset + 11)?,
        termination_reason: row.get(offset + 12)?,
        session_kind,
    })
}

pub(super) fn map_lifecycle_event(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<SessionLifecycleEvent> {
    let kind: String = row.get(2)?;
    let status: String = row.get(3)?;
    Ok(SessionLifecycleEvent {
        terminal_id: TerminalId::from(row.get::<_, String>(0)?),
        sequence: row.get(1)?,
        kind: LifecycleEventKind::try_from(kind.as_str()).map_err(conversion_error(2))?,
        status: TerminalStatus::try_from(status.as_str()).map_err(conversion_error(3))?,
        occurred_at: row.get(4)?,
        exit_code: row.get(5)?,
        reason: row.get(6)?,
    })
}

pub(super) fn map_log_index(row: &rusqlite::Row<'_>) -> rusqlite::Result<TerminalLogIndex> {
    let coverage: String = row.get(5)?;
    Ok(TerminalLogIndex {
        terminal_id: TerminalId::from(row.get::<_, String>(0)?),
        first_sequence: row.get(1)?,
        last_sequence: row.get(2)?,
        chunk_count: row.get(3)?,
        retained_bytes: row.get(4)?,
        coverage: TerminalLogCoverage::try_from(coverage.as_str()).map_err(conversion_error(5))?,
        updated_at: row.get(6)?,
    })
}

fn conversion_error(index: usize) -> impl FnOnce(String) -> rusqlite::Error {
    move |error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
        )
    }
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
        domain::terminal::{
            LifecycleEventKind, NewTerminalSession, SessionKind, TerminalId, TerminalLogCoverage,
            TerminalStatus,
        },
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
                    session_kind: SessionKind::Shell,
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
        assert_eq!(terminal.session_kind, SessionKind::Shell);
        let events = context
            .repository
            .lifecycle_events(&context.workspace_id, &terminal.id)
            .expect("events");
        let index = context
            .repository
            .log_index(&context.workspace_id, &terminal.id)
            .expect("log index");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, LifecycleEventKind::Created);
        assert_eq!(index.coverage, TerminalLogCoverage::Complete);
        assert_eq!(index.chunk_count, 0);
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
        let repeated = context.repository.recover_interrupted(4).expect("repeat");
        let session = context
            .repository
            .get_scoped(&context.workspace_id, &terminal.id)
            .expect("session");

        assert_eq!(recovered, 1);
        assert_eq!(repeated, 0);
        assert_eq!(session.status, TerminalStatus::Interrupted);
        assert_eq!(session.ended_at, Some(3));
        let events = context
            .repository
            .lifecycle_events(&context.workspace_id, &terminal.id)
            .expect("events");
        assert_eq!(
            events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            events.last().map(|event| event.kind),
            Some(LifecycleEventKind::Interrupted)
        );
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
        let index = context
            .repository
            .log_index(&context.workspace_id, &terminal.id)
            .expect("log index");
        assert_eq!(
            index.retained_bytes,
            i64::try_from(retained).expect("retained")
        );
        assert_eq!(
            index.chunk_count,
            i64::try_from(chunks.len()).expect("chunks")
        );
        assert_eq!(index.coverage, TerminalLogCoverage::Truncated);
        assert_eq!(index.last_sequence, Some(140));
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

    #[test]
    fn final_state_and_lifecycle_event_cannot_be_overwritten() {
        let context = Context::new();
        let terminal = context.create("shell");
        context
            .repository
            .mark_running(&context.workspace_id, &terminal.id, 2)
            .expect("running");
        context
            .repository
            .finish(
                &context.workspace_id,
                &terminal.id,
                TerminalStatus::Stopped,
                None,
                "user_stop",
                3,
            )
            .expect("stopped");
        let late = context
            .repository
            .finish(
                &context.workspace_id,
                &terminal.id,
                TerminalStatus::Failed,
                Some(9),
                "late_waiter",
                4,
            )
            .expect("late transition is idempotent");
        let events = context
            .repository
            .lifecycle_events(&context.workspace_id, &terminal.id)
            .expect("events");

        assert_eq!(late.status, TerminalStatus::Stopped);
        assert_eq!(late.exit_code, None);
        assert_eq!(events.len(), 3);
        assert_eq!(
            events.last().map(|event| event.kind),
            Some(LifecycleEventKind::Stopped)
        );
    }

    #[test]
    fn explicit_gap_marker_marks_log_coverage_truncated() {
        let context = Context::new();
        let terminal = context.create("shell");
        context
            .repository
            .append_truncation_marker(&terminal.id, b"[truncated]", 2)
            .expect("marker");
        let index = context
            .repository
            .log_index(&context.workspace_id, &terminal.id)
            .expect("index");

        assert_eq!(index.coverage, TerminalLogCoverage::Truncated);
        assert_eq!(index.first_sequence, Some(1));
        assert_eq!(index.last_sequence, Some(1));
    }
}
