# AGENTS.md

## Scope
- Applies only to child modules under `src/config/keybindings/`.
- The sibling root `src/config/keybindings.rs` is governed by `src/config/AGENTS.md`.

## Architecture
- `actions.rs` owns action enums.
- `binding.rs` owns key binding parsing/display.
- `defaults/` owns default bindings.
- `config/` owns typed keybinding config and per-domain action maps.

## Invariants
- Keep action names, defaults, map construction, parsing, display, and validation coherent.
- Do not add or rename actions without updating action metadata, help/command UI, docs, and configurator keybinding UI.

## Coupled Changes
- Keybinding changes may affect `src/input/`, `src/config/action_meta/`, `src/ui/help_overlay/`, command palette, configurator keybinding models/views, and docs.

## Validation
- Add keybinding parser/map tests for behavior changes.
- Run config and input action tests for action changes.
