pub mod commands;
pub mod data;

use data::{Project, RecentProjects};
use std::sync::Mutex;
use tauri::Manager;

/// All mutable state held by the application, guarded by a single lock.
///
/// Using one `Mutex` instead of several ensures every state mutation is atomic: no command
/// can observe a partially-updated snapshot between separate lock acquisitions. Guards must be
/// dropped before emitting events — never hold a lock across an `app.emit()` call.
pub struct AppStateInner {
    /// The project currently open in the application, or `None` if no project is loaded.
    pub current_project: Option<Project>,

    /// Absolute file path of the open project, or `None` if the project has never been saved.
    pub current_project_path: Option<String>,

    /// `true` when the in-memory project differs from the last saved file.
    pub has_unsaved_changes: bool,

    /// Path of a CSV file selected by the user but not yet written to a database.
    ///
    /// Set by `load_csv` and consumed by `create_nextbom_file`. `None` until the user
    /// selects a file.
    pub pending_csv_path: Option<String>,

    /// Recently opened projects list, persisted to the app config directory.
    pub recent_projects: RecentProjects,
}

/// Shared application state, accessible from all Tauri command handlers via `State<AppState>`.
pub struct AppState {
    pub inner: Mutex<AppStateInner>,
}

/// Initialises and runs the Tauri application.
///
/// Registers all plugins, sets up the managed [`AppState`] (loading recent projects from disk),
/// wires every Tauri command, and blocks until the window is closed.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let recent = commands::load_recent_from_disk(app.handle());
            app.manage(AppState {
                inner: Mutex::new(AppStateInner {
                    current_project: None,
                    current_project_path: None,
                    has_unsaved_changes: false,
                    pending_csv_path: None,
                    recent_projects: recent,
                }),
            });
            Ok(())
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
            commands::set_design_variant,
            commands::set_database_path,
            commands::load_csv,
            commands::create_nextbom_file,
            commands::get_recent_projects,
            commands::remove_recent_project,
            commands::clear_recent_projects,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
