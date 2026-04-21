# File Formats

nextbom uses three file types.

## `.nbp` — Project Files {#nbp-project-files}

Plain JSON, saved with readable formatting. Stores all project metadata but not BOM data.

```json
{
  "uuid": "550e8400-e29b-41d4-a716-446655440000",
  "title": "My Project",
  "engineer": "Jane Smith",
  "database_path": "/path/to/db.nextbom",
  "project_specifics": "parts_2025",
  "design_variant": "full",
  "latest_bom_version": "1",
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
| `latest_bom_version` | Numeric version of the most recently created BOM. |
| `last_change` | Unix timestamp in milliseconds of the last modification. |
| `schema` | File format version (semver). Major-version compatibility: a `1.x` file can only be opened by `1.x` software. |

## `.nextdb` — Parts Database Files {#nextdb-database-files}

A SQLite database containing the mapping from generic part IDs to manufacturer part numbers. This is a shared resource across projects — you create and manage it separately with an external tool; nextbom only reads it.

**Schema:**

| Table | Description |
|---|---|
| `parts` | Primary part list. One row per generic part ID. |
| `alt_<name>` | Project-specific alternative part set. One table per set, named with the `alt_` prefix (e.g. `alt_2025`). |

When generating a BOM, nextbom starts from the `parts` table and appends all entries from the selected `alt_` table. The two lists are combined as-is — entries from both tables appear in the final BOM.

Multiple projects can share the same `.nextdb` file, or each project can use a different one.

## `.nextbom` — BOM Working Files {#nextbom-working-files}

A SQLite working file created when you import a CSV. It contains the schematic designators mapped to their generic part IDs for a specific project. This is a working file used to generate the final BOM output — it is not directly linked to the project file (`.nbp`).

**Schema:**

| Column | Type | Description |
|---|---|---|
| `designator` | TEXT (PK) | Schematic reference designator (e.g. `C1`, `R12`). |
| `part_id` | TEXT | Generic part identifier (e.g. `CAP00100`). |

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
