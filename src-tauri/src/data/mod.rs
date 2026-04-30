//! Data layer: project file structures, BOM database helpers, and recent-projects list.

pub mod bom;
pub mod nextdb;
pub mod project_data;
pub mod recent_projects;

pub use bom::{BomEntry, Metadata, create_database, insert_bom_entries, insert_metadata, parse_csv, resolve_bom_entries, update_resolution_metadata};
pub use nextdb::{NextdbMetadata, list_alt_tables, read_nextdb_metadata};
pub use project_data::{Project, ProjectState};
pub use recent_projects::{RecentProject, RecentProjects};
