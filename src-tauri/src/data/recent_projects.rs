use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Represents a recently opened project
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/RecentProject.ts")]
pub struct RecentProject {
    /// Full path to the project file
    pub file_path: String,

    /// Project title for display
    pub title: Option<String>,

    /// Last opened timestamp (Unix timestamp in milliseconds, UTC)
    pub last_opened: i64,
}

impl RecentProject {
    /// Creates a new RecentProject entry
    pub fn new(file_path: String, title: Option<String>) -> Self {
        Self {
            file_path,
            title,
            last_opened: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Updates the last_opened timestamp to now
    pub fn touch(&mut self) {
        self.last_opened = chrono::Utc::now().timestamp_millis();
    }
}

/// Container for managing recent projects list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentProjects {
    /// List of recent projects, sorted by last_opened (most recent first)
    pub items: Vec<RecentProject>,

    /// Maximum number of recent projects to keep
    pub max_items: usize,
}

impl RecentProjects {
    /// Creates a new RecentProjects list with a maximum size
    pub fn new(max_items: usize) -> Self {
        Self {
            items: Vec::new(),
            max_items,
        }
    }

    /// Adds or updates a recent project entry
    /// If the project already exists, it moves to the top with updated timestamp
    pub fn add(&mut self, file_path: String, title: Option<String>) {
        // Check if project already exists
        if let Some(pos) = self.items.iter().position(|p| p.file_path == file_path) {
            // Remove existing entry
            let mut existing = self.items.remove(pos);
            // Update timestamp and title
            existing.touch();
            if title.is_some() {
                existing.title = title;
            }
            // Add to front
            self.items.insert(0, existing);
        } else {
            // Add new entry at front
            self.items.insert(0, RecentProject::new(file_path, title));

            // Trim list to max_items
            if self.items.len() > self.max_items {
                self.items.truncate(self.max_items);
            }
        }
    }

    /// Removes a project from the recent list by file path
    pub fn remove(&mut self, file_path: &str) {
        self.items.retain(|p| p.file_path != file_path);
    }

    /// Clears all recent projects
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Gets the list of recent projects (most recent first)
    pub fn get_items(&self) -> &[RecentProject] {
        &self.items
    }
}

impl Default for RecentProjects {
    fn default() -> Self {
        Self::new(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_new_project_appears_at_front() {
        let mut rp = RecentProjects::new(10);
        rp.add("/projects/a.json".into(), Some("Alpha".into()));
        rp.add("/projects/b.json".into(), Some("Beta".into()));

        assert_eq!(rp.get_items().len(), 2);
        assert_eq!(rp.get_items()[0].file_path, "/projects/b.json");
        assert_eq!(rp.get_items()[1].file_path, "/projects/a.json");
    }

    #[test]
    fn add_existing_project_moves_to_front_and_updates_title() {
        let mut rp = RecentProjects::new(10);
        rp.add("/projects/a.json".into(), Some("Alpha".into()));
        rp.add("/projects/b.json".into(), Some("Beta".into()));
        // Re-open "a" with a new title
        rp.add("/projects/a.json".into(), Some("Alpha Renamed".into()));

        assert_eq!(rp.get_items().len(), 2);
        assert_eq!(rp.get_items()[0].file_path, "/projects/a.json");
        assert_eq!(rp.get_items()[0].title.as_deref(), Some("Alpha Renamed"));
        assert_eq!(rp.get_items()[1].file_path, "/projects/b.json");
    }

    #[test]
    fn add_existing_project_preserves_title_when_new_title_is_none() {
        let mut rp = RecentProjects::new(10);
        rp.add("/projects/a.json".into(), Some("Alpha".into()));
        rp.add("/projects/b.json".into(), Some("Beta".into()));
        rp.add("/projects/a.json".into(), None);

        assert_eq!(rp.get_items()[0].title.as_deref(), Some("Alpha"));
    }

    #[test]
    fn max_items_is_enforced() {
        let mut rp = RecentProjects::new(3);
        for i in 0..5 {
            rp.add(format!("/projects/{}.json", i), None);
        }
        assert_eq!(rp.get_items().len(), 3);
        // Most recent (index 4) should be at front
        assert_eq!(rp.get_items()[0].file_path, "/projects/4.json");
    }

    #[test]
    fn remove_deletes_matching_entry() {
        let mut rp = RecentProjects::new(10);
        rp.add("/projects/a.json".into(), None);
        rp.add("/projects/b.json".into(), None);
        rp.remove("/projects/a.json");

        assert_eq!(rp.get_items().len(), 1);
        assert_eq!(rp.get_items()[0].file_path, "/projects/b.json");
    }

    #[test]
    fn remove_nonexistent_path_is_a_noop() {
        let mut rp = RecentProjects::new(10);
        rp.add("/projects/a.json".into(), None);
        rp.remove("/does/not/exist.json");
        assert_eq!(rp.get_items().len(), 1);
    }

    #[test]
    fn clear_empties_list() {
        let mut rp = RecentProjects::new(10);
        rp.add("/projects/a.json".into(), None);
        rp.add("/projects/b.json".into(), None);
        rp.clear();
        assert!(rp.get_items().is_empty());
    }
}