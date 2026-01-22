use crate::data::{Project, ProjectState};
use crate::AppState;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

/// Checks if the project schema version is compatible with the current application version
fn check_schema_compatibility(file_schema: &str) -> Result<(), String> {
    let current_version = env!("CARGO_PKG_VERSION");

    // Parse major versions
    let file_major = file_schema
        .split('.')
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or_else(|| format!("Invalid schema version format: {}", file_schema))?;

    let current_major = current_version
        .split('.')
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or_else(|| format!("Invalid current version format: {}", current_version))?;

    // Check major version compatibility
    if file_major != current_major {
        return Err(format!(
            "Incompatible project file version: {} (current software version: {}). Please update the application or use a compatible project file.",
            file_schema,
            current_version
        ));
    }

    Ok(())
}

/// Gets the current project state
#[tauri::command]
pub fn get_project_state(state: State<AppState>) -> ProjectState {
    let current = state.current_project.lock().unwrap();
    let unsaved = state.project_has_unsaved_changes.lock().unwrap();

    ProjectState {
        project: current.clone(),
        has_unsaved_changes: *unsaved,
    }
}

/// Creates a new project
#[tauri::command]
pub fn create_project(
    app: AppHandle,
    title: String,
    engineer: Option<String>,
    project_specifics: Option<String>,
    state: State<AppState>,
) -> Result<(), String> {
    // Create new project
    let mut project = Project::new();
    project.set_title(title);

    if let Some(eng) = engineer {
        if !eng.is_empty() {
            project.set_engineer(eng);
        }
    }

    if let Some(specs) = project_specifics {
        if !specs.is_empty() {
            project.set_project_specifics(specs);
        }
    }

    // Update app state
    let mut current = state.current_project.lock().unwrap();
    *current = Some(project.clone());
    drop(current);

    // Set unsaved changes flag
    let mut unsaved = state.project_has_unsaved_changes.lock().unwrap();
    *unsaved = true;
    drop(unsaved);

    // Emit event to notify frontend
    app.emit("project-changed", ProjectState {
        project: Some(project),
        has_unsaved_changes: true,
    })
    .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Closes the current project
#[tauri::command]
pub fn close_project(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let mut current = state.current_project.lock().unwrap();
    *current = None;
    drop(current);

    let mut current_path = state.current_project_path.lock().unwrap();
    *current_path = None;
    drop(current_path);

    // Clear unsaved changes flag
    let mut unsaved = state.project_has_unsaved_changes.lock().unwrap();
    *unsaved = false;
    drop(unsaved);

    // Emit event to notify frontend
    app.emit("project-changed", ProjectState {
        project: None,
        has_unsaved_changes: false,
    })
    .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Opens a project from file using file picker
#[tauri::command]
pub fn open_project(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    // Show open file dialog
    let file_path = app.dialog()
        .file()
        .set_title("Open project")
        .add_filter("JSON", &["json"])
        .blocking_pick_file();

    // Check if user selected a file
    let path = match file_path {
        Some(p) => p.to_string(),
        None => return Ok(()), // User cancelled
    };

    // Read file content
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Deserialize project
    let project: Project = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse project file: {}", e))?;

    // Check schema compatibility
    check_schema_compatibility(&project.schema)?;

    // Update app state
    let mut current = state.current_project.lock().unwrap();
    *current = Some(project.clone());
    drop(current);

    let mut current_path = state.current_project_path.lock().unwrap();
    *current_path = Some(path);
    drop(current_path);

    // Clear unsaved changes flag
    let mut unsaved = state.project_has_unsaved_changes.lock().unwrap();
    *unsaved = false;
    drop(unsaved);

    // Emit event to notify frontend
    app.emit("project-changed", ProjectState {
        project: Some(project),
        has_unsaved_changes: false,
    })
    .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Saves the current project to file
/// If save_as is true or no path exists, shows save dialog
#[tauri::command]
pub fn save_project(app: AppHandle, state: State<AppState>, save_as: Option<bool>) -> Result<(), String> {
    let save_as = save_as.unwrap_or(false);

    // Check if we need to show the save dialog
    let current_path_guard = state.current_project_path.lock().unwrap();
    let needs_dialog = save_as || current_path_guard.is_none();
    drop(current_path_guard);

    let path = if needs_dialog {
        // Get current project to determine default filename
        let current = state.current_project.lock().unwrap();
        let project = current.as_ref()
            .ok_or_else(|| "No project currently open".to_string())?;

        // Determine default filename from project title
        let default_filename = project.title.as_ref()
            .map(|t| format!("{}.json", t))
            .unwrap_or_else(|| "untitled.json".to_string());

        drop(current); // Release the lock before showing dialog

        // Show save file dialog
        let file_path = app.dialog()
            .file()
            .set_title("Save project as")
            .set_file_name(&default_filename)
            .add_filter("JSON", &["json"])
            .blocking_save_file();

        // Check if user selected a file
        match file_path {
            Some(p) => p.to_string(),
            None => return Ok(()), // User cancelled
        }
    } else {
        // Use existing path
        let current_path = state.current_project_path.lock().unwrap();
        current_path.as_ref()
            .ok_or_else(|| "No file path set".to_string())?
            .clone()
    };

    // Get current project for saving
    let mut current = state.current_project.lock().unwrap();
    let project = current.as_mut()
        .ok_or_else(|| "No project currently open".to_string())?;

    // Serialize to JSON with pretty formatting
    let json = serde_json::to_string_pretty(project)
        .map_err(|e| format!("Failed to serialize project: {}", e))?;

    // Write to file
    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    drop(current); // Release project lock

    // Update current project path in app state
    let mut current_path = state.current_project_path.lock().unwrap();
    *current_path = Some(path);
    drop(current_path);

    // Clear unsaved changes flag
    let mut unsaved = state.project_has_unsaved_changes.lock().unwrap();
    *unsaved = false;
    drop(unsaved);

    // Emit event to notify frontend of the updated project
    let current = state.current_project.lock().unwrap();
    if let Some(project) = current.as_ref() {
        app.emit("project-changed", ProjectState {
            project: Some(project.clone()),
            has_unsaved_changes: false,
        })
        .map_err(|e| format!("Failed to emit event: {}", e))?;
    }

    Ok(())
}

