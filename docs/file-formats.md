# File Formats

nextbom uses three file types.

## `.nbp` — Project Files {#nbp-project-files}

Plain JSON, saved with readable formatting. Stores all project metadata but not BOM data.

```json
{
  "uuid": "550e8400-e29b-41d4-a716-446655440000",
  "title": "My Project",
  "engineer": "Jane Smith",
  "database_path": "/path/to/db.nextdb",
  "project_specifics": "parts_2025",
  "design_variant": "full",
  "bom_template_path": "/path/to/template.xlsx",
  "last_change": 1713707400000,
  "schema": "0.1.0"
}
```

| Field | Description |
|---|---|
| `uuid` | Unique identifier for the project (UUIDv4). |
| `title` | Project title. |
| `engineer` | Engineer name (may be empty). |
| `database_path` | Absolute path to the linked `.nextdb` parts database file. Empty if no database has been linked yet. |
| `project_specifics` | Identifier for a project-specific alternative parts set (may be empty). |
| `design_variant` | Assembly variant name (may be empty). |
| `bom_template_path` | Absolute path to an `.xlsx` BOM export template (may be empty). When set, [Excel export](workflow.md#generate-output) fills the template instead of writing a plain BOM sheet. |
| `last_change` | Unix timestamp in milliseconds of the last modification. |
| `schema` | File format version (semver). Major-version compatibility: a `1.x` file can only be opened by `1.x` software. |

## `.nextdb` — Parts Database Files {#nextdb-database-files}

A SQLite database containing the mapping from generic part IDs to manufacturer part numbers. This is a shared resource across projects — you create and manage it separately with an external tool; nextbom only reads it.

**Schema:**

All tables (`parts` and every `alt_*`) share the same column schema:

| Column | Type | Description |
|---|---|---|
| `ID`   | TEXT | Generic part identifier (e.g. `CAP00100`). A single ID may appear in multiple rows — one per manufacturer/MPN. |
| `mfr`  | TEXT | Manufacturer name (e.g. `TDK`). |
| `mpn`  | TEXT | Manufacturer part number (e.g. `C0402X5R1C104K`). |

| Table | Description |
|---|---|
| `parts` | Primary part list. |
| `alt_<name>` | Project-specific alternative part set. One table per set, named with the `alt_` prefix (e.g. `alt_2025`). |

When resolving a BOM, nextbom queries the `parts` table for each generic part ID. If the project has a `project_specifics` value set, the matching `alt_<project_specifics>` table is also queried and its entries are appended. All matching rows across both tables are merged into manufacturer and MPN arrays.

Multiple projects can share the same `.nextdb` file, or each project can use a different one.

## `.nextbom` — BOM Working Files {#nextbom-working-files}

A SQLite working file created when you import a CSV. It contains the schematic designators mapped to their generic part IDs for a specific project, plus a metadata row describing the BOM. This is a working file used to generate the final BOM output — it is not directly linked to the project file (`.nbp`).

**`bom` table:**

| Column | Type | Description |
|---|---|---|
| `designator` | TEXT (PK) | Schematic reference designator (e.g. `C1`, `R12`). |
| `part_id` | TEXT | Generic part identifier (e.g. `CAP00100`). |
| `mfr`     | TEXT | JSON array of manufacturer names from the main `parts` table (e.g. `["TDK","Murata"]`). Populated during step 2. |
| `mpn`     | TEXT | JSON array of MPNs from the main `parts` table (e.g. `["C0402X5R","GRM0332"]`). Populated during step 2. |
| `alt_mfr` | TEXT | JSON array of manufacturer names from the project-specific `alt_*` table. Empty array if no alt entries exist. Populated during step 2. |
| `alt_mpn` | TEXT | JSON array of MPNs from the project-specific `alt_*` table. Empty array if no alt entries exist. Populated during step 2. |

The four resolution columns are added by step 2. A freshly created `.nextbom` file has only `designator` and `part_id`. The `part_id` column is never modified — when a part has an alt-table match, the project-specifics suffix (e.g. `RES00100_2025`) is applied only at Excel export time, so the working file remains a faithful record of what the schematic specified.

**`metadata` table:**

A single-row table (enforced by `CHECK (id = 1)`) describing the BOM as a whole. Step 1 writes the first six fields below; step 2 adds the resolution fields.

| Column | Type | Description |
|---|---|---|
| `pcb_name`          | TEXT | PCB assembly name. |
| `design_variant`    | TEXT | Assembly variant (e.g. `full`, `lite`). |
| `bom_version`       | TEXT | BOM revision number. |
| `source_csv_path`   | TEXT | Absolute path of the CSV imported in step 1. |
| `csv_imported_at`   | INTEGER | Unix timestamp (ms) when the CSV was imported — also used as the BOM creation date. |
| `engineer`          | TEXT | Engineer name copied from the project at step 1. |
| `project_specifics` | TEXT | Alt-set identifier used during resolution (may be NULL). Populated by step 2. |
| `database_path`     | TEXT | Absolute path of the `.nextdb` queried during step 2. |
| `database_version`  | TEXT | `database_version` value read from the `.nextdb` metadata table (may be NULL). |
| `resolved_at`       | INTEGER | Unix timestamp (ms) when step 2 completed. |

Step 3 (Excel export) reads only this `metadata` row and the resolved `bom` rows. The exported file is therefore self-contained: the values printed into it (PCBA name, version, engineer, creation date, …) come from the `.nextbom` file, not from live project state or the system clock.

## CSV Import Format {#csv-import-format}

The CSV format expected when importing BOM data from your EDA tool.

- **Delimiter**: semicolon (`;`)
- **Header row**: required (first row is always skipped)
- **Column order**: part ID first, designator second

```
part_id;designator
CAP00100;C1
CAP00100;C2
RES00001;R1
IC00050;U1
```

Whitespace around values is trimmed. Files with fewer than two columns are rejected.
