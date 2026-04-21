use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Payload emitted with every `project-changed` event and returned by `get_project_state`.
///
/// Frontend subscribers use this to keep their local state in sync after any state mutation.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/ProjectState.ts")]
pub struct ProjectState {
    /// The currently open project, or `None` when no project is loaded.
    pub project: Option<Project>,

    /// `true` when the in-memory project has changes that have not yet been written to disk.
    pub has_unsaved_changes: bool,
}

/// Represents a NextBOM project with all its properties
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/Project.ts")]
pub struct Project {
    /// Unique identifier for the project (UUID v4)
    pub uuid: String,

    /// Project title
    pub title: Option<String>,

    /// Absolute path to the linked `.nextdb` parts database file
    pub database_path: Option<String>,

    /// Name of the `alt_` table in the `.nextdb` database to append to the base `parts` table
    /// during BOM generation. Entries from both tables are combined as-is in the final BOM.
    pub project_specifics: Option<String>,

    /// Design variant identifier for this project (e.g. "full", "lite")
    pub design_variant: Option<String>,

    /// Name of the engineer responsible for the project
    pub engineer: Option<String>,

    /// Latest BOM version string
    pub latest_bom_version: Option<String>,

    /// Timestamp of the last change (Unix timestamp in milliseconds, UTC)
    /// Updates when BOM is created, project file is modified, or any change occurs
    pub last_change: i64,

    /// Schema version, package version with which the project file was created
    pub schema: String,
}

impl Project {
    /// Creates a new Project with default values
    pub fn new() -> Self {
        let now = chrono::Utc::now().timestamp_millis();

        Self {
            uuid: uuid::Uuid::new_v4().to_string(),
            title: None,
            database_path: None,
            project_specifics: None,
            design_variant: None,
            engineer: None,
            latest_bom_version: None,
            last_change: now,
            schema: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Updates the last_change timestamp to the current time
    pub fn touch(&mut self) {
        self.last_change = chrono::Utc::now().timestamp_millis();
    }

    /// Sets the project title and updates the last_change timestamp
    pub fn set_title(&mut self, title: String) {
        self.title = Some(title);
        self.touch();
    }

    /// Sets the `.nextdb` parts database path and updates the last_change timestamp
    pub fn set_database_path(&mut self, path: String) {
        self.database_path = Some(path);
        self.touch();
    }

    /// Sets the `alt_` table name used for project-specific parts and updates the last_change timestamp.
    /// Pass an empty string to clear the value.
    pub fn set_project_specifics(&mut self, project_specifics: String) {
        self.project_specifics = if project_specifics.is_empty() { None } else { Some(project_specifics) };
        self.touch();
    }

    /// Sets the design variant identifier and updates the last_change timestamp
    pub fn set_design_variant(&mut self, design_variant: String) {
        self.design_variant = Some(design_variant);
        self.touch();
    }

    /// Sets the engineer name and updates the last_change timestamp
    pub fn set_engineer(&mut self, engineer: String) {
        self.engineer = Some(engineer);
        self.touch();
    }

    /// Sets the latest BOM version and updates the last_change timestamp
    pub fn set_latest_bom_version(&mut self, version: String) {
        self.latest_bom_version = Some(version);
        self.touch();
    }
}
