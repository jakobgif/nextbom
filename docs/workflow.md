# BOM Workflow

Once a project is open, the main screen shows a three-step workflow for creating a BOM from your schematic data.

## Step 1: Create a NextBOM file {#create-nextbom-file}

### Import a CSV

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

### Configure the BOM

Before creating the file, set these fields:

**PCBA Name**
: The name of the printed circuit board assembly. Defaults to the project title (Auto mode) but can be edited freely by unchecking **Auto**. Stored as metadata in the generated database.

**BOM Version**
: A numeric version identifier for this BOM (digits only). Stored as metadata alongside the PCBA name and design variant.

**Design Variant**
: Identifies which assembly variant this BOM belongs to — for example `full` or `lite`. Pre-populated from the last value used in this project. After the file is created, the value is saved back to the project so it loads automatically next time.

### Save the file

Click **Create NextBOM file**. A file picker opens so you can choose where to save the `.nextbom` working file.

The generated file is a SQLite working file containing:

- A `bom` table with one row per component: `designator` (primary key) → `part_id`

See [File Formats](file-formats.md#nextbom-working-files) for details.

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

A save-file dialog opens so you can choose where to save the output. The default filename is derived from the PCBA name and BOM version stored in the working file (e.g. `myboard_v2.xlsx`).

After a successful export, the output path is shown and an **Open file** button opens it directly in your system's default spreadsheet application.

The exported file contains one row per part ID with the following columns:

| Column | Contents |
|--------|----------|
| `part_id` | The generic part identifier |
| `designators` | All schematic designators using this part, separated by `; ` |
| `qty` | Number of placements |
| `mfr` | Primary manufacturer(s) followed by any alternatives, separated by `; ` |
| `mpn` | Primary MPN(s) followed by any alternative MPNs, separated by `; ` |
