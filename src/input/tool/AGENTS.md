# AGENTS.md

## Scope
- Applies to tool catalog, kinds, profiles, settings, drag behavior, and tool tests under `src/input/tool/`.

## Architecture
- `kind.rs`, `catalog.rs`, `profile.rs`, `settings.rs`, and `drawing.rs` define tool identity and behavior.
- `drag.rs` owns drag-specific behavior.

## Invariants
- Tool semantics must stay coherent across input state, drawing/rendering, toolbar controls, config defaults, action metadata, and keybindings.
- Preserve serialized/config-facing tool names unless a migration/doc update is intentional.

## Coupled Changes
- Tool changes may affect `src/config/types/`, `src/config/keybindings/`, `src/config/action_meta/`, `src/ui/toolbar/`, `src/backend/wayland/toolbar/`, `src/draw/`, configurator field models/views/search, docs, and tests.

## Validation
- Add tool tests under `src/input/tool/` or input state tests for behavior changes.
- Run config/keybinding tests when changing tool actions or defaults.
