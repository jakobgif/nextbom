<div align="center">

# nextbom

**A desktop app for creating Bills of Materials for electronic projects.**

[![CI](https://github.com/jakobgif/nextbom/actions/workflows/ci.yml/badge.svg)](https://github.com/jakobgif/nextbom/actions/workflows/ci.yml)

[Documentation](https://jakobgif.github.io/nextbom/) · [Report a Bug](https://github.com/jakobgif/nextbom/issues)

</div>

---

nextbom separates the concerns of *design* and *procurement*. In the design, components are assigned generic part IDs — not manufacturer part numbers. When generating a BOM, nextbom resolves those IDs to real MPNs from a separate `.nextdb` database file.

This means manufacturers can be swapped without touching the schematic, and per-project alternative parts are supported out of the box.

## Features

- Import component lists from CSV
- Resolve part IDs to manufacturer part numbers via a shared database
- Override resolutions per-project with project-specific alternatives
- Export a ready-to-use BOM

## Getting Started

See the **[documentation](https://jakobgif.github.io/nextbom/)** for workflow and file format details.

## Built With

- [Tauri 2](https://tauri.app) — native desktop shell
- [React 19](https://react.dev) + [TypeScript](https://www.typescriptlang.org) — UI
- [Rust](https://www.rust-lang.org) — backend logic
