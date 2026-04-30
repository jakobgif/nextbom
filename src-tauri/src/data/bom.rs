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
    pub bom_version: String,

    /// Absolute path to the CSV file that was imported to create this BOM.
    pub source_csv_path: String,

    /// Unix timestamp in milliseconds when the CSV was parsed.
    pub csv_imported_at: i64,
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
            id               INTEGER PRIMARY KEY CHECK (id = 1),
            pcb_name         TEXT NOT NULL,
            design_variant   TEXT NOT NULL,
            bom_version      TEXT NOT NULL,
            source_csv_path  TEXT NOT NULL,
            csv_imported_at  INTEGER NOT NULL,
            project_specifics TEXT,
            database_path    TEXT,
            database_version TEXT,
            resolved_at      INTEGER
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
        "INSERT OR REPLACE INTO metadata (id, pcb_name, design_variant, bom_version, source_csv_path, csv_imported_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            metadata.pcb_name,
            metadata.design_variant,
            metadata.bom_version,
            metadata.source_csv_path,
            metadata.csv_imported_at,
        ],
    )?;
    Ok(())
}

/// Updates the resolution columns of the single metadata row: which alternative set was used,
/// which database file was queried, and what version that database reports.
///
/// Called after [`resolve_bom_entries`] completes. All three values are nullable — pass `None`
/// when the information is unavailable (e.g. no `project_specifics` set, or the database has
/// no metadata table).
pub fn update_resolution_metadata(
    conn: &Connection,
    database_path: &str,
    project_specifics: Option<&str>,
    database_version: Option<&str>,
    resolved_at: i64,
) -> SqliteResult<()> {
    // Migrate older files that pre-date the resolved_at column.
    let _ = conn.execute("ALTER TABLE metadata ADD COLUMN resolved_at INTEGER", []);

    conn.execute(
        "UPDATE metadata SET database_path = ?1, project_specifics = ?2, database_version = ?3, resolved_at = ?4 WHERE id = 1",
        rusqlite::params![database_path, project_specifics, database_version, resolved_at],
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

        let part_id = record.get(0).unwrap_or("").trim().to_string();
        let designator = record.get(1).unwrap_or("").trim().to_string();

        if part_id.is_empty() || designator.is_empty() {
            return Err(format!(
                "Row {} has an empty value; both part ID and designator are required",
                entries.len() + 1
            ));
        }

        entries.push(BomEntry { designator, part_id });
    }

    Ok(entries)
}

/// Resolution result for a single part ID.
///
/// `mfr`/`mpn` come from the main `parts` table; `alt_mfr`/`alt_mpn` come from the
/// project-specific `alt_*` table (empty when no alt table is configured or the part has no
/// alt entries). When `alt_mfr` is non-empty, `part_id` has the project-specifics suffix
/// appended (e.g. `RES00100_2025`).
struct ResolvedPart {
    part_id: String,
    mfr: Vec<String>,
    mpn: Vec<String>,
    alt_mfr: Vec<String>,
    alt_mpn: Vec<String>,
}

/// Adds `mfr`, `mpn`, `alt_mfr`, and `alt_mpn` columns to the `bom` table if not present.
///
/// Safe to call on an already-migrated table — checks `pragma_table_info` before issuing
/// each `ALTER TABLE`. Returns an error if the schema query or migration fails.
pub fn migrate_bom_for_resolution(conn: &Connection) -> SqliteResult<()> {
    let col_exists = |name: &str| -> SqliteResult<bool> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('bom') WHERE name=?1",
            [name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    };

    for col in ["mfr", "mpn", "alt_mfr", "alt_mpn"] {
        if !col_exists(col)? {
            conn.execute(&format!("ALTER TABLE bom ADD COLUMN {} TEXT", col), [])?;
        }
    }

    Ok(())
}

/// Queries `nextdb_conn` for all manufacturer/MPN pairs matching `part_id`.
///
/// Results from the main `parts` table are returned in `mfr`/`mpn`. If `project_specifics`
/// is `Some(s)` and the table `alt_s` exists, its matches are returned separately in
/// `alt_mfr`/`alt_mpn`, and `part_id` has `_{project_specifics}` appended
/// (e.g. `RES00100_2025`).
fn resolve_part_id(
    nextdb_conn: &Connection,
    part_id: &str,
    project_specifics: Option<&str>,
) -> Result<ResolvedPart, String> {
    let mut mfr: Vec<String> = Vec::new();
    let mut mpn: Vec<String> = Vec::new();

    {
        let mut stmt = nextdb_conn
            .prepare("SELECT mfr, mpn FROM parts WHERE ID = ?1")
            .map_err(|e| format!("Failed to query parts table: {}", e))?;

        let rows = stmt
            .query_map([part_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("Failed to read parts rows: {}", e))?;

        for row in rows {
            let (m, p) = row.map_err(|e| format!("Failed to read row: {}", e))?;
            mfr.push(m);
            mpn.push(p);
        }
    }

    let mut alt_mfr: Vec<String> = Vec::new();
    let mut alt_mpn: Vec<String> = Vec::new();

    if let Some(specifics) = project_specifics {
        let alt_table = format!("alt_{}", specifics);

        let table_exists: i64 = nextdb_conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [&alt_table],
                |row| row.get(0),
            )
            .map_err(|e| format!("Failed to check alt table existence: {}", e))?;

        if table_exists > 0 {
            let query = format!("SELECT mfr, mpn FROM {} WHERE ID = ?1", alt_table);
            let mut stmt = nextdb_conn
                .prepare(&query)
                .map_err(|e| format!("Failed to query alt table: {}", e))?;

            let rows = stmt
                .query_map([part_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|e| format!("Failed to read alt rows: {}", e))?;

            for row in rows {
                let (m, p) = row.map_err(|e| format!("Failed to read alt row: {}", e))?;
                alt_mfr.push(m);
                alt_mpn.push(p);
            }
        }
    }

    let resolved_part_id = if !alt_mfr.is_empty() {
        format!("{}_{}", part_id, project_specifics.unwrap())
    } else {
        part_id.to_string()
    };

    Ok(ResolvedPart { part_id: resolved_part_id, mfr, mpn, alt_mfr, alt_mpn })
}

/// Resolves all BOM entries in `nextbom_conn` against the parts database in `nextdb_conn`.
///
/// Migrates the `bom` table to add `mfr`, `mpn`, `alt_mfr`, and `alt_mpn` columns (if not
/// present), then populates them with JSON arrays. Main-table results go into `mfr`/`mpn`;
/// alt-table results go into `alt_mfr`/`alt_mpn`. Part IDs with alt matches have the
/// project-specifics suffix appended.
///
/// Returns the number of rows updated.
pub fn resolve_bom_entries(
    nextbom_conn: &Connection,
    nextdb_conn: &Connection,
    project_specifics: Option<&str>,
) -> Result<usize, String> {
    migrate_bom_for_resolution(nextbom_conn)
        .map_err(|e| format!("Failed to migrate bom schema: {}", e))?;

    let mut stmt = nextbom_conn
        .prepare("SELECT designator, part_id FROM bom")
        .map_err(|e| format!("Failed to read bom entries: {}", e))?;

    let entries: Vec<(String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("Failed to iterate bom: {}", e))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Failed to collect bom entries: {}", e))?;

    let count = entries.len();

    for (designator, part_id) in &entries {
        let resolved = resolve_part_id(nextdb_conn, part_id, project_specifics)?;

        let mfr_json = serde_json::to_string(&resolved.mfr)
            .map_err(|e| format!("Failed to serialize mfr: {}", e))?;
        let mpn_json = serde_json::to_string(&resolved.mpn)
            .map_err(|e| format!("Failed to serialize mpn: {}", e))?;
        let alt_mfr_json = serde_json::to_string(&resolved.alt_mfr)
            .map_err(|e| format!("Failed to serialize alt_mfr: {}", e))?;
        let alt_mpn_json = serde_json::to_string(&resolved.alt_mpn)
            .map_err(|e| format!("Failed to serialize alt_mpn: {}", e))?;

        nextbom_conn
            .execute(
                "UPDATE bom SET part_id = ?1, mfr = ?2, mpn = ?3, alt_mfr = ?4, alt_mpn = ?5 WHERE designator = ?6",
                rusqlite::params![resolved.part_id, mfr_json, mpn_json, alt_mfr_json, alt_mpn_json, designator],
            )
            .map_err(|e| format!("Failed to update bom entry: {}", e))?;
    }

    Ok(count)
}

/// A BOM row with resolved manufacturer and MPN data, suitable for display.
///
/// After resolution the `mfr`/`mpn` arrays are populated from the main parts database and
/// `alt_mfr`/`alt_mpn` from the project-specific alt table. Before resolution all four are
/// empty.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/ResolvedBomEntry.ts")]
pub struct ResolvedBomEntry {
    pub designator: String,
    pub part_id: String,
    pub mfr: Vec<String>,
    pub mpn: Vec<String>,
    pub alt_mfr: Vec<String>,
    pub alt_mpn: Vec<String>,
}

/// Reads all rows from the `bom` table of `conn`, returning them as [`ResolvedBomEntry`]
/// values sorted by designator.
///
/// When the resolution columns (`mfr`, `mpn`, `alt_mfr`, `alt_mpn`) are absent (pre-resolution
/// file), the four `Vec` fields are returned empty.
pub fn read_resolved_bom(conn: &Connection) -> SqliteResult<Vec<ResolvedBomEntry>> {
    let has_resolution_cols: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('bom') WHERE name='mfr'",
        [],
        |row| row.get(0),
    )?;

    if has_resolution_cols > 0 {
        let mut stmt = conn.prepare(
            "SELECT designator, part_id, mfr, mpn, alt_mfr, alt_mpn FROM bom ORDER BY designator",
        )?;
        let rows: Vec<_> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        let parse = |s: Option<String>| -> Vec<String> {
            s.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default()
        };

        Ok(rows
            .into_iter()
            .map(|(designator, part_id, mfr_j, mpn_j, alt_mfr_j, alt_mpn_j)| ResolvedBomEntry {
                designator,
                part_id,
                mfr: parse(mfr_j),
                mpn: parse(mpn_j),
                alt_mfr: parse(alt_mfr_j),
                alt_mpn: parse(alt_mpn_j),
            })
            .collect())
    } else {
        let mut stmt =
            conn.prepare("SELECT designator, part_id FROM bom ORDER BY designator")?;
        let entries = stmt
            .query_map([], |row| {
                Ok(ResolvedBomEntry {
                    designator: row.get(0)?,
                    part_id: row.get(1)?,
                    mfr: vec![],
                    mpn: vec![],
                    alt_mfr: vec![],
                    alt_mpn: vec![],
                })
            })?
            .collect::<SqliteResult<Vec<_>>>();
        entries
    }
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
    fn parse_csv_empty_part_id_returns_error() {
        let path = write_temp_csv("part_id;designator\n;C1\n");
        let result = parse_csv(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty value"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn parse_csv_empty_designator_returns_error() {
        let path = write_temp_csv("part_id;designator\nCAP_100NF;\n");
        let result = parse_csv(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty value"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn parse_csv_nonexistent_file_returns_error() {
        let path = std::path::Path::new("/nonexistent/path/to/file.csv");
        let result = parse_csv(path);
        assert!(result.is_err());
    }

    fn make_nextdb(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE parts (ID TEXT, mfr TEXT, mpn TEXT);
             INSERT INTO parts VALUES ('CAP00100', 'TDK', 'C0402X5R');
             INSERT INTO parts VALUES ('CAP00100', 'Murata', 'GRM0332');
             INSERT INTO parts VALUES ('RES00100', 'Yageo', 'RC0402');",
        )
        .unwrap();
    }

    fn make_nextbom(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE bom (designator TEXT PRIMARY KEY NOT NULL, part_id TEXT NOT NULL);
             INSERT INTO bom VALUES ('C1', 'CAP00100');
             INSERT INTO bom VALUES ('R1', 'RES00100');",
        )
        .unwrap();
    }

    #[test]
    fn migrate_adds_columns() {
        let conn = Connection::open_in_memory().unwrap();
        make_nextbom(&conn);
        migrate_bom_for_resolution(&conn).unwrap();

        for col in ["mfr", "mpn", "alt_mfr", "alt_mpn"] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('bom') WHERE name=?1",
                    [col],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "column {col} should exist");
        }
    }

    #[test]
    fn migrate_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        make_nextbom(&conn);
        migrate_bom_for_resolution(&conn).unwrap();
        migrate_bom_for_resolution(&conn).unwrap();
    }

    #[test]
    fn resolve_part_id_returns_main_rows() {
        let db = Connection::open_in_memory().unwrap();
        make_nextdb(&db);

        let r = resolve_part_id(&db, "CAP00100", None).unwrap();
        assert_eq!(r.part_id, "CAP00100");
        assert_eq!(r.mfr, vec!["TDK", "Murata"]);
        assert_eq!(r.mpn, vec!["C0402X5R", "GRM0332"]);
        assert!(r.alt_mfr.is_empty());
        assert!(r.alt_mpn.is_empty());
    }

    #[test]
    fn resolve_part_id_no_match_returns_empty_arrays() {
        let db = Connection::open_in_memory().unwrap();
        make_nextdb(&db);

        let r = resolve_part_id(&db, "UNKNOWN", None).unwrap();
        assert_eq!(r.part_id, "UNKNOWN");
        assert!(r.mfr.is_empty());
        assert!(r.mpn.is_empty());
        assert!(r.alt_mfr.is_empty());
        assert!(r.alt_mpn.is_empty());
    }

    #[test]
    fn resolve_part_id_with_alt_separates_and_appends_suffix() {
        let db = Connection::open_in_memory().unwrap();
        make_nextdb(&db);
        db.execute_batch(
            "CREATE TABLE alt_2025 (ID TEXT, mfr TEXT, mpn TEXT);
             INSERT INTO alt_2025 VALUES ('RES00100', 'Vishay', 'CRCW0402');",
        )
        .unwrap();

        let r = resolve_part_id(&db, "RES00100", Some("2025")).unwrap();
        assert_eq!(r.part_id, "RES00100_2025");
        assert_eq!(r.mfr, vec!["Yageo"]);
        assert_eq!(r.mpn, vec!["RC0402"]);
        assert_eq!(r.alt_mfr, vec!["Vishay"]);
        assert_eq!(r.alt_mpn, vec!["CRCW0402"]);
    }

    #[test]
    fn resolve_part_id_alt_table_missing_does_not_append_suffix() {
        let db = Connection::open_in_memory().unwrap();
        make_nextdb(&db);

        let r = resolve_part_id(&db, "RES00100", Some("nonexistent")).unwrap();
        assert_eq!(r.part_id, "RES00100");
        assert!(r.alt_mfr.is_empty());
    }

    #[test]
    fn resolve_bom_entries_updates_all_rows() {
        let nextbom = Connection::open_in_memory().unwrap();
        let nextdb = Connection::open_in_memory().unwrap();
        make_nextbom(&nextbom);
        make_nextdb(&nextdb);

        let count = resolve_bom_entries(&nextbom, &nextdb, None).unwrap();
        assert_eq!(count, 2);

        let mfr: String = nextbom
            .query_row("SELECT mfr FROM bom WHERE designator = 'C1'", [], |r| r.get(0))
            .unwrap();
        let mpn: String = nextbom
            .query_row("SELECT mpn FROM bom WHERE designator = 'C1'", [], |r| r.get(0))
            .unwrap();
        let alt_mfr: Option<String> = nextbom
            .query_row("SELECT alt_mfr FROM bom WHERE designator = 'C1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mfr, r#"["TDK","Murata"]"#);
        assert_eq!(mpn, r#"["C0402X5R","GRM0332"]"#);
        assert_eq!(alt_mfr.as_deref(), Some("[]"));
    }

    #[test]
    fn resolve_bom_entries_populates_alt_columns_when_project_specifics_set() {
        let nextbom = Connection::open_in_memory().unwrap();
        let nextdb = Connection::open_in_memory().unwrap();
        make_nextbom(&nextbom);
        make_nextdb(&nextdb);
        nextdb.execute_batch(
            "CREATE TABLE alt_2025 (ID TEXT, mfr TEXT, mpn TEXT);
             INSERT INTO alt_2025 VALUES ('RES00100', 'Vishay', 'CRCW0402');",
        ).unwrap();

        let count = resolve_bom_entries(&nextbom, &nextdb, Some("2025")).unwrap();
        assert_eq!(count, 2);

        // R1 has an alt entry — alt columns should be populated and part_id suffixed
        let part_id: String = nextbom
            .query_row("SELECT part_id FROM bom WHERE designator = 'R1'", [], |r| r.get(0))
            .unwrap();
        let alt_mfr: String = nextbom
            .query_row("SELECT alt_mfr FROM bom WHERE designator = 'R1'", [], |r| r.get(0))
            .unwrap();
        let alt_mpn: String = nextbom
            .query_row("SELECT alt_mpn FROM bom WHERE designator = 'R1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(part_id, "RES00100_2025");
        assert_eq!(alt_mfr, r#"["Vishay"]"#);
        assert_eq!(alt_mpn, r#"["CRCW0402"]"#);

        // C1 has no alt entry — alt columns should be empty arrays, part_id unchanged
        let part_id: String = nextbom
            .query_row("SELECT part_id FROM bom WHERE designator = 'C1'", [], |r| r.get(0))
            .unwrap();
        let alt_mfr: String = nextbom
            .query_row("SELECT alt_mfr FROM bom WHERE designator = 'C1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(part_id, "CAP00100");
        assert_eq!(alt_mfr, r#"[]"#);
    }
}
