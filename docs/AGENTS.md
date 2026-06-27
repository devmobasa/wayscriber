# AGENTS.md

## Scope
- Applies to repository documentation under `docs/`.

## Architecture
- `docs/CONFIG.md` is the user-facing config guide.
- `docs/SETUP.md` documents installation/setup workflows.
- `docs/codebase-overview.md` is architecture reference material for the main crate.
- `docs/temp/` is draft and planning material unless a file explicitly says otherwise.

## Invariants
- Keep user-facing docs aligned with current CLI behavior, config behavior, `config.example.toml`, README usage, and configurator UI.
- Do not treat `docs/temp/` drafts as authoritative product documentation.

## Coupled Changes
- Config docs may need updates when `src/config/`, `config.example.toml`, configurator fields, action metadata, or keybindings change.
- Setup docs may need updates when packaging, daemon service files, shortcut setup, or install scripts change.

## Validation
- For docs-only changes, run `git diff --check`.
- Run code/tests only when docs are generated from code or the change touches scripts/config behavior.
