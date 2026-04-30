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

    /// Name of the engineer responsible for this BOM, copied from the project at import time.
    pub engineer: String,
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
            resolved_at      INTEGER,
            engineer         TEXT
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
        "INSERT OR REPLACE INTO metadata (id, pcb_name, design_variant, bom_version, source_csv_path, csv_imported_at, engineer)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            metadata.pcb_name,
            metadata.design_variant,
            metadata.bom_version,
            metadata.source_csv_path,
            metadata.csv_imported_at,
            metadata.engineer,
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
/// alt entries).
struct ResolvedPart {
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
/// `alt_mfr`/`alt_mpn`. The original `part_id` is never modified — the suffix that signals
/// "this row has an alternative" is applied only at Excel export time.
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

    Ok(ResolvedPart { mfr, mpn, alt_mfr, alt_mpn })
}

/// Resolves all BOM entries in `nextbom_conn` against the parts database in `nextdb_conn`.
///
/// Migrates the `bom` table to add `mfr`, `mpn`, `alt_mfr`, and `alt_mpn` columns (if not
/// present), then populates them with JSON arrays. Main-table results go into `mfr`/`mpn`;
/// alt-table results go into `alt_mfr`/`alt_mpn`. The `part_id` column is left untouched —
/// alt matches are signalled by the alt columns, and the project-specifics suffix is only
/// appended at Excel export time.
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
                "UPDATE bom SET mfr = ?1, mpn = ?2, alt_mfr = ?3, alt_mpn = ?4 WHERE designator = ?5",
                rusqlite::params![mfr_json, mpn_json, alt_mfr_json, alt_mpn_json, designator],
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

/// A grouped BOM row ready for Excel export: one row per `part_id`.
///
/// `designators` is sorted alphabetically. `mfr` and `mpn` each contain primary values
/// followed by alternatives, joined by `"; "`.
pub struct ExcelBomRow {
    pub part_id: String,
    pub designators: Vec<String>,
    pub qty: usize,
    pub mfr: String,
    pub mpn: String,
}

/// Groups resolved BOM entries into one row per `part_id`, sorted alphabetically.
///
/// Designators sharing the same `part_id` are collected and sorted. `mfr` and `mpn` come from
/// the first entry for each `part_id` (they are identical for all designators of the same part);
/// alt values are appended behind the primary values in the same cell, separated by a newline.
///
/// When `project_specifics` is `Some(s)` and a row has alternative entries, the output
/// `part_id` is suffixed with `_{s}` (e.g. `RES00100_2025`) — this is the export-time signal
/// to procurement that an alternative applies. The `part_id` stored in the `bom` table is
/// never modified.
pub fn group_for_excel(entries: &[ResolvedBomEntry], project_specifics: Option<&str>) -> Vec<ExcelBomRow> {
    use std::collections::BTreeMap;

    let mut map: BTreeMap<
        String,
        (Vec<String>, Vec<String>, Vec<String>, Vec<String>, Vec<String>),
    > = BTreeMap::new();

    for e in entries {
        let g = map.entry(e.part_id.clone()).or_insert_with(|| {
            (Vec::new(), e.mfr.clone(), e.mpn.clone(), e.alt_mfr.clone(), e.alt_mpn.clone())
        });
        g.0.push(e.designator.clone());
    }

    map.into_iter()
        .map(|(part_id, (mut designators, mfr, mpn, alt_mfr, alt_mpn))| {
            designators.sort();
            let all_mfr: Vec<&str> =
                mfr.iter().chain(alt_mfr.iter()).map(String::as_str).collect();
            let all_mpn: Vec<&str> =
                mpn.iter().chain(alt_mpn.iter()).map(String::as_str).collect();
            let display_part_id = match project_specifics {
                Some(s) if !alt_mfr.is_empty() => format!("{}_{}", part_id, s),
                _ => part_id,
            };
            ExcelBomRow {
                qty: designators.len(),
                part_id: display_part_id,
                designators,
                mfr: all_mfr.join("\n"),
                mpn: all_mpn.join("\n"),
            }
        })
        .collect()
}

// ── Excel export ─────────────────────────────────────────────────────────────

/// Column keys that define repeating data rows in a BOM template.
const COLUMN_KEYS: &[&str] = &["?no", "?part_id", "?designators", "?qty", "?mfr", "?mpn"];

/// Scalar keys that are replaced once anywhere in a BOM template.
const SCALAR_KEYS: &[&str] = &["?pcb_name", "?bom_version", "?design_variant", "?engineer", "?creation_date"];

/// Finds all `?word` tokens in `s` (where word is `[a-zA-Z_]+`).
fn find_template_keys(s: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'?' {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
                i += 1;
            }
            if i > start + 1 {
                keys.push(s[start..i].to_string());
            }
        } else {
            i += 1;
        }
    }
    keys
}

/// Replaces all known template keys in `s` with their values.
///
/// Column keys are replaced with the corresponding field from `bom_row` when provided,
/// or cleared to `""` when `bom_row` is `None`. Scalar keys are always substituted.
fn apply_template_cell(
    s: &str,
    bom_row: Option<(&ExcelBomRow, usize)>,
    pcb_name: &str,
    bom_version: &str,
    design_variant: &str,
    engineer: &str,
    creation_date: &str,
) -> String {
    let s = s
        .replace("?pcb_name", pcb_name)
        .replace("?bom_version", bom_version)
        .replace("?design_variant", design_variant)
        .replace("?engineer", engineer)
        .replace("?creation_date", creation_date);

    match bom_row {
        Some((row, no)) => s
            .replace("?no", &no.to_string())
            .replace("?part_id", &row.part_id)
            .replace("?designators", &row.designators.join(", "))
            .replace("?qty", &row.qty.to_string())
            .replace("?mfr", &row.mfr)
            .replace("?mpn", &row.mpn),
        None => {
            let mut s = s;
            for key in COLUMN_KEYS {
                s = s.replace(key, "");
            }
            s
        }
    }
}

/// Generates a default (no-template) BOM `.xlsx` file at `path`.
///
/// Writes a header row followed by one data row per entry in `rows`.
pub fn write_default_xlsx(path: &std::path::Path, rows: &[ExcelBomRow]) -> Result<(), String> {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.get_sheet_mut(&0usize).ok_or("Failed to get worksheet")?;

    for (col, header) in ["part_id", "designators", "qty", "mfr", "mpn"].iter().enumerate() {
        sheet.get_cell_mut((col as u32 + 1, 1)).set_value(*header);
    }

    for (i, row) in rows.iter().enumerate() {
        let r = i as u32 + 2;
        sheet.get_cell_mut((1u32, r)).set_value(&row.part_id);
        sheet.get_cell_mut((2u32, r)).set_value(row.designators.join(", "));
        sheet.get_cell_mut((3u32, r)).set_value_number(row.qty as f64);
        sheet.get_cell_mut((4u32, r)).set_value(&row.mfr);
        sheet.get_cell_mut((5u32, r)).set_value(&row.mpn);
        // qty is numeric → right-aligned by default; mpn must always be left-aligned
        for col in [3u32, 5u32] {
            sheet.get_style_mut((col, r))
                .get_alignment_mut()
                .set_horizontal(umya_spreadsheet::structs::HorizontalAlignmentValues::Left);
        }
    }

    umya_spreadsheet::writer::xlsx::write(&book, path).map_err(|e| e.to_string())
}

/// Applies BOM data to an Excel template and returns the modified workbook.
///
/// Scans all cells for `?key` tokens. Scalar keys (`?pcb_name`, `?bom_version`,
/// `?design_variant`) are substituted in-place. The row containing column keys
/// (`?part_id`, `?designators`, `?qty`, `?mfr`, `?mpn`) is replaced by one row per
/// BOM entry, with additional rows inserted to make room. Returns an error if any
/// unrecognised `?key` token is found or if the template contains no column keys.
pub fn apply_bom_template(
    template_path: &std::path::Path,
    rows: &[ExcelBomRow],
    pcb_name: &str,
    bom_version: &str,
    design_variant: &str,
    engineer: &str,
    creation_date: &str,
) -> Result<umya_spreadsheet::Spreadsheet, String> {
    let mut book = umya_spreadsheet::reader::xlsx::read(template_path)
        .map_err(|e| format!("Failed to read template: {}", e))?;

    // Collect original cell data and header/footer strings before any modifications.
    let (cell_data, header_str, footer_str): (Vec<(u32, u32, String)>, String, String) = {
        let sheet = book.get_sheet(&0usize).ok_or("Template has no worksheets")?;
        let (max_col, max_row) = sheet.get_highest_column_and_row();
        let mut cells = Vec::new();
        for row in 1..=max_row {
            for col in 1..=max_col {
                if let Some(cell) = sheet.get_cell((col, row)) {
                    let v = cell.get_value().to_string();
                    if !v.is_empty() {
                        cells.push((row, col, v));
                    }
                }
            }
        }
        let hf = sheet.get_header_footer();
        let header = hf.get_odd_header().get_value().to_string();
        let footer = hf.get_odd_footer().get_value().to_string();
        (cells, header, footer)
    };

    // Validate all keys and locate the column-key row.
    let mut template_row: Option<u32> = None;
    for (row, _col, val) in &cell_data {
        for key in find_template_keys(val) {
            if COLUMN_KEYS.contains(&key.as_str()) {
                match template_row {
                    Some(r) if r != *row => return Err(format!(
                        "Column keys must all be in the same row (found in rows {} and {})", r, row
                    )),
                    _ => template_row = Some(*row),
                }
            } else if !SCALAR_KEYS.contains(&key.as_str()) {
                return Err(format!("Unknown template key: {}", key));
            }
        }
    }
    for s in [&header_str, &footer_str] {
        for key in find_template_keys(s) {
            if !COLUMN_KEYS.contains(&key.as_str()) && !SCALAR_KEYS.contains(&key.as_str()) {
                return Err(format!("Unknown template key: {}", key));
            }
        }
    }
    let tmpl_row = template_row.ok_or(
        "Template must contain at least one column key: ?no, ?part_id, ?designators, ?qty, ?mfr, ?mpn"
    )?;

    // Apply scalar substitutions to all non-template-row cells.
    {
        let sheet = book.get_sheet_mut(&0usize).unwrap();
        for (row, col, val) in &cell_data {
            if *row != tmpl_row {
                let new_val = apply_template_cell(val, None, pcb_name, bom_version, design_variant, engineer, creation_date);
                if new_val != *val {
                    sheet.get_cell_mut((*col, *row)).set_value(new_val);
                }
            }
        }
        // Apply scalar substitutions to header and footer.
        let new_header = apply_template_cell(&header_str, None, pcb_name, bom_version, design_variant, engineer, creation_date);
        let new_footer = apply_template_cell(&footer_str, None, pcb_name, bom_version, design_variant, engineer, creation_date);
        sheet.get_header_footer_mut().get_odd_header_mut().set_value(new_header);
        sheet.get_header_footer_mut().get_odd_footer_mut().set_value(new_footer);
    }

    // Insert extra rows to make room for BOM data (inserts N-1 rows after the template row).
    if rows.len() > 1 {
        let sheet = book.get_sheet_mut(&0usize).unwrap();
        sheet.insert_new_row(&(tmpl_row + 1), &((rows.len() - 1) as u32));
    }

    // Write BOM rows into the sheet, starting at tmpl_row.
    // Each BOM row fills columns that originally contained a column key; other
    // columns in the template row are preserved (with scalar substitutions applied).
    let write_targets: Vec<(u32, Option<(&ExcelBomRow, usize)>)> = if rows.is_empty() {
        vec![(tmpl_row, None)]
    } else {
        rows.iter().enumerate().map(|(i, r)| (tmpl_row + i as u32, Some((r, i + 1)))).collect()
    };

    for (target_row, bom_row) in write_targets {
        let sheet = book.get_sheet_mut(&0usize).unwrap();
        // Use original template-row cell values as the source for substitution.
        for (orig_row, col, val) in &cell_data {
            if *orig_row == tmpl_row {
                let new_val =
                    apply_template_cell(val, bom_row, pcb_name, bom_version, design_variant, engineer, creation_date);
                // Left-align: numeric values (auto right-aligned by Excel) and MPN column.
                let needs_left = new_val.parse::<f64>().is_ok() || val.contains("?mpn");
                sheet.get_cell_mut((*col, target_row)).set_value(new_val);
                if needs_left {
                    sheet.get_style_mut((*col, target_row))
                        .get_alignment_mut()
                        .set_horizontal(umya_spreadsheet::structs::HorizontalAlignmentValues::Left);
                }
            }
        }
    }

    Ok(book)
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
        assert!(r.mfr.is_empty());
        assert!(r.mpn.is_empty());
        assert!(r.alt_mfr.is_empty());
        assert!(r.alt_mpn.is_empty());
    }

    #[test]
    fn resolve_part_id_with_alt_separates_into_alt_columns() {
        let db = Connection::open_in_memory().unwrap();
        make_nextdb(&db);
        db.execute_batch(
            "CREATE TABLE alt_2025 (ID TEXT, mfr TEXT, mpn TEXT);
             INSERT INTO alt_2025 VALUES ('RES00100', 'Vishay', 'CRCW0402');",
        )
        .unwrap();

        let r = resolve_part_id(&db, "RES00100", Some("2025")).unwrap();
        assert_eq!(r.mfr, vec!["Yageo"]);
        assert_eq!(r.mpn, vec!["RC0402"]);
        assert_eq!(r.alt_mfr, vec!["Vishay"]);
        assert_eq!(r.alt_mpn, vec!["CRCW0402"]);
    }

    #[test]
    fn resolve_part_id_alt_table_missing_yields_empty_alt_columns() {
        let db = Connection::open_in_memory().unwrap();
        make_nextdb(&db);

        let r = resolve_part_id(&db, "RES00100", Some("nonexistent")).unwrap();
        assert!(r.alt_mfr.is_empty());
        assert!(r.alt_mpn.is_empty());
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
    fn group_for_excel_single_part_no_alts() {
        let entries = vec![ResolvedBomEntry {
            designator: "C1".into(),
            part_id: "CAP00100".into(),
            mfr: vec!["TDK".into()],
            mpn: vec!["C0402".into()],
            alt_mfr: vec![],
            alt_mpn: vec![],
        }];
        let rows = group_for_excel(&entries, None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].part_id, "CAP00100");
        assert_eq!(rows[0].designators, vec!["C1"]);
        assert_eq!(rows[0].qty, 1);
        assert_eq!(rows[0].mfr, "TDK");
        assert_eq!(rows[0].mpn, "C0402");
    }

    #[test]
    fn group_for_excel_multiple_designators_sorted() {
        let entries = vec![
            ResolvedBomEntry {
                designator: "C3".into(),
                part_id: "CAP00100".into(),
                mfr: vec!["TDK".into()],
                mpn: vec!["C0402".into()],
                alt_mfr: vec![],
                alt_mpn: vec![],
            },
            ResolvedBomEntry {
                designator: "C1".into(),
                part_id: "CAP00100".into(),
                mfr: vec!["TDK".into()],
                mpn: vec!["C0402".into()],
                alt_mfr: vec![],
                alt_mpn: vec![],
            },
        ];
        let rows = group_for_excel(&entries, None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].designators, vec!["C1", "C3"]);
        assert_eq!(rows[0].qty, 2);
    }

    #[test]
    fn group_for_excel_alts_appended_to_primary() {
        let entries = vec![ResolvedBomEntry {
            designator: "R1".into(),
            part_id: "RES00100".into(),
            mfr: vec!["Yageo".into()],
            mpn: vec!["RC0402".into()],
            alt_mfr: vec!["Vishay".into()],
            alt_mpn: vec!["CRCW0402".into()],
        }];
        let rows = group_for_excel(&entries, None);
        assert_eq!(rows[0].mfr, "Yageo\nVishay");
        assert_eq!(rows[0].mpn, "RC0402\nCRCW0402");
    }

    #[test]
    fn group_for_excel_multiple_parts_alphabetical_order() {
        let entries = vec![
            ResolvedBomEntry {
                designator: "R1".into(),
                part_id: "RES00100".into(),
                mfr: vec![],
                mpn: vec![],
                alt_mfr: vec![],
                alt_mpn: vec![],
            },
            ResolvedBomEntry {
                designator: "C1".into(),
                part_id: "CAP00100".into(),
                mfr: vec![],
                mpn: vec![],
                alt_mfr: vec![],
                alt_mpn: vec![],
            },
        ];
        let rows = group_for_excel(&entries, None);
        assert_eq!(rows[0].part_id, "CAP00100");
        assert_eq!(rows[1].part_id, "RES00100");
    }

    #[test]
    fn group_for_excel_empty_mfr_produces_empty_string() {
        let entries = vec![ResolvedBomEntry {
            designator: "U1".into(),
            part_id: "IC00001".into(),
            mfr: vec![],
            mpn: vec![],
            alt_mfr: vec![],
            alt_mpn: vec![],
        }];
        let rows = group_for_excel(&entries, None);
        assert_eq!(rows[0].mfr, "");
        assert_eq!(rows[0].mpn, "");
    }

    #[test]
    fn group_for_excel_empty_input_returns_empty() {
        let rows = group_for_excel(&[], None);
        assert!(rows.is_empty());
    }

    #[test]
    fn group_for_excel_suffixes_part_id_only_when_alt_present() {
        let entries = vec![
            ResolvedBomEntry {
                designator: "R1".into(),
                part_id: "RES00100".into(),
                mfr: vec!["Yageo".into()],
                mpn: vec!["RC0402".into()],
                alt_mfr: vec!["Vishay".into()],
                alt_mpn: vec!["CRCW0402".into()],
            },
            ResolvedBomEntry {
                designator: "C1".into(),
                part_id: "CAP00100".into(),
                mfr: vec!["TDK".into()],
                mpn: vec!["C0402".into()],
                alt_mfr: vec![],
                alt_mpn: vec![],
            },
        ];
        let rows = group_for_excel(&entries, Some("2025"));
        // CAP00100 (no alt) is unchanged; RES00100 (has alt) gets the suffix.
        assert_eq!(rows[0].part_id, "CAP00100");
        assert_eq!(rows[1].part_id, "RES00100_2025");
    }

    #[test]
    fn group_for_excel_no_specifics_never_suffixes() {
        let entries = vec![ResolvedBomEntry {
            designator: "R1".into(),
            part_id: "RES00100".into(),
            mfr: vec!["Yageo".into()],
            mpn: vec!["RC0402".into()],
            alt_mfr: vec!["Vishay".into()],
            alt_mpn: vec!["CRCW0402".into()],
        }];
        let rows = group_for_excel(&entries, None);
        assert_eq!(rows[0].part_id, "RES00100");
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

        // R1 has an alt entry — alt columns are populated, part_id is left untouched.
        let part_id: String = nextbom
            .query_row("SELECT part_id FROM bom WHERE designator = 'R1'", [], |r| r.get(0))
            .unwrap();
        let alt_mfr: String = nextbom
            .query_row("SELECT alt_mfr FROM bom WHERE designator = 'R1'", [], |r| r.get(0))
            .unwrap();
        let alt_mpn: String = nextbom
            .query_row("SELECT alt_mpn FROM bom WHERE designator = 'R1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(part_id, "RES00100");
        assert_eq!(alt_mfr, r#"["Vishay"]"#);
        assert_eq!(alt_mpn, r#"["CRCW0402"]"#);

        // C1 has no alt entry — alt columns are empty arrays, part_id unchanged.
        let part_id: String = nextbom
            .query_row("SELECT part_id FROM bom WHERE designator = 'C1'", [], |r| r.get(0))
            .unwrap();
        let alt_mfr: String = nextbom
            .query_row("SELECT alt_mfr FROM bom WHERE designator = 'C1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(part_id, "CAP00100");
        assert_eq!(alt_mfr, r#"[]"#);
    }

    #[test]
    fn resolve_bom_entries_is_idempotent_under_repeated_calls() {
        // Re-running resolution must not mutate part_id, otherwise the second pass would look
        // up a stale value and lose the resolution.
        let nextbom = Connection::open_in_memory().unwrap();
        let nextdb = Connection::open_in_memory().unwrap();
        make_nextbom(&nextbom);
        make_nextdb(&nextdb);
        nextdb.execute_batch(
            "CREATE TABLE alt_2025 (ID TEXT, mfr TEXT, mpn TEXT);
             INSERT INTO alt_2025 VALUES ('RES00100', 'Vishay', 'CRCW0402');",
        ).unwrap();

        resolve_bom_entries(&nextbom, &nextdb, Some("2025")).unwrap();
        resolve_bom_entries(&nextbom, &nextdb, Some("2025")).unwrap();

        let part_id: String = nextbom
            .query_row("SELECT part_id FROM bom WHERE designator = 'R1'", [], |r| r.get(0))
            .unwrap();
        let mfr: String = nextbom
            .query_row("SELECT mfr FROM bom WHERE designator = 'R1'", [], |r| r.get(0))
            .unwrap();
        let alt_mfr: String = nextbom
            .query_row("SELECT alt_mfr FROM bom WHERE designator = 'R1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(part_id, "RES00100");
        assert_eq!(mfr, r#"["Yageo"]"#);
        assert_eq!(alt_mfr, r#"["Vishay"]"#);
    }
}
