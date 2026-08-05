# AGENTS.md

## Scope
- Applies to GTK4/libadwaita page builders under `configurator/src/app/pages/`.

## Architecture
- Page modules build the sidebar content for configuration, daemon setup, sessions, render profiles, themes, presets, boards, and shared controls.
- Pages render state and emit messages; side effects belong in updates, effects, or app helper modules.

## Invariants
- Keep labels, defaults, validation states, section ordering, and search-visible text aligned with models and docs.
- Do not do file or process work directly from page code.
- Keep reusable page controls consistent across sections.

## Coupled Changes
- Page changes may affect models, update messages, search terms, docs, and tests.

## Validation
- Run configurator tests for model/page coupling changes.
- Manually launch the configurator only when foreground app launch is explicitly acceptable.
