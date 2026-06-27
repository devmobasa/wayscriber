# AGENTS.md

## Scope
- Applies to configurator keybinding models under `configurator/src/models/keybindings/`.

## Architecture
- Owns draft keybinding state, parsing, field-level labels/config read/write, grouping, and tests.

## Invariants
- Keep action labels, categories, defaults, parsing, and display aligned with `src/config/keybindings/` and `src/config/action_meta/`.
- Invalid keybinding input should remain visible and actionable.

## Coupled Changes
- Keybinding model changes may affect configurator views/updates, main keybinding maps, action metadata, help overlay, command palette, docs, and tests.

## Validation
- Add focused keybinding model tests.
- Run main config/keybinding tests for shared action behavior changes.
