# Projects

A **project** is the central object in nextbom. It manages your design metadata and links to the `.nextdb` parts database used to resolve part numbers.

## Creating a Project

Click **Create Project** on the home screen, or use **File → New Project**.

Fill in the fields:

| Field | Required | Description |
|---|---|---|
| **Title** | Yes | Human-readable name for the project. Used as the default filename when saving. |
| **Engineer** | No | Engineer or team responsible. Shown in the status bar and recorded in every `.nextbom` file you create from this project. |
| **Parts Database** | No | Browse for the `.nextdb` parts database to link to the project. Can be linked or changed later via **Project → Select Parts Database**. |
| **Project Specifics** | No | Pick one of the alt-part sets defined in the linked `.nextdb`, or **None**. Disabled until a parts database is selected. |
| **BOM Template** | No | Browse for an `.xlsx` template that will be filled when you export the BOM. See [BOM Templates](#bom-templates). |

## Opening a Project

Click **Open Project** on the home screen, or use **File → Open Project**. Select a `.nbp` file.

The six most recently opened projects also appear on the home screen and under **File → Open Recent**.

## Saving a Project

Use **File → Save Project** (`Ctrl+S`) to save to the current file. Use **File → Save Project As** to choose a new location.

Projects that have unsaved changes show **(unsaved)** in the title bar.

nextbom will prompt you before closing or switching projects if there are unsaved changes.

## Editing Project Metadata

All metadata fields can be changed after creation via the **Project** menu:

- **Project → Set Title**
- **Project → Set Engineer**
- **Project → Select Parts Database** — link or re-link the project to a `.nextdb` parts database file
- **Project → Set Project Specifics** — pick an alt-set from the linked `.nextdb`, or **None** to clear
- **Project → Set BOM Template** — pick an `.xlsx` file to use as the export template
- **Project → Clear BOM Template** — only shown when a template is set; reverts to the default sheet layout

The design variant is not set here — it is entered when [creating a BOM working file](workflow.md#configure-bom) and automatically stored in the project for reuse next time.

The `.nextdb` file is the shared parts database that maps generic part IDs to manufacturer part numbers. See [File Formats](file-formats.md) to understand the relationship between `.nextbom` project BOMs and `.nextdb` parts databases.

## Linking the Parts Database {#linking-the-parts-database}

Before you can create a BOM file, your project must be linked to a `.nextdb` parts database. This database contains the mappings from generic part IDs to actual manufacturer part numbers.

Use **Project → Select Parts Database** to choose or change the linked `.nextdb` file. The database path is then stored in your project file.

If you don't have a `.nextdb` file yet, you'll need to create it separately or obtain one from your organization.

You can change which parts database is linked at any time.

## BOM Templates {#bom-templates}

A project can optionally point to an `.xlsx` file that nextbom uses as a template when [exporting the BOM](workflow.md#generate-output). The template is filled in place: scalar placeholders (PCBA name, BOM version, engineer, …) are substituted everywhere they appear, and a single repeating row holding column placeholders (`?part_id`, `?qty`, …) is expanded into one row per BOM line.

When no template is set, nextbom writes a plain BOM sheet with a default column order.

Set or change the template via **Project → Set BOM Template**. To go back to the default layout, use **Project → Clear BOM Template**. The template path is stored in the `.nbp` project file.

For the full list of placeholder keys and how they are expanded, see [BOM template placeholders](workflow.md#bom-template-placeholders).

## Closing a Project

Use **File → Close Project**. If there are unsaved changes, you will be prompted to confirm.

## Project File Format

Projects are saved as `.nbp` files — plain JSON, human-readable. They store all metadata including a link to the `.nextdb` parts database file.

See [File Formats](file-formats.md#nbp-project-files) for the full structure.
