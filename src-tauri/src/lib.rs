pub mod commands;
pub mod data;

use data::Project;
use std::sync::Mutex;

/// Application state
pub struct AppState {
    pub current_project: Mutex<Option<Project>>,
    pub current_project_path: Mutex<Option<String>>,
    pub project_has_unsaved_changes: Mutex<bool>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .manage(AppState {
            current_project: Mutex::new(None),
            current_project_path: Mutex::new(None),
            project_has_unsaved_changes: Mutex::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_project_state,
            commands::create_project,
            commands::close_project,
            commands::open_project,
            commands::save_project,
            commands::set_project_title,
            commands::set_project_engineer,
            commands::set_project_specifics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
