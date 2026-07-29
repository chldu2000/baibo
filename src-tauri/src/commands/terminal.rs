use std::sync::Arc;

use tauri::{ipc::Channel, State};

use crate::{
    domain::{
        terminal::{
            TerminalAttachment, TerminalEvent, TerminalId, TerminalSession, TerminalSubscriptionId,
        },
        workspace::WorkspaceId,
    },
    services::terminal::{
        TerminalCommandError, TerminalError, TerminalManager, TerminalSubscriber,
    },
};

struct ChannelSubscriber {
    output: Channel<Vec<u8>>,
    events: Channel<TerminalEvent>,
}

impl TerminalSubscriber for ChannelSubscriber {
    fn send_output(&self, data: Vec<u8>) -> Result<(), ()> {
        self.output.send(data).map_err(|_| ())
    }

    fn send_event(&self, event: TerminalEvent) -> Result<(), ()> {
        self.events.send(event).map_err(|_| ())
    }
}

#[tauri::command]
pub async fn list_terminals(
    workspace_id: WorkspaceId,
    manager: State<'_, TerminalManager>,
) -> Result<Vec<TerminalSession>, TerminalCommandError> {
    run_blocking(manager.inner().clone(), move |manager| {
        manager.list(&workspace_id)
    })
    .await
}

#[tauri::command]
pub async fn create_terminal(
    workspace_id: WorkspaceId,
    cols: u16,
    rows: u16,
    manager: State<'_, TerminalManager>,
) -> Result<TerminalSession, TerminalCommandError> {
    run_blocking(manager.inner().clone(), move |manager| {
        manager.create(workspace_id, cols, rows)
    })
    .await
}

#[tauri::command]
pub async fn attach_terminal(
    workspace_id: WorkspaceId,
    terminal_id: TerminalId,
    output_channel: Channel<Vec<u8>>,
    event_channel: Channel<TerminalEvent>,
    manager: State<'_, TerminalManager>,
) -> Result<TerminalAttachment, TerminalCommandError> {
    let subscriber = Arc::new(ChannelSubscriber {
        output: output_channel,
        events: event_channel,
    });
    run_blocking(manager.inner().clone(), move |manager| {
        manager.attach(&workspace_id, &terminal_id, subscriber)
    })
    .await
}

#[tauri::command]
pub async fn detach_terminal(
    workspace_id: WorkspaceId,
    terminal_id: TerminalId,
    subscription_id: TerminalSubscriptionId,
    manager: State<'_, TerminalManager>,
) -> Result<(), TerminalCommandError> {
    run_blocking(manager.inner().clone(), move |manager| {
        manager.detach(&workspace_id, &terminal_id, &subscription_id)
    })
    .await
}

#[tauri::command]
pub async fn write_terminal_input(
    workspace_id: WorkspaceId,
    terminal_id: TerminalId,
    data: Vec<u8>,
    manager: State<'_, TerminalManager>,
) -> Result<(), TerminalCommandError> {
    run_blocking(manager.inner().clone(), move |manager| {
        manager.input(&workspace_id, &terminal_id, &data)
    })
    .await
}

#[tauri::command]
pub async fn resize_terminal(
    workspace_id: WorkspaceId,
    terminal_id: TerminalId,
    cols: u16,
    rows: u16,
    manager: State<'_, TerminalManager>,
) -> Result<TerminalSession, TerminalCommandError> {
    run_blocking(manager.inner().clone(), move |manager| {
        manager.resize(&workspace_id, &terminal_id, cols, rows)
    })
    .await
}

#[tauri::command]
pub async fn stop_terminal(
    workspace_id: WorkspaceId,
    terminal_id: TerminalId,
    manager: State<'_, TerminalManager>,
) -> Result<TerminalSession, TerminalCommandError> {
    run_blocking(manager.inner().clone(), move |manager| {
        manager.stop(&workspace_id, &terminal_id)
    })
    .await
}

#[tauri::command]
pub async fn delete_terminal(
    workspace_id: WorkspaceId,
    terminal_id: TerminalId,
    manager: State<'_, TerminalManager>,
) -> Result<(), TerminalCommandError> {
    run_blocking(manager.inner().clone(), move |manager| {
        manager.delete(&workspace_id, &terminal_id)
    })
    .await
}

async fn run_blocking<T, F>(
    manager: TerminalManager,
    operation: F,
) -> Result<T, TerminalCommandError>
where
    T: Send + 'static,
    F: FnOnce(&TerminalManager) -> Result<T, TerminalError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || operation(&manager))
        .await
        .map_err(|_| TerminalCommandError {
            code: "terminal_task_failed",
            message: "终端任务异常结束".into(),
        })?
        .map_err(Into::into)
}
