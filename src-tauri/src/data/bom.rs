use rusqlite::{Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

/// A single row in the BOM table, mapping a schematic designator to a generic part ID.
///
/// `designator` is the reference designator on the schematic (e.g. `R1`, `C3`); it uniquely
/// identifies a component placement. `part_id` is the designer-assigned generic identifier,
/// resolved to a manufacturer part number during BOM generation.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/BomEntry.ts")]
pub struct BomEntry {
    /// Schematic reference designator (e.g. `R1`, `C3`, `U12`). Unique within a BOM.
    pub designator: String,

    /// Designer-assigned generic part identifier, resolved to a manufacturer part number at
    /// BOM generation time.
    pub part_id: String,
}

/// Metadata stored in every `.nextbom` database file.
///
/// Describes the PCB assembly this BOM belongs to. `pcb_name` identifies the board,
/// `design_variant` distinguishes assembly variants of the same board (e.g. `"full"` vs
/// `"lite"`), and `version` is the BOM revision number.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/Metadata.ts")]
pub struct Metadata {
    /// Name of the PCB assembly this BOM describes.
    pub pcb_name: String,

    /// Assembly variant identifier, used to distinguish builds of the same board.
    pub design_variant: String,

    /// BOM revision number (e.g. `"1"`, `"2"`).
    pub version: String,
}

/// Opens (or creates) a SQLite database at `path` and ensures the `bom` and `metadata` schemas
/// exist.
///
/// Uses `CREATE TABLE IF NOT EXISTS`, so calling this on an existing `.nextbom` file is safe
/// and idempotent. Returns the open [`Connection`] on success.
pub fn create_database(path: &Path) -> SqliteResult<Connection> {
    let conn = Connection::open(path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS bom (
            designator TEXT PRIMARY KEY NOT NULL,
            part_id    TEXT NOT NULL
        )",
        [],
    )?;

    // Single-row constraint: only one metadata record is allowed per database.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS metadata (
            id             INTEGER PRIMARY KEY CHECK (id = 1),
            pcb_name       TEXT NOT NULL,
            design_variant TEXT NOT NULL,
            version        TEXT NOT NULL
        )",
        [],
    )?;

    Ok(conn)
}

/// Inserts `metadata` into the `metadata` table of `conn`.
///
/// Replaces any existing row (there can only be one). Returns an error if the insert fails.
pub fn insert_metadata(conn: &Connection, metadata: &Metadata) -> SqliteResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO metadata (id, pcb_name, design_variant, version)
         VALUES (1, ?1, ?2, ?3)",
        rusqlite::params![metadata.pcb_name, metadata.design_variant, metadata.version],
    )?;
    Ok(())
}

/// Inserts `entries` into the `bom` table of `conn`.
///
/// Rows are appended; the function does not deduplicate or check for existing data. Returns
/// `Ok(())` after all entries are written, or the first [`rusqlite::Error`] encountered.
pub fn insert_bom_entries(conn: &Connection, entries: &[BomEntry]) -> SqliteResult<()> {
    let mut stmt = conn.prepare("INSERT INTO bom (designator, part_id) VALUES (?1, ?2)")?;

    for entry in entries {
        stmt.execute([&entry.designator, &entry.part_id])?;
    }

    Ok(())
}

/// Parses a semicolon-delimited CSV file at `csv_path` and returns the BOM entries.
///
/// The file must have a header row (skipped automatically). Each data row must contain at least
/// two columns: column 0 is the part ID and column 1 is the designator. Leading/trailing
/// whitespace is trimmed from both fields. Rows with fewer than two columns return an error.
///
/// Returns an empty `Vec` if the file contains only the header row.
pub fn parse_csv(csv_path: &Path) -> Result<Vec<BomEntry>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .flexible(true) // Allow rows with fewer fields; we validate the minimum ourselves.
        .from_path(csv_path)
        .map_err(|e| format!("Failed to open CSV file: {}", e))?;

    let mut entries = Vec::new();

    for result in reader.records() {
        let record = result.map_err(|e| format!("Failed to parse CSV record: {}", e))?;

        if record.len() < 2 {
            return Err("CSV must have at least 2 columns (part ID and designator)".to_string());
        }

        entries.push(BomEntry {
            designator: record.get(1).unwrap_or("").trim().to_string(),
            part_id: record.get(0).unwrap_or("").trim().to_string(),
        });
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Writes `content` to a temp file and returns its path.
    fn write_temp_csv(content: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("nextbom_test_{}.csv", uuid::Uuid::new_v4()));
        let mut file = std::fs::File::create(&path).expect("create temp file");
        file.write_all(content.as_bytes()).expect("write temp file");
        path
    }

    #[test]
    fn parse_csv_valid_entries() {
        let path = write_temp_csv("part_id;designator\nCAP_100NF;C1\nRES_10K;R2\n");
        let entries = parse_csv(&path).expect("parse should succeed");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].part_id, "CAP_100NF");
        assert_eq!(entries[0].designator, "C1");
        assert_eq!(entries[1].part_id, "RES_10K");
        assert_eq!(entries[1].designator, "R2");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn parse_csv_trims_whitespace() {
        let path = write_temp_csv("part_id;designator\n  CAP_100NF ;  C1  \n");
        let entries = parse_csv(&path).expect("parse should succeed");
        assert_eq!(entries[0].part_id, "CAP_100NF");
        assert_eq!(entries[0].designator, "C1");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn parse_csv_header_only_returns_empty() {
        let path = write_temp_csv("part_id;designator\n");
        let entries = parse_csv(&path).expect("parse should succeed");
        assert!(entries.is_empty());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn parse_csv_too_few_columns_returns_error() {
        let path = write_temp_csv("part_id;designator\nCAP_100NF\n");
        let result = parse_csv(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 2 columns"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn parse_csv_nonexistent_file_returns_error() {
        let path = std::path::Path::new("/nonexistent/path/to/file.csv");
        let result = parse_csv(path);
        assert!(result.is_err());
    }
}
