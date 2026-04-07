use crate::data::{create_database, insert_bom_entries, parse_csv, Project, ProjectState};
use crate::AppState;
use std::path::Path;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

/// Returns `Ok(())` if `file_schema` is compatible with the running application version.
///
/// Compatibility is determined by comparing the **major** version component only: a file created
/// with v1.x can be opened by any v1.y build, but not by v2.x. Both `file_schema` and the
/// package version (`CARGO_PKG_VERSION`) must be valid semver strings; invalid formats return
/// an error.
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

/// Returns a snapshot of the current [`ProjectState`] (open project + unsaved-changes flag).
///
/// Called by frontend components on mount to bootstrap their local state before the first
/// `project-changed` event arrives.
#[tauri::command]
pub fn get_project_state(state: State<AppState>) -> ProjectState {
    let current = state.current_project.lock().unwrap();
    let unsaved = state.project_has_unsaved_changes.lock().unwrap();

    ProjectState {
        project: current.clone(),
        has_unsaved_changes: *unsaved,
    }
}

/// Creates a new in-memory project and makes it the active project.
///
/// `title` is required; `engineer` and `project_specifics` are set only when non-empty. The new
/// project has no file path — it must be saved before a path is assigned. Emits `project-changed`
/// with `has_unsaved_changes: true` on success.
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

    // Clear project path (new project has no file location yet)
    let mut current_path = state.current_project_path.lock().unwrap();
    *current_path = None;
    drop(current_path);

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

/// Clears all project state and emits `project-changed` with `project: null`.
///
/// Resets the open project, its file path, and the unsaved-changes flag. The caller is
/// responsible for prompting the user to save before invoking this command.
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

/// Presents a file-open dialog and loads the selected `.json` project file.
///
/// Returns `Ok(())` without changing state if the user cancels the dialog. Validates schema
/// compatibility before accepting the file. On success, replaces the active project and emits
/// `project-changed` with `has_unsaved_changes: false`.
#[tauri::command]
pub async fn open_project(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // Show open file dialog (run in background thread to avoid blocking UI)
    let dialog = app.dialog()
        .file()
        .set_title("Open project")
        .add_filter("JSON", &["json"]);

    let file_path = tauri::async_runtime::spawn_blocking(move || {
        dialog.blocking_pick_file()
    }).await.map_err(|e| e.to_string())?;

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

/// Writes the current project to disk as pretty-printed JSON.
///
/// Shows a save-file dialog when `save_as` is `true` or when the project has no existing path.
/// Returns `Ok(())` without saving if the user cancels the dialog. The default filename is
/// derived from the project title. Clears the unsaved-changes flag and emits `project-changed`
/// on success.
#[tauri::command]
pub async fn save_project(app: AppHandle, state: State<'_, AppState>, save_as: Option<bool>) -> Result<(), String> {
    let save_as = save_as.unwrap_or(false);

    // Check if we need to show the save dialog and get default filename
    // Use scoping blocks to ensure MutexGuards are dropped before await
    let (needs_dialog, default_filename, existing_path) = {
        let current_path_guard = state.current_project_path.lock().unwrap();
        let needs = save_as || current_path_guard.is_none();
        let existing = current_path_guard.clone();

        let filename = if needs {
            let current = state.current_project.lock().unwrap();
            current.as_ref()
                .ok_or_else(|| "No project currently open".to_string())?
                .title.as_ref()
                .map(|t| format!("{}.json", t))
                .unwrap_or_else(|| "untitled.json".to_string())
        } else {
            String::new()
        };

        (needs, filename, existing)
    };

    let path = if needs_dialog {
        // Show save file dialog (run in background thread to avoid blocking UI)
        let dialog = app.dialog()
            .file()
            .set_title("Save project as")
            .set_file_name(&default_filename)
            .add_filter("JSON", &["json"]);

        let file_path = tauri::async_runtime::spawn_blocking(move || {
            dialog.blocking_save_file()
        }).await.map_err(|e| e.to_string())?;

        // Check if user selected a file
        match file_path {
            Some(p) => p.to_string(),
            None => return Ok(()), // User cancelled
        }
    } else {
        // Use existing path
        existing_path.ok_or_else(|| "No file path set".to_string())?
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

/// Sets the title of the open project, marks it as unsaved, and emits `project-changed`.
///
/// Returns an error if no project is currently open.
#[tauri::command]
pub fn set_project_title(app: AppHandle, title: String, state: State<AppState>) -> Result<(), String> {
    // Get and update current project
    let mut current = state.current_project.lock().unwrap();
    let project = current.as_mut()
        .ok_or_else(|| "No project currently open".to_string())?;

    project.set_title(title);
    let updated_project = project.clone();
    drop(current);

    // Set unsaved changes flag
    let mut unsaved = state.project_has_unsaved_changes.lock().unwrap();
    *unsaved = true;
    drop(unsaved);

    // Emit event to notify frontend
    app.emit("project-changed", ProjectState {
        project: Some(updated_project),
        has_unsaved_changes: true,
    })
    .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Sets the engineer name of the open project, marks it as unsaved, and emits `project-changed`.
///
/// Returns an error if no project is currently open.
#[tauri::command]
pub fn set_project_engineer(app: AppHandle, engineer: String, state: State<AppState>) -> Result<(), String> {
    // Get and update current project
    let mut current = state.current_project.lock().unwrap();
    let project = current.as_mut()
        .ok_or_else(|| "No project currently open".to_string())?;

    project.set_engineer(engineer);
    let updated_project = project.clone();
    drop(current);

    // Set unsaved changes flag
    let mut unsaved = state.project_has_unsaved_changes.lock().unwrap();
    *unsaved = true;
    drop(unsaved);

    // Emit event to notify frontend
    app.emit("project-changed", ProjectState {
        project: Some(updated_project),
        has_unsaved_changes: true,
    })
    .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Sets the project-specifics identifier of the open project, marks it as unsaved, and emits
/// `project-changed`.
///
/// Returns an error if no project is currently open.
#[tauri::command]
pub fn set_project_specifics(app: AppHandle, project_specifics: String, state: State<AppState>) -> Result<(), String> {
    // Get and update current project
    let mut current = state.current_project.lock().unwrap();
    let project = current.as_mut()
        .ok_or_else(|| "No project currently open".to_string())?;

    project.set_project_specifics(project_specifics);
    let updated_project = project.clone();
    drop(current);

    // Set unsaved changes flag
    let mut unsaved = state.project_has_unsaved_changes.lock().unwrap();
    *unsaved = true;
    drop(unsaved);

    // Emit event to notify frontend
    app.emit("project-changed", ProjectState {
        project: Some(updated_project),
        has_unsaved_changes: true,
    })
    .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Presents a file-open dialog to select a `.nextbom` database and sets it on the open project.
///
/// Returns `Ok(())` without changing state if the user cancels. Marks the project as unsaved
/// and emits `project-changed` on success. Returns an error if no project is currently open.
#[tauri::command]
pub async fn set_database_path(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // Show open file dialog (run in background thread to avoid blocking UI)
    let dialog = app.dialog()
        .file()
        .set_title("Select NextBOM Database File")
        .add_filter("NextBOM Database", &["nextbom"]);

    let file_path = tauri::async_runtime::spawn_blocking(move || {
        dialog.blocking_pick_file()
    }).await.map_err(|e| e.to_string())?;

    // Check if user selected a file
    let path = match file_path {
        Some(p) => p.to_string(),
        None => return Ok(()), // User cancelled
    };

    // Get and update current project
    let mut current = state.current_project.lock().unwrap();
    let project = current.as_mut()
        .ok_or_else(|| "No project currently open".to_string())?;

    project.set_database_path(path);
    let updated_project = project.clone();
    drop(current);

    // Set unsaved changes flag
    let mut unsaved = state.project_has_unsaved_changes.lock().unwrap();
    *unsaved = true;
    drop(unsaved);

    // Emit event to notify frontend
    app.emit("project-changed", ProjectState {
        project: Some(updated_project),
        has_unsaved_changes: true,
    })
    .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Imports a semicolon-delimited CSV file into a new `.nextbom` SQLite database.
///
/// Presents two dialogs in sequence: first a file-open dialog to select the source CSV, then a
/// save-file dialog to choose the output database location. The default database filename is
/// derived from the CSV filename. Returns `Err("No file selected")` or
/// `Err("No save location selected")` if the user cancels either dialog.
///
/// On success, the new database path is set on the open project (if any), the unsaved-changes
/// flag is set, `project-changed` is emitted, and a summary string is returned
/// (e.g. `"Successfully imported 42 entries to /path/to/bom.nextbom"`).
#[tauri::command]
pub async fn import_csv_to_database(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    // Show open file dialog for CSV
    let csv_dialog = app.dialog()
        .file()
        .set_title("Select CSV file")
        .add_filter("CSV", &["csv"]);

    let csv_path = tauri::async_runtime::spawn_blocking(move || {
        csv_dialog.blocking_pick_file()
    }).await.map_err(|e| e.to_string())?;

    // Check if user selected a file
    let csv_path = match csv_path {
        Some(p) => p.to_string(),
        None => return Err("No file selected".to_string()),
    };

    // Parse CSV file
    let entries = parse_csv(Path::new(&csv_path))?;

    if entries.is_empty() {
        return Err("CSV file contains no data".to_string());
    }

    // Get default filename from CSV filename
    let csv_stem = Path::new(&csv_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("bom");
    let default_db_name = format!("{}.nextbom", csv_stem);

    // Show save file dialog for SQLite database
    let db_dialog = app.dialog()
        .file()
        .set_title("Save NextBOM database as")
        .set_file_name(&default_db_name)
        .add_filter("NextBOM Database", &["nextbom"]);

    let db_path = tauri::async_runtime::spawn_blocking(move || {
        db_dialog.blocking_save_file()
    }).await.map_err(|e| e.to_string())?;

    // Check if user selected a location
    let db_path = match db_path {
        Some(p) => p.to_string(),
        None => return Err("No save location selected".to_string()),
    };

    // Create database and insert entries
    let conn = create_database(Path::new(&db_path))
        .map_err(|e| format!("Failed to create database: {}", e))?;

    insert_bom_entries(&conn, &entries)
        .map_err(|e| format!("Failed to insert BOM entries: {}", e))?;

    // Update current project with database path
    let mut current = state.current_project.lock().unwrap();
    if let Some(project) = current.as_mut() {
        project.set_database_path(db_path.clone());
        let updated_project = project.clone();
        drop(current);

        // Set unsaved changes flag
        let mut unsaved = state.project_has_unsaved_changes.lock().unwrap();
        *unsaved = true;
        drop(unsaved);

        // Emit event to notify frontend
        app.emit("project-changed", ProjectState {
            project: Some(updated_project),
            has_unsaved_changes: true,
        })
        .map_err(|e| format!("Failed to emit event: {}", e))?;
    }

    Ok(format!("Successfully imported {} entries to {}", entries.len(), db_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

    fn current_major() -> u32 {
        CURRENT_VERSION
            .split('.')
            .next()
            .and_then(|v| v.parse().ok())
            .expect("package version must be valid semver")
    }

    #[test]
    fn compatible_same_major_version() {
        let major = current_major();
        let schema = format!("{}.5.3", major);
        assert!(check_schema_compatibility(&schema).is_ok());
    }

    #[test]
    fn compatible_exact_current_version() {
        assert!(check_schema_compatibility(CURRENT_VERSION).is_ok());
    }

    #[test]
    fn incompatible_higher_major_version() {
        let major = current_major() + 1;
        let schema = format!("{}.0.0", major);
        let result = check_schema_compatibility(&schema);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("Incompatible"));
    }

    #[test]
    fn incompatible_lower_major_version_when_current_is_nonzero() {
        let major = current_major();
        if major == 0 {
            // Can't test a lower major when we're already at 0 — skip.
            return;
        }
        let schema = format!("{}.99.0", major - 1);
        assert!(check_schema_compatibility(&schema).is_err());
    }

    #[test]
    fn invalid_schema_format_returns_error() {
        let result = check_schema_compatibility("not-a-version");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid schema version format"));
    }

    #[test]
    fn empty_schema_string_returns_error() {
        let result = check_schema_compatibility("");
        assert!(result.is_err());
    }
}
