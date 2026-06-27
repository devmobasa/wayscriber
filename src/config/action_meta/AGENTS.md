# AGENTS.md

## Scope
- Applies to action metadata under `src/config/action_meta/`.

## Architecture
- `entries/` groups human-facing action labels, descriptions, and categories by domain.
- Metadata feeds help overlay, command palette, configurator labels/search, and documentation.

## Invariants
- Keep labels, descriptions, categories, action enum variants, keybinding defaults, and UI presentation synchronized.
- Avoid renaming user-facing actions without considering config/docs compatibility.

## Coupled Changes
- Action metadata changes may affect `src/config/keybindings/`, `src/input/`, `src/ui/help_overlay/`, command palette, configurator search/labels, and docs.

## Validation
- Run action metadata tests and relevant config/keybinding tests.
