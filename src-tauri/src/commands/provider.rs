use tauri::State;

use crate::{
    domain::{
        provider::{PiProjectTrust, PiRpcProbeResult, ProviderInfo},
        workspace::WorkspaceId,
    },
    services::provider::{ProviderCommandError, ProviderError, ProviderService},
};

#[tauri::command]
pub async fn list_providers(
    service: State<'_, ProviderService>,
) -> Result<Vec<ProviderInfo>, ProviderCommandError> {
    run_blocking(service.inner().clone(), ProviderService::list).await
}

#[tauri::command]
pub async fn refresh_providers(
    service: State<'_, ProviderService>,
) -> Result<Vec<ProviderInfo>, ProviderCommandError> {
    run_blocking(service.inner().clone(), ProviderService::refresh).await
}

#[tauri::command]
pub async fn get_pi_project_trust(
    workspace_id: WorkspaceId,
    service: State<'_, ProviderService>,
) -> Result<PiProjectTrust, ProviderCommandError> {
    run_blocking(service.inner().clone(), move |service| {
        service.pi_project_trust(&workspace_id)
    })
    .await
}

#[tauri::command]
pub async fn run_pi_rpc_probe(
    workspace_id: WorkspaceId,
    service: State<'_, ProviderService>,
) -> Result<PiRpcProbeResult, ProviderCommandError> {
    run_blocking(service.inner().clone(), move |service| {
        service.run_pi_rpc_probe(&workspace_id)
    })
    .await
}

async fn run_blocking<T, F>(
    service: ProviderService,
    operation: F,
) -> Result<T, ProviderCommandError>
where
    T: Send + 'static,
    F: FnOnce(&ProviderService) -> Result<T, ProviderError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || operation(&service))
        .await
        .map_err(|_| ProviderCommandError {
            code: "provider_task_failed",
            message: "provider 任务异常结束".into(),
        })?
        .map_err(Into::into)
}
