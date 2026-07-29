use std::sync::{Arc, Mutex};

use serde::Serialize;
use thiserror::Error;

use crate::domain::{
    agent::{AgentIsolationMode, AgentLaunchMode, AgentSession, AgentSessionId, NewAgentSession},
    provider::ProviderId,
    session::SessionDetail,
    terminal::TerminalStatus,
    workspace::{WorkspaceId, WorkspaceRegistrySnapshot},
};
use crate::persistence::AgentRepository;

use super::{
    provider::{ProviderError, ProviderService},
    terminal::{TerminalError, TerminalManager},
};

#[derive(Clone)]
pub struct AgentManager {
    providers: ProviderService,
    terminals: TerminalManager,
    repository: AgentRepository,
    operation_lock: Arc<Mutex<()>>,
}

impl AgentManager {
    pub fn new(
        providers: ProviderService,
        terminals: TerminalManager,
        repository: AgentRepository,
    ) -> Self {
        Self {
            providers,
            terminals,
            repository,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn list(&self, workspace_id: &WorkspaceId) -> Result<Vec<AgentSession>, AgentError> {
        self.terminals
            .list(workspace_id)
            .map_err(AgentError::from_terminal)?;
        self.repository.list(workspace_id)
    }

    pub fn create(
        &self,
        workspace_id: WorkspaceId,
        provider_id: ProviderId,
        cols: u16,
        rows: u16,
    ) -> Result<AgentSession, AgentError> {
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|_| AgentError::Internal)?;
        self.create_locked(workspace_id, provider_id, cols, rows, None)
    }

    pub fn restart(
        &self,
        workspace_id: WorkspaceId,
        agent_session_id: &AgentSessionId,
        cols: u16,
        rows: u16,
    ) -> Result<AgentSession, AgentError> {
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|_| AgentError::Internal)?;
        let previous = self.scoped(&workspace_id, agent_session_id)?;
        let terminal = self
            .terminals
            .get(&workspace_id, &previous.terminal.id)
            .map_err(AgentError::from_terminal)?;
        if matches!(
            terminal.status,
            TerminalStatus::Starting | TerminalStatus::Running
        ) {
            return Err(AgentError::StillRunning);
        }
        self.providers
            .refresh()
            .map_err(AgentError::from_provider)?;
        self.create_locked(
            workspace_id,
            previous.provider_id,
            cols,
            rows,
            Some(previous.id),
        )
    }

    pub fn stop(
        &self,
        workspace_id: &WorkspaceId,
        agent_session_id: &AgentSessionId,
    ) -> Result<AgentSession, AgentError> {
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|_| AgentError::Internal)?;
        let session = self.scoped(workspace_id, agent_session_id)?;
        self.terminals
            .stop(workspace_id, &session.terminal.id)
            .map_err(AgentError::from_terminal)?;
        self.repository.get_scoped(workspace_id, agent_session_id)
    }

    pub fn delete(
        &self,
        workspace_id: &WorkspaceId,
        agent_session_id: &AgentSessionId,
    ) -> Result<(), AgentError> {
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|_| AgentError::Internal)?;
        let session = self.scoped(workspace_id, agent_session_id)?;
        self.terminals
            .delete(workspace_id, &session.terminal.id)
            .map_err(AgentError::from_terminal)?;
        Ok(())
    }

    pub fn remove_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceRegistrySnapshot, AgentError> {
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|_| AgentError::Internal)?;
        self.terminals
            .remove_workspace(workspace_id.clone())
            .map_err(AgentError::from_terminal)
    }

    fn create_locked(
        &self,
        workspace_id: WorkspaceId,
        provider_id: ProviderId,
        cols: u16,
        rows: u16,
        restarted_from_session_id: Option<AgentSessionId>,
    ) -> Result<AgentSession, AgentError> {
        let id = AgentSessionId::new();
        let prepared = self
            .providers
            .build_launch_spec(provider_id, &workspace_id, &id)
            .map_err(AgentError::from_provider)?;
        let new_session = NewAgentSession {
            id: id.clone(),
            workspace_id: workspace_id.clone(),
            provider_id,
            provider_session_id: None,
            launch_mode: AgentLaunchMode::InteractivePty,
            isolation_mode: AgentIsolationMode::Workspace,
            restarted_from_session_id,
            created_at: 0,
            launch_snapshot: prepared.snapshot,
        };
        let terminal = self
            .terminals
            .create_agent_with_launch_spec(
                workspace_id.clone(),
                cols,
                rows,
                new_session,
                prepared.launch,
            )
            .map_err(AgentError::from_terminal)?;
        self.repository
            .get_scoped(&workspace_id, &id)
            .map(|mut session| {
                session.terminal = terminal;
                session
            })
    }

    fn scoped(
        &self,
        workspace_id: &WorkspaceId,
        agent_session_id: &AgentSessionId,
    ) -> Result<AgentSession, AgentError> {
        self.repository.get_scoped(workspace_id, agent_session_id)
    }

    pub fn detail(
        &self,
        workspace_id: &WorkspaceId,
        terminal_id: &crate::domain::terminal::TerminalId,
    ) -> Result<SessionDetail, AgentError> {
        self.repository.detail(workspace_id, terminal_id)
    }
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Agent 会话内部状态不可用")]
    Internal,
    #[error("Agent 会话数据库不可用")]
    Database,
    #[error("未找到 Agent 会话 {0}")]
    NotFound(AgentSessionId),
    #[error("运行中的 Agent 会话不能重新开始")]
    StillRunning,
    #[error("{0}")]
    Provider(ProviderError),
    #[error("{0}")]
    Terminal(TerminalError),
    #[error("{0}")]
    Workspace(crate::services::workspace::WorkspaceError),
}

impl AgentError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Internal => "agent_internal",
            Self::Database => "agent_database_unavailable",
            Self::NotFound(_) => "agent_session_not_found",
            Self::StillRunning => "agent_session_still_running",
            Self::Provider(error) => error.code(),
            Self::Terminal(error) => error.code(),
            Self::Workspace(error) => error.code(),
        }
    }

    pub(crate) fn not_found(id: &AgentSessionId) -> Self {
        Self::NotFound(id.clone())
    }

    fn from_provider(error: ProviderError) -> Self {
        Self::Provider(error)
    }

    fn from_terminal(error: TerminalError) -> Self {
        Self::Terminal(error)
    }

    pub(crate) fn database(_error: rusqlite::Error) -> Self {
        Self::Database
    }

    pub(crate) fn from_workspace(error: crate::services::workspace::WorkspaceError) -> Self {
        Self::Workspace(error)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<AgentError> for AgentCommandError {
    fn from(error: AgentError) -> Self {
        log::error!(
            target: "baibo::agent",
            "agent operation failed: {}",
            error.code()
        );
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use std::{
        collections::BTreeMap,
        ffi::OsString,
        fs,
        path::PathBuf,
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    use tempfile::TempDir;

    use crate::{
        domain::{provider::ProviderId, terminal::TerminalStatus},
        persistence::{AgentRepository, Database, TerminalRepository, WorkspaceRepository},
        services::{
            provider::ProviderService, terminal::TerminalManager, workspace::WorkspaceService,
        },
    };

    use super::AgentManager;

    struct Context {
        _temp: TempDir,
        manager: AgentManager,
        workspace_a: crate::domain::workspace::WorkspaceId,
        workspace_b: crate::domain::workspace::WorkspaceId,
        fixture: PathBuf,
        database_path: PathBuf,
    }

    impl Context {
        fn new() -> Self {
            Self::new_with_executable(PathBuf::from("/bin/sh"))
        }

        fn new_with_executable(executable: PathBuf) -> Self {
            let temp = TempDir::new().expect("temp");
            let app_data = temp.path().join("app-data/baibo");
            let database_path = app_data.join("baibo.sqlite3");
            let database = Database::open(&database_path).expect("database");
            let repository = WorkspaceRepository::new(database.clone());
            let workspace_service = WorkspaceService::new(repository, app_data);
            let path_a = temp.path().join("workspace-a");
            let path_b = temp.path().join("workspace-b");
            fs::create_dir_all(&path_a).expect("workspace A");
            fs::create_dir_all(&path_b).expect("workspace B");
            let fixture = path_a.join("keep.txt");
            fs::write(&fixture, "keep").expect("fixture");
            let workspace_a = workspace_service
                .register_path(&path_a)
                .expect("register A")
                .active_workspace_id
                .expect("active A");
            let workspace_b = workspace_service
                .register_path(&path_b)
                .expect("register B")
                .active_workspace_id
                .expect("active B");
            let terminal_manager = TerminalManager::new(
                TerminalRepository::new(database.clone()),
                workspace_service.clone(),
            );
            let provider_service = ProviderService::new_for_test(
                workspace_service,
                ProviderId::Codex,
                executable,
                BTreeMap::from([
                    (OsString::from("HOME"), temp.path().as_os_str().to_owned()),
                    (OsString::from("PATH"), OsString::from("/usr/bin:/bin")),
                    (
                        OsString::from("PROVIDER_SECRET"),
                        OsString::from("must-not-persist"),
                    ),
                ]),
            );
            Self {
                _temp: temp,
                manager: AgentManager::new(
                    provider_service,
                    terminal_manager,
                    AgentRepository::new(database),
                ),
                workspace_a,
                workspace_b,
                fixture,
                database_path,
            }
        }

        fn wait_for_finish(&self, terminal_id: &crate::domain::terminal::TerminalId) {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let status = self
                    .manager
                    .terminals
                    .get(&self.workspace_a, terminal_id)
                    .expect("terminal")
                    .status;
                if !matches!(status, TerminalStatus::Starting | TerminalStatus::Running) {
                    return;
                }
                assert!(Instant::now() < deadline, "agent stop timed out");
                thread::sleep(Duration::from_millis(20));
            }
        }
    }

    #[test]
    fn scopes_stop_restart_and_delete_without_touching_workspace_files() {
        let context = Context::new();
        let first = context
            .manager
            .create(context.workspace_a.clone(), ProviderId::Codex, 80, 24)
            .expect("create");
        assert_eq!(
            context
                .manager
                .stop(&context.workspace_b, &first.id)
                .expect_err("cross-workspace stop")
                .code(),
            "agent_session_not_found"
        );
        context
            .manager
            .stop(&context.workspace_a, &first.id)
            .expect("stop");
        context.wait_for_finish(&first.terminal.id);

        let restarted = context
            .manager
            .restart(context.workspace_a.clone(), &first.id, 80, 24)
            .expect("restart");
        assert_eq!(context.manager.providers.detection_count(), 1);
        assert_eq!(restarted.restarted_from_session_id, Some(first.id.clone()));
        assert_ne!(restarted.id, first.id);
        context
            .manager
            .stop(&context.workspace_a, &restarted.id)
            .expect("stop restarted");
        context.wait_for_finish(&restarted.terminal.id);
        context
            .manager
            .delete(&context.workspace_a, &first.id)
            .expect("delete first");

        assert!(context.fixture.exists());
        assert_eq!(
            context
                .manager
                .list(&context.workspace_a)
                .expect("list")
                .len(),
            1
        );
        context
            .manager
            .remove_workspace(context.workspace_a.clone())
            .expect("remove workspace");
        assert!(context.fixture.exists());
        assert!(context
            .manager
            .repository
            .list(&context.workspace_a)
            .expect("durable sessions")
            .is_empty());
    }

    #[test]
    fn stop_is_serialized_with_other_agent_mutations() {
        let context = Context::new();
        let session = context
            .manager
            .create(context.workspace_a.clone(), ProviderId::Codex, 80, 24)
            .expect("create");
        let operation = context
            .manager
            .operation_lock
            .lock()
            .expect("operation lock");
        let manager = context.manager.clone();
        let workspace_id = context.workspace_a.clone();
        let agent_session_id = session.id.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = sender.send(manager.stop(&workspace_id, &agent_session_id));
        });

        assert!(matches!(
            receiver.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        drop(operation);
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("serialized stop")
            .expect("stop");
        worker.join().expect("stop worker");
        context.wait_for_finish(&session.terminal.id);
    }

    #[test]
    fn persists_agent_identity_launch_snapshot_and_session_detail() {
        let context = Context::new();
        let created = context
            .manager
            .create(context.workspace_a.clone(), ProviderId::Codex, 80, 24)
            .expect("create");
        context
            .manager
            .stop(&context.workspace_a, &created.id)
            .expect("stop");
        context.wait_for_finish(&created.terminal.id);

        let reopened = Database::open(&context.database_path).expect("reopen database");
        let secret_count: i64 = reopened
            .lock()
            .expect("database")
            .query_row(
                "SELECT COUNT(*) FROM agent_sessions
                 WHERE executable_path LIKE '%must-not-persist%'
                    OR launch_argv_json LIKE '%must-not-persist%'
                    OR provider_version LIKE '%must-not-persist%'",
                [],
                |row| row.get(0),
            )
            .expect("secret query");
        let durable = AgentRepository::new(reopened)
            .list(&context.workspace_a)
            .expect("durable sessions");
        let detail = context
            .manager
            .detail(&context.workspace_a, &created.terminal.id)
            .expect("detail");

        assert_eq!(durable.len(), 1);
        assert_eq!(durable[0].id, created.id);
        assert_eq!(
            durable[0].launch_snapshot.executable_path.as_deref(),
            Some("/bin/sh")
        );
        assert_eq!(
            durable[0].launch_snapshot.provider_version.as_deref(),
            Some("test")
        );
        assert!(durable[0].launch_snapshot.argv.is_empty());
        assert_eq!(secret_count, 0);
        assert_eq!(
            detail.agent_session.as_ref().map(|session| &session.id),
            Some(&created.id)
        );
        assert_eq!(
            detail
                .lifecycle_events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(detail.log_index.terminal_id, created.terminal.id);
        assert_eq!(
            context
                .manager
                .detail(&context.workspace_b, &created.terminal.id)
                .expect_err("cross-workspace detail")
                .code(),
            "terminal_not_found"
        );
    }

    #[test]
    fn persists_failed_agent_when_the_detected_executable_is_unavailable() {
        let context = Context::new_with_executable(PathBuf::from(
            "/baibo-test/provider-executable-does-not-exist",
        ));

        assert_eq!(
            context
                .manager
                .create(context.workspace_a.clone(), ProviderId::Codex, 80, 24)
                .expect_err("missing executable")
                .code(),
            "terminal_spawn_failed"
        );
        let sessions = context
            .manager
            .list(&context.workspace_a)
            .expect("durable failed agent");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].terminal.status, TerminalStatus::Failed);
        let detail = context
            .manager
            .detail(&context.workspace_a, &sessions[0].terminal.id)
            .expect("failed detail");
        assert_eq!(
            detail
                .lifecycle_events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            vec![
                crate::domain::terminal::LifecycleEventKind::Created,
                crate::domain::terminal::LifecycleEventKind::Failed,
            ]
        );
        assert_eq!(
            detail
                .lifecycle_events
                .last()
                .and_then(|event| event.reason.as_deref()),
            Some("executable_unavailable")
        );
    }
}
