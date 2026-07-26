# AGENTS.md

## Scope
- Applies to the separate Wayland about dialog under `src/about_window/`.

## Architecture
- `content.rs` owns the dialog's wording and the `UpdateState` machine (pure).
- `layout.rs` turns content into a `Plan` of rectangles and baselines (pure).
- `interaction.rs` owns the one `Element` list that focus order, hit testing,
  and painting all read, so keyboard and pointer cannot disagree.
- `diagnostics.rs` builds the "Copy diagnostics" payload; `icon.rs` decodes the
  embedded app icon.
- `state.rs` owns about-window state and the actions handlers trigger.
- `handlers/` owns Wayland protocol callbacks for the dialog.
- `render/` owns Cairo drawing and text/widget helpers; `render/draw.rs` only
  decides color and paint order.
- `clipboard.rs` supports about-dialog clipboard behavior.

## Invariants
- Keep about-window runtime separate from the annotation overlay backend unless a shared abstraction is real.
- Preserve foreground/fullscreen safety for any launch, focus, or window behavior.
- Keep protocol handlers thin and rendering deterministic.
- Chrome colors come from `crate::ui::theme`, never hardcoded literals.
- Every outbound link points at wayscriber.com (no code-host links); the update
  card never installs anything.
- The window is sized from `layout::plan`, so adding a row must not need a
  constant edit elsewhere.
- Blocking work (the update fetch) runs in the event loop, never in a handler.

## Coupled Changes
- About dialog changes may affect app metadata, clipboard behavior, docs, and Wayland handler dependencies.
- Update-status wording is shared with `src/update_check/`; the dialog only
  reads its cache.

## Validation
- Add focused tests if logic becomes testable (`content`, `layout`,
  `interaction`, and `diagnostics` are all unit-testable without a compositor).
- Run clippy for handler/render changes.
