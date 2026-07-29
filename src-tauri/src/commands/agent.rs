use tauri::State;

use crate::{
    domain::{
        agent::{AgentSession, AgentSessionId},
        provider::ProviderId,
        session::SessionDetail,
        terminal::TerminalId,
        workspace::WorkspaceId,
    },
    services::agent::{AgentCommandError, AgentError, AgentManager},
};

#[tauri::command]
pub async fn list_agent_sessions(
    workspace_id: WorkspaceId,
    manager: State<'_, AgentManager>,
) -> Result<Vec<AgentSession>, AgentCommandError> {
    run_blocking(manager.inner().clone(), move |manager| {
        manager.list(&workspace_id)
    })
    .await
}

#[tauri::command]
pub async fn create_agent_session(
    workspace_id: WorkspaceId,
    provider_id: ProviderId,
    cols: u16,
    rows: u16,
    manager: State<'_, AgentManager>,
) -> Result<AgentSession, AgentCommandError> {
    run_blocking(manager.inner().clone(), move |manager| {
        manager.create(workspace_id, provider_id, cols, rows)
    })
    .await
}

#[tauri::command]
pub async fn restart_agent_session(
    workspace_id: WorkspaceId,
    agent_session_id: AgentSessionId,
    cols: u16,
    rows: u16,
    manager: State<'_, AgentManager>,
) -> Result<AgentSession, AgentCommandError> {
    run_blocking(manager.inner().clone(), move |manager| {
        manager.restart(workspace_id, &agent_session_id, cols, rows)
    })
    .await
}

#[tauri::command]
pub async fn stop_agent_session(
    workspace_id: WorkspaceId,
    agent_session_id: AgentSessionId,
    manager: State<'_, AgentManager>,
) -> Result<AgentSession, AgentCommandError> {
    run_blocking(manager.inner().clone(), move |manager| {
        manager.stop(&workspace_id, &agent_session_id)
    })
    .await
}

#[tauri::command]
pub async fn delete_agent_session(
    workspace_id: WorkspaceId,
    agent_session_id: AgentSessionId,
    manager: State<'_, AgentManager>,
) -> Result<(), AgentCommandError> {
    run_blocking(manager.inner().clone(), move |manager| {
        manager.delete(&workspace_id, &agent_session_id)
    })
    .await
}

#[tauri::command]
pub async fn get_session_detail(
    workspace_id: WorkspaceId,
    terminal_id: TerminalId,
    manager: State<'_, AgentManager>,
) -> Result<SessionDetail, AgentCommandError> {
    run_blocking(manager.inner().clone(), move |manager| {
        manager.detail(&workspace_id, &terminal_id)
    })
    .await
}

async fn run_blocking<T, F>(manager: AgentManager, operation: F) -> Result<T, AgentCommandError>
where
    T: Send + 'static,
    F: FnOnce(&AgentManager) -> Result<T, AgentError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || operation(&manager))
        .await
        .map_err(|_| AgentCommandError {
            code: "agent_task_failed",
            message: "Agent 任务异常结束".into(),
        })?
        .map_err(Into::into)
}
