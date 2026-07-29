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
            app.manage(services::workspace::WorkspaceService::new(
                persistence::WorkspaceRepository::new(database),
                app_data_root,
            ));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_info,
            commands::workspace::list_workspaces,
            commands::workspace::register_workspace,
            commands::workspace::open_workspace,
            commands::workspace::rename_workspace,
            commands::workspace::remove_workspace,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Baibo");
}
