use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::{
    domain::workspace::{WorkspaceId, WorkspaceRegistrySnapshot},
    services::workspace::{CommandError, WorkspaceService},
};

#[tauri::command]
pub async fn list_workspaces(
    service: State<'_, WorkspaceService>,
) -> Result<WorkspaceRegistrySnapshot, CommandError> {
    run_blocking(service.inner().clone(), WorkspaceService::list).await
}

#[tauri::command]
pub async fn register_workspace(
    app: AppHandle,
    service: State<'_, WorkspaceService>,
) -> Result<Option<WorkspaceRegistrySnapshot>, CommandError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let selected = app
            .dialog()
            .file()
            .set_title("选择 Baibo 工作空间")
            .blocking_pick_folder();
        let Some(selected) = selected else {
            return Ok(None);
        };
        let path = selected.into_path().map_err(|error| CommandError {
            code: "unsupported_path_encoding",
            message: format!("无法读取所选目录：{error}"),
        })?;
        service.register_path(&path).map(Some).map_err(Into::into)
    })
    .await
    .map_err(|error| CommandError {
        code: "workspace_task_failed",
        message: format!("工作空间任务异常结束：{error}"),
    })?
}

#[tauri::command]
pub async fn open_workspace(
    workspace_id: WorkspaceId,
    service: State<'_, WorkspaceService>,
) -> Result<WorkspaceRegistrySnapshot, CommandError> {
    run_blocking(service.inner().clone(), move |service| {
        service.open(workspace_id)
    })
    .await
}

#[tauri::command]
pub async fn rename_workspace(
    workspace_id: WorkspaceId,
    name: String,
    service: State<'_, WorkspaceService>,
) -> Result<WorkspaceRegistrySnapshot, CommandError> {
    run_blocking(service.inner().clone(), move |service| {
        service.rename(workspace_id, &name)
    })
    .await
}

#[tauri::command]
pub async fn remove_workspace(
    workspace_id: WorkspaceId,
    service: State<'_, WorkspaceService>,
) -> Result<WorkspaceRegistrySnapshot, CommandError> {
    run_blocking(service.inner().clone(), move |service| {
        service.remove(workspace_id)
    })
    .await
}

async fn run_blocking<T, F>(service: WorkspaceService, operation: F) -> Result<T, CommandError>
where
    T: Send + 'static,
    F: FnOnce(&WorkspaceService) -> Result<T, crate::services::workspace::WorkspaceError>
        + Send
        + 'static,
{
    tauri::async_runtime::spawn_blocking(move || operation(&service))
        .await
        .map_err(|error| CommandError {
            code: "workspace_task_failed",
            message: format!("工作空间任务异常结束：{error}"),
        })?
        .map_err(Into::into)
}
