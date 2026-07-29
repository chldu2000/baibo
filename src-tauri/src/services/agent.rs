use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use thiserror::Error;

use crate::domain::{
    agent::{AgentIsolationMode, AgentLaunchMode, AgentSession, AgentSessionId},
    provider::ProviderId,
    terminal::TerminalStatus,
    workspace::{WorkspaceId, WorkspaceRegistrySnapshot},
};

use super::{
    provider::{ProviderError, ProviderService},
    terminal::{TerminalError, TerminalManager},
};

#[derive(Clone)]
pub struct AgentManager {
    providers: ProviderService,
    terminals: TerminalManager,
    sessions: Arc<Mutex<HashMap<AgentSessionId, AgentSession>>>,
    operation_lock: Arc<Mutex<()>>,
}

impl AgentManager {
    pub fn new(providers: ProviderService, terminals: TerminalManager) -> Self {
        Self {
            providers,
            terminals,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn list(&self, workspace_id: &WorkspaceId) -> Result<Vec<AgentSession>, AgentError> {
        self.terminals
            .list(workspace_id)
            .map_err(AgentError::from_terminal)?;
        let records: Vec<AgentSession> = self
            .sessions
            .lock()
            .map_err(|_| AgentError::Internal)?
            .values()
            .filter(|session| &session.workspace_id == workspace_id)
            .cloned()
            .collect();
        let mut sessions = Vec::with_capacity(records.len());
        for mut session in records {
            session.terminal = self
                .terminals
                .get(workspace_id, &session.terminal.id)
                .map_err(AgentError::from_terminal)?;
            sessions.push(session);
        }
        sessions.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });
        Ok(sessions)
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
        let mut session = self.scoped(workspace_id, agent_session_id)?;
        session.terminal = self
            .terminals
            .stop(workspace_id, &session.terminal.id)
            .map_err(AgentError::from_terminal)?;
        self.sessions
            .lock()
            .map_err(|_| AgentError::Internal)?
            .insert(session.id.clone(), session.clone());
        Ok(session)
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
        let removed = self
            .sessions
            .lock()
            .map_err(|_| AgentError::Internal)?
            .remove(agent_session_id);
        if removed.is_some() {
            Ok(())
        } else {
            Err(AgentError::not_found(agent_session_id))
        }
    }

    pub fn remove_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceRegistrySnapshot, AgentError> {
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|_| AgentError::Internal)?;
        let snapshot = self
            .terminals
            .remove_workspace(workspace_id.clone())
            .map_err(AgentError::from_terminal)?;
        self.sessions
            .lock()
            .map_err(|_| AgentError::Internal)?
            .retain(|_, session| session.workspace_id != workspace_id);
        Ok(snapshot)
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
        let launch = self
            .providers
            .build_launch_spec(provider_id, &workspace_id, &id)
            .map_err(AgentError::from_provider)?;
        let ordinal = self
            .sessions
            .lock()
            .map_err(|_| AgentError::Internal)?
            .values()
            .filter(|session| {
                session.workspace_id == workspace_id && session.provider_id == provider_id
            })
            .count()
            + 1;
        let title = format!(
            "{} {ordinal}",
            match provider_id {
                ProviderId::Codex => "Codex",
                ProviderId::Pi => "Pi",
            }
        );
        let terminal = self
            .terminals
            .create_with_launch_spec(workspace_id.clone(), cols, rows, title, launch)
            .map_err(AgentError::from_terminal)?;
        let session = AgentSession {
            id,
            workspace_id,
            provider_id,
            provider_session_id: None,
            created_at: terminal.created_at,
            terminal,
            launch_mode: AgentLaunchMode::InteractivePty,
            isolation_mode: AgentIsolationMode::Workspace,
            restarted_from_session_id,
        };
        self.sessions
            .lock()
            .map_err(|_| AgentError::Internal)?
            .insert(session.id.clone(), session.clone());
        Ok(session)
    }

    fn scoped(
        &self,
        workspace_id: &WorkspaceId,
        agent_session_id: &AgentSessionId,
    ) -> Result<AgentSession, AgentError> {
        self.sessions
            .lock()
            .map_err(|_| AgentError::Internal)?
            .get(agent_session_id)
            .filter(|session| &session.workspace_id == workspace_id)
            .cloned()
            .ok_or_else(|| AgentError::not_found(agent_session_id))
    }
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Agent 会话内部状态不可用")]
    Internal,
    #[error("未找到 Agent 会话 {0}")]
    NotFound(AgentSessionId),
    #[error("运行中的 Agent 会话不能重新开始")]
    StillRunning,
    #[error("{0}")]
    Provider(ProviderError),
    #[error("{0}")]
    Terminal(TerminalError),
}

impl AgentError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Internal => "agent_internal",
            Self::NotFound(_) => "agent_session_not_found",
            Self::StillRunning => "agent_session_still_running",
            Self::Provider(error) => error.code(),
            Self::Terminal(error) => error.code(),
        }
    }

    fn not_found(id: &AgentSessionId) -> Self {
        Self::NotFound(id.clone())
    }

    fn from_provider(error: ProviderError) -> Self {
        Self::Provider(error)
    }

    fn from_terminal(error: TerminalError) -> Self {
        Self::Terminal(error)
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
        persistence::{Database, TerminalRepository, WorkspaceRepository},
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
    }

    impl Context {
        fn new() -> Self {
            let temp = TempDir::new().expect("temp");
            let app_data = temp.path().join("app-data/baibo");
            let database = Database::open(&app_data.join("baibo.sqlite3")).expect("database");
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
            let terminal_manager =
                TerminalManager::new(TerminalRepository::new(database), workspace_service.clone());
            let provider_service = ProviderService::new_for_test(
                workspace_service,
                ProviderId::Codex,
                PathBuf::from("/bin/sh"),
                BTreeMap::from([
                    (OsString::from("HOME"), temp.path().as_os_str().to_owned()),
                    (OsString::from("PATH"), OsString::from("/usr/bin:/bin")),
                ]),
            );
            Self {
                _temp: temp,
                manager: AgentManager::new(provider_service, terminal_manager),
                workspace_a,
                workspace_b,
                fixture,
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
            .sessions
            .lock()
            .expect("sessions")
            .values()
            .all(|session| session.workspace_id != context.workspace_a));
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
}
