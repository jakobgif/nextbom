use crate::data::Project;
use crate::AppState;
use tauri::{AppHandle, Emitter, State};

/// Creates a new project
#[tauri::command]
pub fn create_project(
    app: AppHandle,
    title: String,
    state: State<AppState>,
) -> Result<(), String> {
    // Create new project
    let mut project = Project::new();
    project.title = Some(title);

    // Update app state
    let mut current = state.current_project.lock().unwrap();
    *current = Some(project.clone());

    // Emit event to notify frontend
    app.emit("project-changed", Some(project.clone()))
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Closes the current project
#[tauri::command]
pub fn close_project(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let mut current = state.current_project.lock().unwrap();
    *current = None;

    // Emit event to notify frontend
    app.emit("project-changed", None::<Project>)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}
