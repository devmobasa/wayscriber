# AGENTS.md

## Scope
- Applies to runtime toolbar layout, hit testing, event types, rows, surfaces, rendering, widgets, and main toolbar state.

## Architecture
- `view/` is the declarative engine: `view/top.rs` builds the top strip as a typed `WidgetTree` of `WidgetNode`s (pure function of the `ToolbarSnapshot`); `view/popover.rs` places anchored popovers (shapes/options, overflow); `view/node.rs`/`view/tree.rs` define the node model and the legacy `HitRegion` adapter.
- `render/paint.rs` is the single painter over the tree; `render/side_palette/` still hand-renders the side palette pane-by-pane (Draw/Canvas/Session/Settings) and pushes hit regions while drawing.
- `layout/spec/` holds size constants and the surface-size math. The top strip's width and extra height come from the same tree walk the builder performs (`view::top::top_natural_width`/`top_extra_height`), so size and content cannot drift.
- `surfaces/` owns Wayland surface lifecycle (resize via `set_size`, no destroy/recreate), buffers, input regions (`sync_top_input_region` keeps popover-grown surfaces click-through outside bar + panels), hit testing, and surface rendering.
- `main/` owns runtime toolbar state and lifecycle.
- `events.rs`, `hit.rs`, and `rows.rs` connect runtime events, hit targets, and row helpers. `hit.rs` owns the 24px minimum-target inflation both paths share.

## Invariants
- Every rect exists once: the tree (top) or the render pass (side) is the only geometry source; no payload rects, no post-render coordinate fixups beyond the one uniform scale.
- Blue is reserved for the active tool/selected value; destructive actions are red and isolated; disabled buttons are dimmed and non-interactive.
- Section visibility resolves through `config::resolve_section_visibility` (explicit `items.shown`/`items.hidden` overrides over the layout-mode baseline); the nine `show_*` booleans are derived mirrors — never write them directly.
- Minimize leaves a restore tab; the restore controls are not customization item ids and must never become hideable.
- Colors live only in the contextual style pill; the top strip's islands read tools | presets | history | chrome. Width pressure drops the non-essential presets island first, then narrows the pill's swatches, then moves droppable tools/utilities into the overflow menu; Pen, Eraser, Undo/Redo, Clear, and chrome are never dropped.
- Config keys and item ids are additive-only; unknown ids round-trip through saves.

## Coupled Changes
- Runtime toolbar changes may affect `src/backend/wayland/state/toolbar.rs`, `src/backend/wayland/state/toolbar/`, `src/ui/toolbar/`, `src/input/`, toolbar icons, config toolbar settings, and action metadata.
- New config keys require coupled updates to `config.example.toml`, `docs/CONFIG.md`, and the configurator.

## Validation
- Add or update tests under `layout/tests/` (render-oracle hits for the side palette) and `view/top.rs` (tree assertions) for geometry or content changes.
- Run full local CI (`./tools/lint-and-test.sh`) for broad toolbar behavior changes.
