# AGENTS.md

## Scope
- Applies to the separate Wayland about dialog under `src/about_window/`.

## Architecture
- `state.rs` owns about-window state.
- `handlers/` owns Wayland protocol callbacks for the dialog.
- `render/` owns Cairo drawing and text/widget helpers.
- `clipboard.rs` supports about-dialog clipboard behavior.

## Invariants
- Keep about-window runtime separate from the annotation overlay backend unless a shared abstraction is real.
- Preserve foreground/fullscreen safety for any launch, focus, or window behavior.
- Keep protocol handlers thin and rendering deterministic.

## Coupled Changes
- About dialog changes may affect app metadata, clipboard behavior, docs, and Wayland handler dependencies.

## Validation
- Add focused tests if logic becomes testable.
- Run clippy for handler/render changes.
