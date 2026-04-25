# Projects

A **project** is the central object in nextbom. It manages your design metadata and links to the `.nextdb` parts database used to resolve part numbers.

## Creating a Project

Click **Create Project** on the home screen, or use **File → New Project**.

Fill in the fields:

| Field | Required | Description |
|---|---|---|
| **Title** | Yes | Human-readable name for the project. Used as the default filename when saving. |
| **Engineer** | No | Engineer or team responsible. Shown in the status bar. |
| **Project Specifics ID** | No | Identifier for a set of project-specific alternative parts. Leave blank if you are not using per-project alternatives. |

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
- **Project → Set Project Specifics**
- **Project → Select Parts Database** — link or re-link the project to a `.nextdb` parts database file

The design variant is not set here — it is entered when [creating a BOM working file](workflow.md#configure-bom) and automatically stored in the project for reuse next time.

The `.nextdb` file is the shared parts database that maps generic part IDs to manufacturer part numbers. See [File Formats](file-formats.md) to understand the relationship between `.nextbom` project BOMs and `.nextdb` parts databases.

## Linking the Parts Database {#linking-the-parts-database}

Before you can create a BOM file, your project must be linked to a `.nextdb` parts database. This database contains the mappings from generic part IDs to actual manufacturer part numbers.

Use **Project → Select Parts Database** to choose or change the linked `.nextdb` file. The database path is then stored in your project file.

If you don't have a `.nextdb` file yet, you'll need to create it separately or obtain one from your organization.

You can change which parts database is linked at any time.

## Closing a Project

Use **File → Close Project**. If there are unsaved changes, you will be prompted to confirm.

## Project File Format

Projects are saved as `.nbp` files — plain JSON, human-readable. They store all metadata including a link to the `.nextdb` parts database file.

See [File Formats](file-formats.md#nbp-project-files) for the full structure.
