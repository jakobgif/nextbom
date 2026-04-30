# BOM Workflow

Once a project is open, the main screen shows a three-step workflow for creating a BOM from your schematic data.

## Step 1: Create a NextBOM file {#create-nextbom-file}

### Import a CSV {#import-csv}

Click **Import CSV file** to load a BOM export from your EDA tool.

nextbom expects a **semicolon-delimited** CSV with a header row. The column order is:

```
part_id;designator
CAP00100;C1
CAP00100;C2
RES00001;R1
```

- **Column 1 — Part ID**: The generic part identifier (e.g. `CAP00100`). This is the ID that will later be resolved to a manufacturer part number.
- **Column 2 — Designator**: The schematic reference designator (e.g. `C1`, `R2`, `U12`).
- The header row is skipped automatically.
- Leading and trailing whitespace is trimmed from both columns.

After a successful import, nextbom shows the file path and a checkmark.

!!! tip
    Most EDA tools (KiCad, Altium, etc.) can export a BOM as CSV. You may need to configure the delimiter and column order to match the format above.

### Configure the BOM {#configure-bom}

Before creating the file, set these fields:

**PCBA Name**
: The name of the printed circuit board assembly. Defaults to the project title (Auto mode) but can be edited freely by unchecking **Auto**. Stored as metadata in the generated database.

**BOM Version**
: A numeric version identifier for this BOM (digits only). Stored as metadata alongside the PCBA name and design variant.

**Design Variant**
: Identifies which assembly variant this BOM belongs to — for example `full` or `lite`. Pre-populated from the last value used in this project. After the file is created, the value is saved back to the project so it loads automatically next time.

### Save the file

Click **Create NextBOM file**. A file picker opens so you can choose where to save the `.nextbom` working file. The default filename is `{PCBA name}_v{BOM version}.nextbom`.

The generated file is a SQLite working file containing:

- A `bom` table with one row per component: `designator` (primary key) → `part_id`
- A single-row `metadata` table with the PCBA name, BOM version, design variant, source CSV path, the engineer (copied from the project), and the import timestamp (used as the BOM creation date during Excel export)

See [File Formats](file-formats.md#nextbom-working-files) for the full schema.

## Step 2: Resolve Manufacturers & MPNs {#resolve-manufacturers}

This step looks up each part ID in the linked `.nextdb` parts database and writes the resolved manufacturer name and MPN back into the `.nextbom` file.

!!! note "Prerequisite"
    Before proceeding, make sure your project is [linked to a `.nextdb` parts database](project.md#linking-the-parts-database).

By default, **Load NextBOM from step 1** is checked and the file from Step 1 is used automatically. Uncheck it to select a different `.nextbom` file manually.

Click **Resolve**. nextbom:

1. Opens the linked parts database
2. Queries each unique part ID against the `parts` table
3. If a project-specific alternative set is configured, also queries the matching `alt_*` table
4. Writes the results (`mfr`, `mpn`, `alt_mfr`, `alt_mpn`) back into the `.nextbom` file

After resolution, a preview table shows all resolved rows grouped by part ID, including any alternative manufacturer and MPN columns when alternatives are present.

## Step 3: Generate BOM Output {#generate-output}

Click **Export to Excel** to export the resolved BOM to an `.xlsx` file.

By default, **Load resolved NextBOM from step 2** is checked and the file from Step 2 is used automatically. Uncheck it to select a different `.nextbom` file manually.

If the open project has a [BOM template](project.md#bom-templates) set, the template is filled with the BOM data. Otherwise nextbom writes a plain default sheet. The current template path (or `—` if none) is shown above the **Export to Excel** button.

A save-file dialog opens so you can choose where to save the output. The default filename is derived from the PCBA name and BOM version stored in the working file (e.g. `myboard_v2.xlsx`).

After a successful export, the output path is shown and an **Open file** button opens it directly in your system's default spreadsheet application.

All values written into the output are read from the `.nextbom` file's `metadata` row (PCBA name, BOM version, engineer, creation date, …) and its resolved `bom` rows — never from the live project state or the system clock. This means the same `.nextbom` file always produces the same export, no matter when or from which project it is opened.

### Default sheet (no template)

When no template is set, the exported file contains one row per part ID with the following columns:

| Column | Contents |
|--------|----------|
| `part_id` | The generic part identifier. Suffixed with `_{project_specifics}` (e.g. `RES00100_2025`) on rows that had an alt-table match. |
| `designators` | All schematic designators using this part, sorted alphabetically and joined by `, `. |
| `qty` | Number of placements. |
| `mfr` | Primary manufacturer(s) followed by any alternatives, one per line within the cell. |
| `mpn` | Primary MPN(s) followed by any alternative MPNs, one per line within the cell. |

### BOM template placeholders {#bom-template-placeholders}

When a [BOM template](project.md#bom-templates) is set, nextbom scans every cell — plus the page header and footer — for `?key` tokens and substitutes them. Unknown `?key` tokens cause the export to fail, so typos are surfaced rather than silently passed through.

**Scalar keys** are replaced once wherever they appear. Each value comes from the `.nextbom` file's `metadata` row.

| Key | Replaced with |
|---|---|
| `?pcb_name` | PCBA name |
| `?bom_version` | BOM version |
| `?design_variant` | Design variant |
| `?engineer` | Engineer name (from the project at step 1) |
| `?creation_date` | The CSV import timestamp, formatted `YYYY-MM-DD HH:MM:SS` in local time |

**Column keys** mark a single repeating row. The row containing column keys is duplicated once per BOM line, with each column key replaced by the matching field. All column keys must sit on the same row.

| Key | Replaced with |
|---|---|
| `?no` | 1-based row number |
| `?part_id` | Generic part identifier (with the `_{project_specifics}` suffix on alt rows) |
| `?designators` | Designators joined by `, ` |
| `?qty` | Number of placements |
| `?mfr` | Manufacturers, one per line within the cell |
| `?mpn` | MPNs, one per line within the cell |

Cells in the column-key row that contain no column key are preserved on every output row (with scalar substitutions still applied), so static labels and formulas in the repeating row are kept as-is.

Header/footer images and other VML drawings present in the template are preserved in the output.
