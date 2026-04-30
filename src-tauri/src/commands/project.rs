use crate::data::{apply_bom_template, create_database, group_for_excel, insert_bom_entries, insert_metadata, list_alt_tables, parse_csv, read_nextdb_metadata, read_resolved_bom, resolve_bom_entries, update_resolution_metadata, write_default_xlsx, Metadata, Project, ProjectState, RecentProject, RecentProjects, ResolvedBomEntry};
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
    database_path: Option<String>,
    bom_template_path: Option<String>,
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

    if let Some(db_path) = database_path {
        if !db_path.is_empty() {
            project.set_database_path(db_path);
        }
    }

    if let Some(tmpl_path) = bom_template_path {
        if !tmpl_path.is_empty() {
            project.set_bom_template_path(Some(tmpl_path));
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
        let window = app.get_webview_window("main")
            .ok_or_else(|| "main window not found".to_string())?;
        let dialog = app.dialog()
            .file()
            .set_title("Open project")
            .add_filter("NextBOM Project", &["nbp"])
            .set_parent(&window);

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
        let window = app.get_webview_window("main")
            .ok_or_else(|| "main window not found".to_string())?;
        let dialog = app.dialog()
            .file()
            .set_title("Save project as")
            .set_file_name(&default_filename)
            .add_filter("NextBOM Project", &["nbp"])
            .set_parent(&window);

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

    list_alt_tables(&conn)
}

/// Returns the project-specifics identifiers from a parts database file at the given path.
///
/// Used when creating a new project before any project is open.
#[tauri::command]
pub fn get_parts_tables_from_path(database_path: String) -> Result<Vec<String>, String> {
    let conn = rusqlite::Connection::open(&database_path)
        .map_err(|e| format!("Failed to open parts database: {}", e))?;

    list_alt_tables(&conn)
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
    let window = app.get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let dialog = app.dialog()
        .file()
        .set_title("Select Parts Database File")
        .add_filter("Parts Database", &["nextdb"])
        .set_parent(&window);

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
    let window = app.get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let csv_dialog = app.dialog()
        .file()
        .set_title("Select CSV file")
        .add_filter("CSV", &["csv"])
        .set_parent(&window);

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

    let path_for_response = csv_path.clone();
    state.inner.lock().unwrap().pending_csv_path = Some(csv_path);

    Ok(serde_json::json!({
        "message": format!("Imported {} lines from CSV", entries.len()),
        "filename_stem": filename_stem,
        "csv_path": path_for_response
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
    bom_version: String,
    design_variant: String,
) -> Result<serde_json::Value, String> {
    let csv_path = {
        let inner = state.inner.lock().unwrap();
        inner.pending_csv_path.clone()
            .ok_or_else(|| "No CSV loaded".to_string())?
    };

    let entries = parse_csv(Path::new(&csv_path))?;

    let default_db_name = format!("{}.nextbom", pcb_name);

    let window = app.get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let db_dialog = app.dialog()
        .file()
        .set_title("Save file as")
        .set_file_name(&default_db_name)
        .add_filter("NextBOM working file", &["nextbom"])
        .set_parent(&window);

    let db_path = tauri::async_runtime::spawn_blocking(move || {
        db_dialog.blocking_save_file()
    }).await.map_err(|e| e.to_string())?;

    let db_path = match db_path {
        Some(p) => p.to_string(),
        None => return Err("No save location selected".to_string()),
    };

    if Path::new(&db_path).exists() {
        std::fs::remove_file(&db_path)
            .map_err(|e| format!("Failed to overwrite existing file: {}", e))?;
    }

    let conn = create_database(Path::new(&db_path))
        .map_err(|e| format!("Failed to create database: {}", e))?;

    insert_bom_entries(&conn, &entries)
        .map_err(|e| format!("Failed to insert BOM entries: {}", e))?;

    let csv_imported_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let engineer = state
        .inner
        .lock()
        .unwrap()
        .current_project
        .as_ref()
        .and_then(|p| p.engineer.clone())
        .unwrap_or_default();

    insert_metadata(&conn, &Metadata {
        pcb_name,
        design_variant: design_variant.clone(),
        bom_version,
        source_csv_path: csv_path.clone(),
        csv_imported_at,
        engineer,
    })
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

    state.inner.lock().unwrap().pending_nextbom_path = Some(db_path.clone());

    Ok(serde_json::json!({
        "message": format!("Successfully created NextBOM file with {} entries", entries.len()),
        "nextbom_path": db_path
    }))
}

// ── BOM resolution commands ───────────────────────────────────────────────────

#[tauri::command]
pub fn get_database_info(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db_path = {
        let inner = state.inner.lock().unwrap();
        let project = inner.current_project.as_ref()
            .ok_or_else(|| "No project currently open".to_string())?;
        project.database_path.clone()
            .ok_or_else(|| "No parts database linked to this project".to_string())?
    };

    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    let meta = read_nextdb_metadata(&conn)?;
    let alts = list_alt_tables(&conn)?;

    Ok(serde_json::json!({
        "database_version": meta.map(|m| m.database_version),
        "available_alternatives": alts
    }))
}

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
    use_pending: bool,
) -> Result<serde_json::Value, String> {
    let (db_path, project_specifics, pending_nextbom_path) = {
        let inner = state.inner.lock().unwrap();
        let project = inner.current_project.as_ref()
            .ok_or_else(|| "No project currently open".to_string())?;
        let db_path = project.database_path.clone()
            .ok_or_else(|| "No parts database linked to this project".to_string())?;
        let specifics = project.project_specifics.clone();
        (db_path, specifics, inner.pending_nextbom_path.clone())
    };

    let nextbom_path = if use_pending {
        if let Some(path) = pending_nextbom_path {
            path
        } else {
            return Err("No NextBOM file from step 1 available".to_string());
        }
    } else {
        let window = app.get_webview_window("main")
            .ok_or_else(|| "main window not found".to_string())?;
        let dialog = app.dialog()
            .file()
            .set_title("Select NextBOM working file")
            .add_filter("NextBOM working file", &["nextbom"])
            .set_parent(&window);

        let picked = tauri::async_runtime::spawn_blocking(move || {
            dialog.blocking_pick_file()
        }).await.map_err(|e| e.to_string())?;

        match picked {
            Some(p) => p.to_string(),
            None => return Err("No file selected".to_string()),
        }
    };

    let nextbom_conn = rusqlite::Connection::open(&nextbom_path)
        .map_err(|e| format!("Failed to open .nextbom file: {}", e))?;
    let nextdb_conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("Failed to open parts database: {}", e))?;

    let nextdb_meta = read_nextdb_metadata(&nextdb_conn)?;
    let database_version = nextdb_meta.as_ref().map(|m| m.database_version.as_str());

    let count = resolve_bom_entries(&nextbom_conn, &nextdb_conn, project_specifics.as_deref())?;

    let resolved_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    update_resolution_metadata(&nextbom_conn, &db_path, project_specifics.as_deref(), database_version, resolved_at)
        .map_err(|e| format!("Failed to update resolution metadata: {}", e))?;

    Ok(serde_json::json!({
        "message": format!("Resolved {} entries", count),
        "nextbom_path": nextbom_path
    }))
}

/// Returns all rows from the `bom` table of the given `.nextbom` file, including resolved
/// manufacturer and MPN data if the file has been through step 2.
#[tauri::command]
pub fn get_resolved_bom(nextbom_path: String) -> Result<Vec<ResolvedBomEntry>, String> {
    let conn = rusqlite::Connection::open(&nextbom_path)
        .map_err(|e| format!("Failed to open .nextbom file: {}", e))?;
    read_resolved_bom(&conn).map_err(|e| format!("Failed to read BOM: {}", e))
}

/// Exports the resolved BOM from a `.nextbom` file to an Excel `.xlsx` file.
///
/// When `nextbom_path` is `None`, presents a file-open dialog to select the input file.
/// If the open project has a `bom_template_path` set, the template is filled with BOM data;
/// otherwise a default blank-sheet layout is generated. Always presents a save-file dialog.
/// Returns `Err("cancelled")` if the user dismisses either dialog.
#[tauri::command]
pub async fn export_bom_to_excel(
    app: AppHandle,
    state: State<'_, AppState>,
    nextbom_path: Option<String>,
) -> Result<String, String> {
    // Resolve input path — open file picker if not provided
    let nextbom_path = if let Some(p) = nextbom_path {
        p
    } else {
        let window = app.get_webview_window("main")
            .ok_or_else(|| "main window not found".to_string())?;
        let dialog = app.dialog()
            .file()
            .set_title("Select resolved NextBOM file")
            .add_filter("NextBOM working file", &["nextbom"])
            .set_parent(&window);
        let picked = tauri::async_runtime::spawn_blocking(move || {
            dialog.blocking_pick_file()
        }).await.map_err(|e| e.to_string())?;
        match picked {
            Some(p) => p.to_string(),
            None => return Err("cancelled".to_string()),
        }
    };

    // Read BOM data and metadata (all sync before any await).
    // Per the export rule, every value used in the output must come from the .nextbom file.
    let (default_filename, rows, template_path, pcb_name, bom_version, design_variant, engineer, creation_date) = {
        let conn = rusqlite::Connection::open(&nextbom_path)
            .map_err(|e| format!("Failed to open .nextbom file: {}", e))?;

        // Migrate older `.nextbom` files that pre-date the engineer column.
        let _ = conn.execute("ALTER TABLE metadata ADD COLUMN engineer TEXT", []);

        let (pcb_name, bom_version, design_variant, engineer, csv_imported_at, default_filename) = conn
            .query_row(
                "SELECT pcb_name, bom_version, design_variant, engineer, csv_imported_at FROM metadata WHERE id = 1",
                [],
                |row| Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                )),
            )
            .map(|(n, v, d, e, t)| {
                let filename = format!("{}_v{}.xlsx", n, v);
                (n, v, d, e.unwrap_or_default(), t, filename)
            })
            .unwrap_or_else(|_| (String::new(), String::new(), String::new(), String::new(), 0, "bom.xlsx".to_string()));

        let creation_date = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(csv_imported_at)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_default();

        let entries = read_resolved_bom(&conn)
            .map_err(|e| format!("Failed to read BOM: {}", e))?;
        let rows = group_for_excel(&entries);

        let template_path = state.inner.lock().unwrap()
            .current_project
            .as_ref()
            .and_then(|p| p.bom_template_path.clone());

        (default_filename, rows, template_path, pcb_name, bom_version, design_variant, engineer, creation_date)
    };

    // Save-file dialog
    let window = app.get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let dialog = app.dialog()
        .file()
        .set_title("Save BOM as Excel")
        .set_file_name(&default_filename)
        .add_filter("Excel Workbook", &["xlsx"])
        .set_parent(&window);

    let save_path = tauri::async_runtime::spawn_blocking(move || {
        dialog.blocking_save_file()
    }).await.map_err(|e| e.to_string())?;

    let save_path = match save_path {
        Some(p) => p.to_string(),
        None => return Err("cancelled".to_string()),
    };

    // Generate the Excel file
    if let Some(tmpl) = template_path {
        let book = apply_bom_template(
            Path::new(&tmpl),
            &rows,
            &pcb_name,
            &bom_version,
            &design_variant,
            &engineer,
            &creation_date,
        )?;
        umya_spreadsheet::writer::xlsx::write(&book, Path::new(&save_path))
            .map_err(|e| e.to_string())?;
        restore_hf_drawing(Path::new(&tmpl), Path::new(&save_path))?;
    } else {
        write_default_xlsx(Path::new(&save_path), &rows)?;
    }

    Ok(save_path)
}

/// Sets the BOM export template for the open project.
///
/// Presents a file-open dialog to select an `.xlsx` template file. Returns `Ok(())` without
/// changing state if the user cancels. Marks the project as unsaved and emits `project-changed`.
#[tauri::command]
pub async fn set_bom_template(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let window = app.get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let dialog = app.dialog()
        .file()
        .set_title("Select BOM Template")
        .add_filter("Excel Workbook", &["xlsx"])
        .set_parent(&window);

    let picked = tauri::async_runtime::spawn_blocking(move || {
        dialog.blocking_pick_file()
    }).await.map_err(|e| e.to_string())?;

    let path = match picked {
        Some(p) => p.to_string(),
        None => return Ok(()),
    };

    let mut inner = state.inner.lock().unwrap();
    let project_clone = {
        let project = inner.current_project.as_mut()
            .ok_or_else(|| "No project currently open".to_string())?;
        project.set_bom_template_path(Some(path));
        project.clone()
    };
    inner.has_unsaved_changes = true;
    let snapshot = ProjectState { project: Some(project_clone), has_unsaved_changes: true };
    drop(inner);

    app.emit("project-changed", snapshot)
        .map_err(|e| format!("Failed to emit event: {}", e))?;
    Ok(())
}

/// Clears the BOM export template from the open project, reverting to the default layout.
#[tauri::command]
pub fn clear_bom_template(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let mut inner = state.inner.lock().unwrap();
    let project_clone = {
        let project = inner.current_project.as_mut()
            .ok_or_else(|| "No project currently open".to_string())?;
        project.set_bom_template_path(None);
        project.clone()
    };
    inner.has_unsaved_changes = true;
    let snapshot = ProjectState { project: Some(project_clone), has_unsaved_changes: true };
    drop(inner);

    app.emit("project-changed", snapshot)
        .map_err(|e| format!("Failed to emit event: {}", e))?;
    Ok(())
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

// ── Header/footer drawing restoration ────────────────────────────────────────

/// Patches `output_path` to restore header/footer VML drawing data from `template_path`.
///
/// umya-spreadsheet drops `legacyDrawingHF` and all VML drawings when round-tripping an XLSX
/// file. This function copies the VML file, its image, and relationship entries from the
/// template ZIP back into the output ZIP, and wires them into the output's XML.
fn restore_hf_drawing(template_path: &Path, output_path: &Path) -> Result<(), String> {
    use std::io::Write;

    let tmpl = std::fs::read(template_path).map_err(|e| format!("read template: {e}"))?;
    let out = std::fs::read(output_path).map_err(|e| format!("read output: {e}"))?;

    // Find VML drawing relationships in template sheet rels
    let sheet_rels = read_zip_str(&tmpl, "xl/worksheets/_rels/sheet1.xml.rels")?;
    let vml_rels: Vec<_> = parse_zip_rels(&sheet_rels)
        .into_iter()
        .filter(|r| r.rel_type.ends_with("/vmlDrawing"))
        .collect();
    if vml_rels.is_empty() {
        return Ok(());
    }

    // Find legacyDrawingHF elements in template sheet1.xml
    let tmpl_sheet = read_zip_str(&tmpl, "xl/worksheets/sheet1.xml")?;
    let hf_elems = extract_legacy_drawing_hf(&tmpl_sheet);
    if hf_elems.is_empty() {
        return Ok(());
    }

    // Collect VML files, their _rels, and media to copy from the template
    let mut copy: Vec<(String, Vec<u8>)> = Vec::new();
    for rel in &vml_rels {
        let vml_path = resolve_zip_rel("xl/worksheets/", &rel.target);
        if let Ok(bytes) = read_zip_bytes(&tmpl, &vml_path) {
            let vml_rels_path = make_rels_path(&vml_path);
            if let Ok(vml_rels_bytes) = read_zip_bytes(&tmpl, &vml_rels_path) {
                let vml_rels_str = String::from_utf8_lossy(&vml_rels_bytes);
                for media in parse_zip_rels(&vml_rels_str) {
                    let dir = parent_dir(&vml_path);
                    let media_path = resolve_zip_rel(&dir, &media.target);
                    if let Ok(mb) = read_zip_bytes(&tmpl, &media_path) {
                        copy.push((media_path, mb));
                    }
                }
                copy.push((vml_rels_path, vml_rels_bytes));
            }
            copy.push((vml_path, bytes));
        }
    }

    // Read all output entries; determine safe new rIds
    let mut entries = read_all_zip_entries(&out)?;

    let out_rels = read_zip_str(&out, "xl/worksheets/_rels/sheet1.xml.rels").unwrap_or_default();
    let max_rid = parse_zip_rels(&out_rels)
        .iter()
        .filter_map(|r| r.id.strip_prefix("rId").and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(0);

    let mut rid_map = std::collections::HashMap::new();
    for (i, rel) in vml_rels.iter().enumerate() {
        rid_map.insert(rel.id.clone(), format!("rId{}", max_rid + 1 + i as u32));
    }

    // Patch sheet rels: add VML drawing relationships
    {
        let ns = "http://schemas.openxmlformats.org/package/2006/relationships";
        let default_rels = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{}"></Relationships>"#,
            ns
        );
        let rels = entries
            .entry("xl/worksheets/_rels/sheet1.xml.rels".to_string())
            .or_insert_with(|| default_rels.into_bytes());
        let mut xml = String::from_utf8_lossy(rels).into_owned();
        for rel in &vml_rels {
            let new_id = &rid_map[&rel.id];
            let entry = format!(
                r#"<Relationship Id="{}" Type="{}" Target="{}"/>"#,
                new_id, rel.rel_type, rel.target
            );
            xml = xml.replace("</Relationships>", &format!("{entry}</Relationships>"));
        }
        *rels = xml.into_bytes();
    }

    // Patch sheet1.xml: inject legacyDrawingHF before </worksheet>
    if let Some(sheet) = entries.get_mut("xl/worksheets/sheet1.xml") {
        let mut xml = String::from_utf8_lossy(sheet).into_owned();
        for elem in &hf_elems {
            let mut patched = elem.clone();
            for (old, new) in &rid_map {
                patched = patched.replace(
                    &format!(r#"r:id="{old}""#),
                    &format!(r#"r:id="{new}""#),
                );
            }
            xml = xml.replace("</worksheet>", &format!("{patched}</worksheet>"));
        }
        *sheet = xml.into_bytes();
    }

    // Patch [Content_Types].xml: add vml and media content types if missing
    if let Some(ct) = entries.get_mut("[Content_Types].xml") {
        let mut xml = String::from_utf8_lossy(ct).into_owned();
        let needed: &[(&str, &str)] = &[
            ("vml", "application/vnd.openxmlformats-officedocument.vmlDrawing"),
            ("png",  "image/png"),
            ("jpg",  "image/jpeg"),
            ("jpeg", "image/jpeg"),
            ("gif",  "image/gif"),
            ("tiff", "image/tiff"),
            ("emf",  "image/x-emf"),
        ];
        for (ext, mime) in needed {
            if !xml.contains(&format!(r#"Extension="{}""#, ext)) {
                xml = xml.replace(
                    "</Types>",
                    &format!(r#"<Default Extension="{}" ContentType="{}"/></Types>"#, ext, mime),
                );
            }
        }
        *ct = xml.into_bytes();
    }

    // Add VML and media files (skip if somehow already present)
    for (path, bytes) in copy {
        entries.entry(path).or_insert(bytes);
    }

    // Write patched ZIP back to output_path
    let mut buf = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in &entries {
            writer.start_file(name, options).map_err(|e| e.to_string())?;
            writer.write_all(bytes).map_err(|e| e.to_string())?;
        }
        writer.finish().map_err(|e| e.to_string())?;
    }
    std::fs::write(output_path, &buf).map_err(|e| format!("write patched output: {e}"))?;
    Ok(())
}

struct ZipRel { id: String, rel_type: String, target: String }

fn parse_zip_rels(xml: &str) -> Vec<ZipRel> {
    let mut rels = Vec::new();
    let mut s = xml;
    while let Some(start) = s.find("<Relationship") {
        let rest = &s[start..];
        let end = rest.find("/>").map(|i| i + 2)
            .or_else(|| rest.find("</Relationship>").map(|i| i + 15))
            .unwrap_or(rest.len());
        let elem = &rest[..end];
        let id = xml_attr(elem, "Id");
        let rel_type = xml_attr(elem, "Type");
        let target = xml_attr(elem, "Target");
        if !id.is_empty() && !rel_type.is_empty() {
            rels.push(ZipRel { id, rel_type, target });
        }
        s = &rest[end.min(rest.len())..];
    }
    rels
}

fn xml_attr(xml: &str, attr: &str) -> String {
    let pat = format!(r#"{}=""#, attr);
    if let Some(i) = xml.find(&pat) {
        let after = &xml[i + pat.len()..];
        if let Some(end) = after.find('"') {
            return after[..end].to_string();
        }
    }
    String::new()
}

fn extract_legacy_drawing_hf(sheet_xml: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut s = sheet_xml;
    while let Some(start) = s.find("<legacyDrawingHF") {
        let rest = &s[start..];
        if let Some(end) = rest.find("/>") {
            results.push(rest[..end + 2].to_string());
            s = &rest[end + 2..];
        } else {
            break;
        }
    }
    results
}

fn resolve_zip_rel(base_dir: &str, target: &str) -> String {
    let mut parts: Vec<&str> = base_dir.trim_end_matches('/').split('/').collect();
    for seg in target.split('/') {
        if seg == ".." { parts.pop(); } else { parts.push(seg); }
    }
    parts.join("/")
}

fn make_rels_path(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => format!("{}/_rels/{}.rels", &path[..i], &path[i + 1..]),
        None => format!("_rels/{}.rels", path),
    }
}

fn parent_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => format!("{}/", &path[..i]),
        None => String::new(),
    }
}

fn read_zip_str(zip_bytes: &[u8], name: &str) -> Result<String, String> {
    String::from_utf8(read_zip_bytes(zip_bytes, name)?).map_err(|e| e.to_string())
}

fn read_zip_bytes(zip_bytes: &[u8], name: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))
        .map_err(|e| e.to_string())?;
    let mut file = archive.by_name(name).map_err(|_| format!("not in zip: {name}"))?;
    let mut out = Vec::new();
    file.read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

fn read_all_zip_entries(zip_bytes: &[u8]) -> Result<std::collections::BTreeMap<String, Vec<u8>>, String> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))
        .map_err(|e| e.to_string())?;
    let mut map = std::collections::BTreeMap::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        if file.is_dir() { continue; }
        let name = file.name().to_string();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        map.insert(name, bytes);
    }
    Ok(map)
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
