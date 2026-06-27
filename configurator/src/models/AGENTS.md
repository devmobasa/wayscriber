# AGENTS.md

## Scope
- Applies to configurator models under `configurator/src/models/`.

## Architecture
- Models convert editable UI fields to/from core wayscriber config and session types.
- `config/` owns draft config and conversion to core config.
- `fields/` owns reusable form field wrappers.
- `keybindings/` owns keybinding draft/parsing/labels.
- Color, session, search, daemon, tab, and utility models support app state and views.

## Invariants
- Invalid user input should surface as form errors, not silent coercion.
- Keep defaults, labels, parsing, and conversion aligned with main crate config/session/action metadata.

## Coupled Changes
- Model changes may affect configurator views/updates/search, main `src/config/`, `src/session/`, action metadata, docs, and tests.

## Validation
- Add focused model tests.
- Run `cargo test -p wayscriber-configurator` for broad model changes.
