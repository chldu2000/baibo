use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::{
    domain::{
        agent::{
            AgentIsolationMode, AgentLaunchMode, AgentLaunchSnapshot, AgentSession, AgentSessionId,
        },
        provider::ProviderId,
        session::SessionDetail,
        terminal::{TerminalId, TerminalLogIndex},
        workspace::WorkspaceId,
    },
    services::{agent::AgentError, terminal::TerminalError},
};

use super::{
    terminal_repository::{find_scoped, map_lifecycle_event, map_log_index, map_session_at},
    Database,
};

const AGENT_WITH_TERMINAL_SELECT: &str = "SELECT agent.id, agent.workspace_id, agent.provider_id,
            agent.provider_session_id, agent.launch_mode, agent.isolation_mode,
            agent.restarted_from_session_id, agent.executable_path,
            agent.launch_argv_json, agent.provider_version, agent.created_at,
            terminal.id, terminal.workspace_id, terminal.title, terminal.shell,
            terminal.cwd, terminal.status, terminal.cols, terminal.rows,
            terminal.created_at, terminal.started_at, terminal.ended_at,
            terminal.exit_code, terminal.termination_reason, terminal.session_kind
     FROM agent_sessions AS agent
     JOIN terminal_sessions AS terminal ON terminal.id = agent.terminal_id";

#[derive(Clone)]
pub struct AgentRepository {
    database: Database,
}

impl AgentRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn list(&self, workspace_id: &WorkspaceId) -> Result<Vec<AgentSession>, AgentError> {
        let connection = self.database.lock().map_err(AgentError::from_workspace)?;
        let sql = format!(
            "{AGENT_WITH_TERMINAL_SELECT}
             WHERE agent.workspace_id = ?1
             ORDER BY agent.created_at DESC, agent.id ASC"
        );
        let mut statement = connection.prepare(&sql).map_err(AgentError::database)?;
        let result = statement
            .query_map(params![workspace_id.as_str()], map_agent_session)
            .map_err(AgentError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AgentError::database)?;
        Ok(result)
    }

    pub fn get_scoped(
        &self,
        workspace_id: &WorkspaceId,
        agent_session_id: &AgentSessionId,
    ) -> Result<AgentSession, AgentError> {
        let connection = self.database.lock().map_err(AgentError::from_workspace)?;
        let sql = format!(
            "{AGENT_WITH_TERMINAL_SELECT}
             WHERE agent.id = ?1 AND agent.workspace_id = ?2"
        );
        connection
            .query_row(
                &sql,
                params![agent_session_id.as_str(), workspace_id.as_str()],
                map_agent_session,
            )
            .optional()
            .map_err(AgentError::database)?
            .ok_or_else(|| AgentError::not_found(agent_session_id))
    }

    pub fn detail(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &TerminalId,
    ) -> Result<SessionDetail, AgentError> {
        let mut connection = self.database.lock().map_err(AgentError::from_workspace)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(AgentError::database)?;
        let terminal = find_scoped(&transaction, workspace_id, terminal_id)
            .map_err(AgentError::Terminal)?
            .ok_or_else(|| AgentError::Terminal(TerminalError::not_found(terminal_id)))?;
        let agent_session = {
            let sql = format!(
                "{AGENT_WITH_TERMINAL_SELECT}
                 WHERE agent.terminal_id = ?1 AND agent.workspace_id = ?2"
            );
            transaction
                .query_row(
                    &sql,
                    params![terminal_id.as_str(), workspace_id.as_str()],
                    map_agent_session,
                )
                .optional()
                .map_err(AgentError::database)?
        };
        let lifecycle_events = {
            let mut statement = transaction
                .prepare(
                    "SELECT terminal_id, sequence, kind, status, occurred_at, exit_code, reason
                     FROM terminal_lifecycle_events
                     WHERE terminal_id = ?1 ORDER BY sequence ASC",
                )
                .map_err(AgentError::database)?;
            let events = statement
                .query_map(params![terminal_id.as_str()], map_lifecycle_event)
                .map_err(AgentError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(AgentError::database)?;
            events
        };
        let log_index: TerminalLogIndex = transaction
            .query_row(
                "SELECT terminal_id, first_sequence, last_sequence, chunk_count,
                        retained_bytes, coverage, updated_at
                 FROM terminal_log_index WHERE terminal_id = ?1",
                params![terminal_id.as_str()],
                map_log_index,
            )
            .map_err(AgentError::database)?;
        transaction.commit().map_err(AgentError::database)?;
        Ok(SessionDetail {
            terminal,
            agent_session,
            lifecycle_events,
            log_index,
        })
    }
}

fn map_agent_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentSession> {
    let provider_id: String = row.get(2)?;
    let launch_mode: String = row.get(4)?;
    let isolation_mode: String = row.get(5)?;
    let argv_json: String = row.get(8)?;
    let argv = serde_json::from_str::<Vec<String>>(&argv_json)
        .map_err(|error| conversion_error(8, error.to_string()))?;
    Ok(AgentSession {
        id: AgentSessionId::from(row.get::<_, String>(0)?),
        workspace_id: WorkspaceId::from(row.get::<_, String>(1)?),
        provider_id: ProviderId::try_from(provider_id.as_str())
            .map_err(|error| conversion_error(2, error))?,
        provider_session_id: row.get(3)?,
        launch_mode: AgentLaunchMode::try_from(launch_mode.as_str())
            .map_err(|error| conversion_error(4, error))?,
        isolation_mode: AgentIsolationMode::try_from(isolation_mode.as_str())
            .map_err(|error| conversion_error(5, error))?,
        restarted_from_session_id: row.get::<_, Option<String>>(6)?.map(AgentSessionId::from),
        created_at: row.get(10)?,
        launch_snapshot: AgentLaunchSnapshot {
            executable_path: row.get(7)?,
            argv,
            provider_version: row.get(9)?,
        },
        terminal: map_session_at(row, 11)?,
    })
}

fn conversion_error(index: usize, error: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
    )
}
