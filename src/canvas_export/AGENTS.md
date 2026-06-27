# AGENTS.md

## Scope
- Applies to PNG/PDF canvas export code under `src/canvas_export/`.

## Architecture
- `png.rs` owns PNG export.
- `pdf.rs` and `pdf/` own PDF export and tests.
- `page.rs` and `pdf_labels.rs` support page metadata and PDF labels.

## Invariants
- Export from snapshots, not live mutable state.
- Preserve viewport origin, scale handling, persisted backdrop behavior, render profile remapping, PDF labels/layout, and page metadata.

## Coupled Changes
- Export changes may affect capture delivery, render profiles, session snapshots, board/page state, docs, and tests.

## Validation
- Add pixel-style tests for PNG regressions where practical.
- Add PDF header/layout/label tests for PDF behavior changes.
