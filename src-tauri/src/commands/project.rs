use crate::data::{create_database, insert_bom_entries, insert_metadata, parse_csv, resolve_bom_entries, Metadata, Project, ProjectState, RecentProject, RecentProjects};
use crate::AppState;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

// ── Recent-projects persistence helpers ──────────────────────────────────────

/// Returns the path to `recent_projects.json` inside the app config directory.
fn recent_projects_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("recent_projects.json"))
        .map_err(|e| format!("Failed to resolve config dir: {}", e))
}

/// Loads the recent projects list from disk, returning a default empty list on any error.
///
/// Errors (missing file, parse failure) are silently swallowed so a missing or corrupt
/// persistence file never prevents the app from starting.
pub fn load_recent_from_disk(app: &AppHandle) -> RecentProjects {
    let Ok(path) = recent_projects_path(app) else {
        return RecentProjects::default();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return RecentProjects::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Serialises `recent` to JSON and writes it to the app config directory.
///
/// Creates the config directory if it does not exist. Returns an error only if the
/// directory cannot be created or the file cannot be written — serialisation failures
/// are not expected for this type.
fn persist_recent(app: &AppHandle, recent: &RecentProjects) -> Result<(), String> {
    let path = recent_projects_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config dir: {}", e))?;
    }
    let json = serde_json::to_string_pretty(recent)
        .map_err(|e| format!("Failed to serialise recent projects: {}", e))?;
    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write recent projects: {}", e))?;
    Ok(())
}

/// Emits a `recent-projects-changed` event carrying the current items list.
fn emit_recent_changed(app: &AppHandle, recent: &RecentProjects) {
    let _ = app.emit("recent-projects-changed", recent.items.clone());
}

// ── Schema compatibility ──────────────────────────────────────────────────────

/// Returns `Ok(())` if `file_schema` is compatible with the running application version.
///
/// Compatibility is determined by comparing the **major** version component only: a file created
/// with v1.x can be opened by any v1.y build, but not by v2.x. Both `file_schema` and the
/// package version (`CARGO_PKG_VERSION`) must be valid semver strings; invalid formats return
/// an error.
fn check_schema_compatibility(file_schema: &str) -> Result<(), String> {
    let current_version = env!("CARGO_PKG_VERSION");

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

    if file_major != current_major {
        return Err(format!(
            "Incompatible project file version: {} (current software version: {}). Please update the application or use a compatible project file.",
            file_schema,
            current_version
        ));
    }

    Ok(())
}

// ── Project commands ──────────────────────────────────────────────────────────

/// Returns a snapshot of the current [`ProjectState`] (open project + unsaved-changes flag).
///
/// Called by frontend components on mount to bootstrap their local state before the first
/// `project-changed` event arrives.
#[tauri::command]
pub fn get_project_state(state: State<AppState>) -> ProjectState {
    let inner = state.inner.lock().unwrap();
    ProjectState {
        project: inner.current_project.clone(),
        has_unsaved_changes: inner.has_unsaved_changes,
    }
}

/// Creates a new in-memory project and makes it the active project.
///
/// `title` is required; `engineer`, `project_specifics`, and `design_variant` are set only when
/// non-empty. The new project has no file path — it must be saved before a path is assigned.
/// Emits `project-changed` with `has_unsaved_changes: true` on success.
#[tauri::command]
pub fn create_project(
    app: AppHandle,
    title: String,
    engineer: Option<String>,
    project_specifics: Option<String>,
    design_variant: Option<String>,
    state: State<AppState>,
) -> Result<(), String> {
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

    if let Some(variant) = design_variant {
        if !variant.is_empty() {
            project.set_design_variant(variant);
        }
    }

    let mut inner = state.inner.lock().unwrap();
    inner.current_project = Some(project.clone());
    inner.current_project_path = None;
    inner.has_unsaved_changes = true;
    let snapshot = ProjectState { project: Some(project), has_unsaved_changes: true };
    drop(inner);

    app.emit("project-changed", snapshot)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Clears all project state and emits `project-changed` with `project: null`.
///
/// Resets the open project, its file path, and the unsaved-changes flag. The caller is
/// responsible for prompting the user to save before invoking this command.
#[tauri::command]
pub fn close_project(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let mut inner = state.inner.lock().unwrap();
    inner.current_project = None;
    inner.current_project_path = None;
    inner.has_unsaved_changes = false;
    let snapshot = ProjectState { project: None, has_unsaved_changes: false };
    drop(inner);

    app.emit("project-changed", snapshot)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Opens a project from `path`, or presents a file-open dialog when `path` is `None`.
///
/// Returns `Ok(())` without changing state if the user cancels the dialog. When a path is
/// provided and the file is not found, the project is removed from the recent list before
/// returning an error. Validates schema compatibility before accepting the file. On success,
/// adds the project to the recent list, persists it, and emits both `project-changed` and
/// `recent-projects-changed`.
#[tauri::command]
pub async fn open_project(
    app: AppHandle,
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<(), String> {
    let path = if let Some(p) = path {
        p
    } else {
        let dialog = app.dialog()
            .file()
            .set_title("Open project")
            .add_filter("NextBOM Project", &["nbp"]);

        let file_path = tauri::async_runtime::spawn_blocking(move || {
            dialog.blocking_pick_file()
        }).await.map_err(|e| e.to_string())?;

        match file_path {
            Some(p) => p.to_string(),
            None => return Ok(()),
        }
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                let mut inner = state.inner.lock().unwrap();
                inner.recent_projects.remove(&path);
                let recent = inner.recent_projects.clone();
                drop(inner);
                let _ = persist_recent(&app, &recent);
                emit_recent_changed(&app, &recent);
            }
            return Err(format!("Failed to read file: {}", e));
        }
    };

    let project: Project = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse project file: {}", e))?;

    check_schema_compatibility(&project.schema)?;

    let mut inner = state.inner.lock().unwrap();
    inner.current_project = Some(project.clone());
    inner.current_project_path = Some(path.clone());
    inner.has_unsaved_changes = false;
    inner.recent_projects.add(path, project.title.clone());
    let recent = inner.recent_projects.clone();
    let snapshot = ProjectState { project: Some(project), has_unsaved_changes: false };
    drop(inner);

    let _ = persist_recent(&app, &recent);
    emit_recent_changed(&app, &recent);
    app.emit("project-changed", snapshot)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Writes the current project to disk as pretty-printed JSON.
///
/// Shows a save-file dialog when `save_as` is `true` or when the project has no existing path.
/// Returns `Ok(())` without saving if the user cancels the dialog. The default filename is
/// derived from the project title. Adds the saved path to the recent list, persists it, and
/// emits both `recent-projects-changed` and `project-changed` on success.
#[tauri::command]
pub async fn save_project(app: AppHandle, state: State<'_, AppState>, save_as: Option<bool>) -> Result<(), String> {
    let save_as = save_as.unwrap_or(false);

    // Read necessary info before the dialog await point
    let (needs_dialog, default_filename, existing_path) = {
        let inner = state.inner.lock().unwrap();
        let needs = save_as || inner.current_project_path.is_none();
        let existing = inner.current_project_path.clone();
        let filename = if needs {
            inner.current_project.as_ref()
                .ok_or_else(|| "No project currently open".to_string())?
                .title.as_ref()
                .map(|t| format!("{}.nbp", t))
                .unwrap_or_else(|| "untitled.nbp".to_string())
        } else {
            String::new()
        };
        (needs, filename, existing)
    };

    let path = if needs_dialog {
        let dialog = app.dialog()
            .file()
            .set_title("Save project as")
            .set_file_name(&default_filename)
            .add_filter("NextBOM Project", &["nbp"]);

        let file_path = tauri::async_runtime::spawn_blocking(move || {
            dialog.blocking_save_file()
        }).await.map_err(|e| e.to_string())?;

        match file_path {
            Some(p) => p.to_string(),
            None => return Ok(()),
        }
    } else {
        existing_path.ok_or_else(|| "No file path set".to_string())?
    };

    // Serialize while holding the lock briefly
    let json = {
        let inner = state.inner.lock().unwrap();
        let project = inner.current_project.as_ref()
            .ok_or_else(|| "No project currently open".to_string())?;
        serde_json::to_string_pretty(project)
            .map_err(|e| format!("Failed to serialize project: {}", e))?
    };

    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    let mut inner = state.inner.lock().unwrap();
    inner.current_project_path = Some(path.clone());
    inner.has_unsaved_changes = false;
    let title = inner.current_project.as_ref().and_then(|p| p.title.clone());
    inner.recent_projects.add(path, title);
    let recent = inner.recent_projects.clone();
    let snapshot = ProjectState {
        project: inner.current_project.clone(),
        has_unsaved_changes: false,
    };
    drop(inner);

    let _ = persist_recent(&app, &recent);
    emit_recent_changed(&app, &recent);
    app.emit("project-changed", snapshot)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Sets the title of the open project, marks it as unsaved, and emits `project-changed`.
///
/// Returns an error if no project is currently open.
#[tauri::command]
pub fn set_project_title(app: AppHandle, title: String, state: State<AppState>) -> Result<(), String> {
    let mut inner = state.inner.lock().unwrap();
    let project_clone = {
        let project = inner.current_project.as_mut()
            .ok_or_else(|| "No project currently open".to_string())?;
        project.set_title(title);
        project.clone()
    };
    inner.has_unsaved_changes = true;
    let snapshot = ProjectState { project: Some(project_clone), has_unsaved_changes: true };
    drop(inner);

    app.emit("project-changed", snapshot)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Sets the engineer name of the open project, marks it as unsaved, and emits `project-changed`.
///
/// Returns an error if no project is currently open.
#[tauri::command]
pub fn set_project_engineer(app: AppHandle, engineer: String, state: State<AppState>) -> Result<(), String> {
    let mut inner = state.inner.lock().unwrap();
    let project_clone = {
        let project = inner.current_project.as_mut()
            .ok_or_else(|| "No project currently open".to_string())?;
        project.set_engineer(engineer);
        project.clone()
    };
    inner.has_unsaved_changes = true;
    let snapshot = ProjectState { project: Some(project_clone), has_unsaved_changes: true };
    drop(inner);

    app.emit("project-changed", snapshot)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Returns the list of `alt_`-prefixed table names in the `.nextdb` parts database linked to the
/// open project. These tables represent project-specific alternative part sets.
///
/// Returns an error if no project is open, no database is linked, or the file cannot be opened.
#[tauri::command]
pub fn get_parts_tables(state: State<AppState>) -> Result<Vec<String>, String> {
    let inner = state.inner.lock().unwrap();
    let db_path = inner.current_project.as_ref()
        .ok_or_else(|| "No project currently open".to_string())?
        .database_path.as_ref()
        .ok_or_else(|| "No parts database linked to this project".to_string())?
        .clone();
    drop(inner);

    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("Failed to open parts database: {}", e))?;

    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'alt_%' ORDER BY name")
        .map_err(|e| format!("Failed to query tables: {}", e))?;

    let tables: Result<Vec<String>, _> = stmt.query_map([], |row| row.get(0))
        .map_err(|e| format!("Failed to read tables: {}", e))?
        .collect();

    tables.map_err(|e| format!("Failed to read table name: {}", e))
}

/// Sets the project-specifics identifier of the open project, marks it as unsaved, and emits
/// `project-changed`.
///
/// Returns an error if no project is currently open.
#[tauri::command]
pub fn set_project_specifics(app: AppHandle, project_specifics: String, state: State<AppState>) -> Result<(), String> {
    let mut inner = state.inner.lock().unwrap();
    let project_clone = {
        let project = inner.current_project.as_mut()
            .ok_or_else(|| "No project currently open".to_string())?;
        project.set_project_specifics(project_specifics);
        project.clone()
    };
    inner.has_unsaved_changes = true;
    let snapshot = ProjectState { project: Some(project_clone), has_unsaved_changes: true };
    drop(inner);

    app.emit("project-changed", snapshot)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Sets the design variant of the open project, marks it as unsaved, and emits `project-changed`.
///
/// Returns an error if no project is currently open.
#[tauri::command]
pub fn set_design_variant(app: AppHandle, design_variant: String, state: State<AppState>) -> Result<(), String> {
    let mut inner = state.inner.lock().unwrap();
    let project_clone = {
        let project = inner.current_project.as_mut()
            .ok_or_else(|| "No project currently open".to_string())?;
        project.set_design_variant(design_variant);
        project.clone()
    };
    inner.has_unsaved_changes = true;
    let snapshot = ProjectState { project: Some(project_clone), has_unsaved_changes: true };
    drop(inner);

    app.emit("project-changed", snapshot)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Presents a file-open dialog to select a `.nextdb` parts database and sets it on the open project.
///
/// Returns `Ok(())` without changing state if the user cancels. Marks the project as unsaved
/// and emits `project-changed` on success. Returns an error if no project is currently open.
#[tauri::command]
pub async fn set_database_path(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let dialog = app.dialog()
        .file()
        .set_title("Select Parts Database File")
        .add_filter("Parts Database", &["nextdb"]);

    let file_path = tauri::async_runtime::spawn_blocking(move || {
        dialog.blocking_pick_file()
    }).await.map_err(|e| e.to_string())?;

    let path = match file_path {
        Some(p) => p.to_string(),
        None => return Ok(()),
    };

    let mut inner = state.inner.lock().unwrap();
    let project_clone = {
        let project = inner.current_project.as_mut()
            .ok_or_else(|| "No project currently open".to_string())?;
        project.set_database_path(path);
        project.clone()
    };
    inner.has_unsaved_changes = true;
    let snapshot = ProjectState { project: Some(project_clone), has_unsaved_changes: true };
    drop(inner);

    app.emit("project-changed", snapshot)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Presents a file-open dialog to select a semicolon-delimited CSV file, validates its format,
/// and stores the path for a subsequent `create_nextbom_file` call.
///
/// Returns `Err("No file selected")` if the user cancels the dialog, or a parse error if the
/// CSV is malformed. On success, returns the entry count and the filename stem (without
/// extension), which the frontend uses as the default PCBA name.
#[tauri::command]
pub async fn load_csv(app: AppHandle, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let csv_dialog = app.dialog()
        .file()
        .set_title("Select CSV file")
        .add_filter("CSV", &["csv"]);

    let csv_path = tauri::async_runtime::spawn_blocking(move || {
        csv_dialog.blocking_pick_file()
    }).await.map_err(|e| e.to_string())?;

    let csv_path = match csv_path {
        Some(p) => p.to_string(),
        None => return Err("No file selected".to_string()),
    };

    let entries = parse_csv(Path::new(&csv_path))?;

    if entries.is_empty() {
        return Err("CSV file contains no data".to_string());
    }

    let filename_stem = Path::new(&csv_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    state.inner.lock().unwrap().pending_csv_path = Some(csv_path);

    Ok(serde_json::json!({
        "message": format!("Imported {} lines from CSV", entries.len()),
        "filename_stem": filename_stem
    }))
}

/// Creates a new `.nextbom` SQLite database from the CSV file previously loaded with `load_csv`.
///
/// Presents a save-file dialog to choose the output location. Returns `Err("No CSV loaded")`
/// if `load_csv` has not been called, or `Err("No save location selected")` if the user
/// cancels the dialog. On success, writes the BOM and metadata tables and returns a summary
/// string. The `.nextbom` file is a standalone working file and is not linked to the project.
#[tauri::command]
pub async fn create_nextbom_file(
    app: AppHandle,
    state: State<'_, AppState>,
    pcb_name: String,
    version: String,
    design_variant: String,
) -> Result<String, String> {
    let csv_path = {
        let inner = state.inner.lock().unwrap();
        inner.pending_csv_path.clone()
            .ok_or_else(|| "No CSV loaded".to_string())?
    };

    let entries = parse_csv(Path::new(&csv_path))?;

    let default_db_name = format!("{}.nextbom", pcb_name);

    let db_dialog = app.dialog()
        .file()
        .set_title("Save file as")
        .set_file_name(&default_db_name)
        .add_filter("NextBOM working file", &["nextbom"]);

    let db_path = tauri::async_runtime::spawn_blocking(move || {
        db_dialog.blocking_save_file()
    }).await.map_err(|e| e.to_string())?;

    let db_path = match db_path {
        Some(p) => p.to_string(),
        None => return Err("No save location selected".to_string()),
    };

    let conn = create_database(Path::new(&db_path))
        .map_err(|e| format!("Failed to create database: {}", e))?;

    insert_bom_entries(&conn, &entries)
        .map_err(|e| format!("Failed to insert BOM entries: {}", e))?;

    insert_metadata(&conn, &Metadata { pcb_name, design_variant: design_variant.clone(), version })
        .map_err(|e| format!("Failed to write metadata: {}", e))?;

    // Store variant in project so it can be auto-loaded next time
    let snapshot = {
        let mut inner = state.inner.lock().unwrap();
        if inner.current_project.is_some() {
            let project = inner.current_project.as_mut().unwrap();
            project.set_design_variant(design_variant);
            let project_clone = project.clone();
            inner.has_unsaved_changes = true;
            Some(ProjectState { project: Some(project_clone), has_unsaved_changes: true })
        } else {
            None
        }
    };
    if let Some(snapshot) = snapshot {
        let _ = app.emit("project-changed", snapshot);
    }

    Ok(format!("Successfully created NextBOM file with {} entries", entries.len()))
}

// ── BOM resolution commands ───────────────────────────────────────────────────

/// Presents a file-open dialog to select a `.nextbom` working file, then resolves every BOM
/// entry against the parts database linked to the open project.
///
/// Migrates the `.nextbom` schema to add `mfr`, `mpn`, `alt_mfr`, and `alt_mpn` columns, then
/// populates them with JSON arrays built from the `parts` table (and the project-specific
/// `alt_*` table, if set).
/// Returns `Ok(())` without changing state if the user cancels the dialog. Returns an error
/// if no project is open, no parts database is linked, or resolution fails.
#[tauri::command]
pub async fn resolve_bom_manufacturers(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let (db_path, project_specifics) = {
        let inner = state.inner.lock().unwrap();
        let project = inner.current_project.as_ref()
            .ok_or_else(|| "No project currently open".to_string())?;
        let db_path = project.database_path.clone()
            .ok_or_else(|| "No parts database linked to this project".to_string())?;
        let specifics = project.project_specifics.clone();
        (db_path, specifics)
    };

    let dialog = app.dialog()
        .file()
        .set_title("Select NextBOM working file")
        .add_filter("NextBOM working file", &["nextbom"]);

    let nextbom_path = tauri::async_runtime::spawn_blocking(move || {
        dialog.blocking_pick_file()
    }).await.map_err(|e| e.to_string())?;

    let nextbom_path = match nextbom_path {
        Some(p) => p.to_string(),
        None => return Err("No file selected".to_string()),
    };

    let nextbom_conn = rusqlite::Connection::open(&nextbom_path)
        .map_err(|e| format!("Failed to open .nextbom file: {}", e))?;
    let nextdb_conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("Failed to open parts database: {}", e))?;

    let count = resolve_bom_entries(&nextbom_conn, &nextdb_conn, project_specifics.as_deref())?;

    Ok(format!("Resolved {} entries", count))
}

// ── Recent projects commands ──────────────────────────────────────────────────

/// Returns the current list of recently opened projects, most recent first.
#[tauri::command]
pub fn get_recent_projects(state: State<AppState>) -> Vec<RecentProject> {
    state.inner.lock().unwrap().recent_projects.items.clone()
}

/// Removes a single entry from the recent projects list by file path, persists, and emits
/// `recent-projects-changed`.
#[tauri::command]
pub fn remove_recent_project(
    app: AppHandle,
    state: State<AppState>,
    file_path: String,
) -> Result<(), String> {
    let mut inner = state.inner.lock().unwrap();
    inner.recent_projects.remove(&file_path);
    let recent = inner.recent_projects.clone();
    drop(inner);
    let _ = persist_recent(&app, &recent);
    emit_recent_changed(&app, &recent);
    Ok(())
}

/// Clears the entire recent projects list, persists the empty list, and emits
/// `recent-projects-changed`.
#[tauri::command]
pub fn clear_recent_projects(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let mut inner = state.inner.lock().unwrap();
    inner.recent_projects.clear();
    let recent = inner.recent_projects.clone();
    drop(inner);
    let _ = persist_recent(&app, &recent);
    emit_recent_changed(&app, &recent);
    Ok(())
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
