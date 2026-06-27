# AGENTS.md

## Scope
- Applies to configurator Iced views under `configurator/src/app/view/`.

## Architecture
- View modules build UI for config sections, daemon setup, session settings, render profiles, themes, presets, boards, and shared widgets.
- Views should render state and emit messages; side effects belong in updates or app helper modules.

## Invariants
- Keep labels, defaults, validation states, section ordering, and search-visible text aligned with models/docs.
- Do not do file/process work directly from view code.
- Keep reusable widgets consistent across sections.

## Coupled Changes
- View changes may affect models, update messages, search terms, docs, and tests.

## Validation
- Run configurator tests for model/view coupling changes.
- Manually launch the configurator only when foreground app launch is explicitly acceptable.
