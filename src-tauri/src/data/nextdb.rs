use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Metadata stored in the `metadata` table of a `.nextdb` parts database.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/NextdbMetadata.ts")]
pub struct NextdbMetadata {
    /// User-assigned version string for this parts database.
    pub database_version: String,
}

/// Reads the metadata row from a `.nextdb` connection.
///
/// Returns `None` if the `metadata` table does not exist or contains no row — the table is
/// optional; older or externally-created databases may omit it.
pub fn read_nextdb_metadata(conn: &Connection) -> Result<Option<NextdbMetadata>, String> {
    let table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='metadata'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to check nextdb metadata table: {}", e))?;

    if table_exists == 0 {
        return Ok(None);
    }

    match conn.query_row(
        "SELECT database_version FROM metadata WHERE id = 1",
        [],
        |row| row.get::<_, String>(0),
    ) {
        Ok(database_version) => Ok(Some(NextdbMetadata { database_version })),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to read nextdb metadata: {}", e)),
    }
}

/// Returns the project-specifics identifiers available in this database — i.e. the names of all
/// `alt_*` tables with the `alt_` prefix stripped.
///
/// The returned strings are the values suitable for storing in `project_specifics` and passing
/// directly to `resolve_bom_entries`. The `alt_` prefix is an internal schema detail that callers
/// should not need to know about.
pub fn list_alt_tables(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'alt_%' ORDER BY name")
        .map_err(|e| format!("Failed to query alt tables: {}", e))?;

    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to read alt table names: {}", e))?;

    let mut names = Vec::new();
    for row in rows {
        let name = row.map_err(|e| format!("Failed to read table name: {}", e))?;
        names.push(name.trim_start_matches("alt_").to_string());
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn list_alt_tables_strips_prefix() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE parts (ID TEXT, mfr TEXT, mpn TEXT);
             CREATE TABLE alt_2025 (ID TEXT, mfr TEXT, mpn TEXT);
             CREATE TABLE alt_proto (ID TEXT, mfr TEXT, mpn TEXT);",
        ).unwrap();

        let tables = list_alt_tables(&conn).unwrap();
        assert_eq!(tables, vec!["2025", "proto"]);
    }

    #[test]
    fn list_alt_tables_excludes_non_alt_tables() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE parts (ID TEXT, mfr TEXT, mpn TEXT);
             CREATE TABLE metadata (id INTEGER PRIMARY KEY, database_version TEXT);",
        ).unwrap();

        let tables = list_alt_tables(&conn).unwrap();
        assert!(tables.is_empty());
    }

    #[test]
    fn list_alt_tables_returns_value_compatible_with_resolve() {
        // Verify the stripped name can directly index the alt table in resolve logic:
        // resolve_part_id does format!("alt_{}", specifics), so specifics must be WITHOUT prefix.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE parts (ID TEXT, mfr TEXT, mpn TEXT);
             CREATE TABLE alt_2025 (ID TEXT, mfr TEXT, mpn TEXT);",
        ).unwrap();

        let tables = list_alt_tables(&conn).unwrap();
        let specifics = &tables[0]; // "2025"
        let reconstructed = format!("alt_{}", specifics);
        assert_eq!(reconstructed, "alt_2025");
    }
}
