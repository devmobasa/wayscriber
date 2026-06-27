# AGENTS.md

## Scope
- Applies to configurator config draft models, parsing, setters, board/render profile models, toolbar overrides, and conversion to core config.

## Architecture
- `draft/` loads editable state from core config.
- `to_config/` converts validated draft state back to `wayscriber::config::Config`.
- Section files mirror main config areas such as boards, presets, render profiles, and toolbar overrides.

## Invariants
- Keep draft defaults and conversion behavior aligned with `src/config/types/` and `src/config/validate/`.
- Preserve validation feedback; do not silently discard invalid user input.

## Coupled Changes
- Config model changes may require configurator views/updates/search, `config.example.toml`, `docs/CONFIG.md`, schema behavior, and tests.

## Validation
- Add or update tests under `configurator/src/models/config/`.
- Run main config tests when conversion behavior mirrors core config changes.
