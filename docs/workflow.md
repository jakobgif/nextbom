# BOM Workflow

Once a project is open, the main screen shows a step-by-step workflow for creating a nextbom database from your schematic data.

## Step 1: Import a CSV {#import-csv}

Click **Import CSV** to load a BOM export from your EDA tool.

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

After a successful import, nextbom shows how many entries were loaded and uses the CSV filename as a default PCBA name.

!!! tip
    Most EDA tools (KiCad, Altium, etc.) can export a BOM as CSV. You may need to configure the delimiter and column order to match the format above.

!!! note "Prerequisite"
    Before proceeding, make sure your project is [linked to a `.nextdb` parts database](project.md#linking-the-parts-database).

## Step 2: Configure the BOM {#configure-bom}

Before creating the database, set two fields:

**PCBA Name**
: The name of the printed circuit board assembly. Defaults to the CSV filename stem but can be edited freely. Stored as metadata in the generated database.

**BOM Version**
: A numeric version identifier for this BOM (digits only). Stored as metadata alongside the PCBA name and design variant.

## Step 3: Create the BOM Working File {#create-bom-file}

Click **Create nextbom BOM file**. A file picker opens so you can choose where to save the `.nextbom` working file.

The generated file is a SQLite working file containing:

- A `bom` table with one row per component: `designator` (primary key) → `part_id`

This file is used internally to generate the final BOM output. It is not directly linked to your project — only the `.nextdb` parts database is linked to the project.

See [File Formats](file-formats.md#nextbom-working-files) for details.

<!-- ## Step 4: Generate BOM Output

Once a `.nextbom` working file is created, you can generate the final BOM output (Excel format). 

The app reads the `.nextbom` file to get each designator and its generic part ID, then looks up the manufacturer part number in the linked `.nextdb` parts database.

The output is an Excel table ready for procurement or manufacturing.

See [File Formats](file-formats.md) to understand how these files work together. -->
