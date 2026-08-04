# AGENTS.md

## Scope
- Applies to configurator update handlers under `configurator/src/app/update/`.

## Architecture
- Update modules handle `Message` variants and return `Vec<Effect>` for async work; the component runs each effect exactly once.
- Modules are split by config sections and workflows such as boards, daemon, fields, presets, render profiles, session catalog, and tabs.

## Invariants
- Keep update routing centralized and explicit.
- Preserve non-blocking I/O/process work through effects.
- Surface validation errors instead of silently coercing invalid input.

## Coupled Changes
- Update changes may affect messages, state, pages, models, search, docs, and tests.

## Validation
- Add focused update tests where available.
- Run `cargo test -p wayscriber-configurator` for broad app behavior changes.
