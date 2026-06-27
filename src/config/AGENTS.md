# AGENTS.md

## Scope
- Applies to configuration types, defaults, validation, I/O, config paths, schema export, keybindings, and action metadata.
- Parent guidance here covers split root `keybindings.rs`; `keybindings/AGENTS.md` covers only child modules under `keybindings/`.

## Architecture
- `types/` owns config structs and defaults.
- `validate/` owns clamping, warnings, and user-provided config validation.
- `keybindings.rs` and `keybindings/` adapt action bindings, defaults, parsing, display, and per-domain maps.
- `action_meta/` owns human-facing action names, labels, descriptions, and categories.
- `schema.rs` exports JSON schema behind `config-schema`.

## Invariants
- Preserve serde compatibility, documented defaults, validation warning behavior, and feature-gated schema output.
- Do not silently drop invalid user config; validation should make coercion or rejection explicit.
- Keep action labels/categories aligned with help overlay, command palette, keybindings, configurator labels, and docs.

## Coupled Changes
- Any config field change may require `config.example.toml`, `docs/CONFIG.md`, configurator draft/models/views/update code, tests, and schema behavior.
- Keybinding/action changes may require `src/input/`, `src/ui/`, toolbar model/rendering, configurator keybinding UI, and docs.

## Validation
- Use config-focused tests under `src/config/tests/` and keybinding tests for parser/map changes.
- Run schema tests when changing `config-schema` output.
- Run full local CI for broad config or feature-gate changes.
