# nextbom

A desktop application for creating Bills of Materials (BOMs) for electronic projects, built with Tauri 2, React 19, and TypeScript.

## Coding Guidelines

### Think Before Coding

Before implementing, state assumptions explicitly. If uncertain, ask. If multiple interpretations exist, present them — don't pick silently. If a simpler approach exists, say so. If something is unclear, stop, name what's confusing, and ask.

### Simplicity First

Minimum code that solves the problem. No features beyond what was asked, no abstractions for single-use code, no flexibility or configurability that wasn't requested, no error handling for impossible scenarios. Ask: "Would a senior engineer say this is overcomplicated?" If yes, rewrite it.

### Surgical Changes

Touch only what you must. Don't improve adjacent code, comments, or formatting. Don't refactor things that aren't broken. Match existing style. If you notice unrelated dead code, mention it — don't delete it.

When your changes create orphans: remove imports/variables/functions that *your* changes made unused. Don't remove pre-existing dead code unless asked. Every changed line should trace directly to the request.

### Goal-Driven Execution

Transform tasks into verifiable goals. For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
```
After Rust changes: run `cargo check` (and `cargo test` if logic changed). After TypeScript changes: run `npx tsc --noEmit`. Weak success criteria require constant clarification — define done before starting.

## Working with Subagents

Delegate self-contained tasks to background subagents so the main session stays focused on the primary work. A task is a good candidate for a subagent when it does not need the full conversation context and can be described completely in a short prompt.

Good examples:
- Writing or updating `docs/` pages while the main session is modifying code
- Running `cargo test` or checking for compile errors after a change
- Searching the codebase for all usages of a type or function before a refactor

Spawn these with `run_in_background: true` so the main session continues unblocked.

**Subagents cannot spawn their own subagents.** If a subagent needs to delegate further work, it must do that work itself instead.

## Git Style

Commit one file at a time (or one tightly related file group). Never bundle unrelated files into a single commit. Keep commit messages short and focused — one change, one message. Do not create long commit message chains.

Do not touch already-committed code unless the task requires it. No reformatting, no comment tweaks, no whitespace cleanup as a side effect. Before committing, review the full diff and remove any unintended changes.

Never commit "current state" snapshots or plan markdown files. These exist as working files only and must not enter git history. Before every commit, review what is staged and exclude any planning or status documents.

Always add Claude as co-author in every commit message:

```
Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
```

## Project Overview

nextbom separates the concerns of *design* and *procurement*. Designers assign generic part IDs to components; the app resolves these to manufacturer part numbers from a separate database during BOM generation. This enables changing manufacturers without touching the design, and supports per-project alternative parts.

## User Documentation

End-user-facing documentation lives in `docs/` and is published via MkDocs to GitHub Pages. The nav is defined in `mkdocs.yml`.

Any feature, behaviour, or concept that a user needs to understand must be documented there — not just in code comments. This includes:
- How to create and manage projects
- File formats (`.nbp` project files, `.nextbom` database files, CSV import format)
- Workflow steps (importing a BOM, generating output, project-specific alternatives)
- Any non-obvious UI behaviour

When implementing a user-facing feature, update or create the relevant page in `docs/` as part of the same piece of work. Keep the writing clear and non-technical — aimed at engineers using the tool, not developers building it.

## Development Reference

Architecture, tech stack, data structures, file formats, build commands, conventions, and Rust code quality rules are documented in [`docs/development.md`](docs/development.md).
