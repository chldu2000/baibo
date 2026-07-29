mod adapters;
mod commands;
mod domain;
mod persistence;
mod services;

use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut log_targets = vec![Target::new(TargetKind::LogDir {
        file_name: Some("baibo".into()),
    })];
    if cfg!(debug_assertions) {
        log_targets.push(Target::new(TargetKind::Stdout));
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .filter(|metadata| metadata.target().starts_with("baibo"))
                .targets(log_targets)
                .build(),
        )
        .setup(|app| {
            let app_data_root = app.path().app_data_dir()?.join("baibo");
            let database = persistence::Database::open(&app_data_root.join("baibo.sqlite3"))
                .inspect_err(|error| {
                    log::error!(
                        target: "baibo::database",
                        "workspace registry startup failed: {}",
                        error.code()
                    );
                })?;
            let workspace_service = services::workspace::WorkspaceService::new(
                persistence::WorkspaceRepository::new(database.clone()),
                app_data_root,
            );
            let terminal_manager = services::terminal::TerminalManager::new(
                persistence::TerminalRepository::new(database.clone()),
                workspace_service.clone(),
            );
            terminal_manager.recover().inspect_err(|error| {
                log::error!(
                    target: "baibo::terminal",
                    "terminal recovery failed: {}",
                    error.code()
                );
            })?;
            let provider_service =
                services::provider::ProviderService::new(workspace_service.clone());
            let agent_manager = services::agent::AgentManager::new(
                provider_service.clone(),
                terminal_manager.clone(),
                persistence::AgentRepository::new(database),
            );
            app.manage(workspace_service);
            app.manage(terminal_manager);
            app.manage(provider_service);
            app.manage(agent_manager);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_info,
            commands::workspace::list_workspaces,
            commands::workspace::register_workspace,
            commands::workspace::open_workspace,
            commands::workspace::rename_workspace,
            commands::workspace::remove_workspace,
            commands::terminal::list_terminals,
            commands::terminal::create_terminal,
            commands::terminal::attach_terminal,
            commands::terminal::detach_terminal,
            commands::terminal::write_terminal_input,
            commands::terminal::resize_terminal,
            commands::terminal::stop_terminal,
            commands::terminal::delete_terminal,
            commands::provider::list_providers,
            commands::provider::refresh_providers,
            commands::provider::get_pi_project_trust,
            commands::provider::run_pi_rpc_probe,
            commands::agent::list_agent_sessions,
            commands::agent::create_agent_session,
            commands::agent::restart_agent_session,
            commands::agent::stop_agent_session,
            commands::agent::delete_agent_session,
            commands::agent::get_session_detail,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Baibo");
}
