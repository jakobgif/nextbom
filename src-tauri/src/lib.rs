pub mod commands;
pub mod data;

use data::Project;
use std::sync::Mutex;

/// Shared application state, accessible from all Tauri command handlers via `State<AppState>`.
///
/// Each field is wrapped in a `Mutex` to allow mutation from async commands. Guards must be
/// dropped before emitting events — never hold a lock across an `app.emit()` call.
pub struct AppState {
    /// The project currently open in the application, or `None` if no project is loaded.
    pub current_project: Mutex<Option<Project>>,

    /// Absolute file path of the open project, or `None` if the project has never been saved.
    pub current_project_path: Mutex<Option<String>>,

    /// `true` when the in-memory project differs from the last saved file.
    pub project_has_unsaved_changes: Mutex<bool>,
}

/// Initialises and runs the Tauri application.
///
/// Registers all plugins, sets up the managed [`AppState`], wires every Tauri command, and
/// blocks until the window is closed.
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
            commands::set_database_path,
            commands::import_csv_to_database,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
