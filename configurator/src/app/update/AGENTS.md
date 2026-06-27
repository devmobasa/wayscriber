# AGENTS.md

## Scope
- Applies to configurator update handlers under `configurator/src/app/update/`.

## Architecture
- Update modules handle `Message` variants and return `iced::Task<Message>` for async work.
- Modules are split by config sections and workflows such as boards, daemon, fields, presets, render profiles, session catalog, and tabs.

## Invariants
- Keep update routing centralized and explicit.
- Preserve non-blocking I/O/process work through tasks.
- Surface validation errors instead of silently coercing invalid input.

## Coupled Changes
- Update changes may affect messages, state, views, models, search, docs, and tests.

## Validation
- Add focused update tests where available.
- Run `cargo test -p wayscriber-configurator` for broad app behavior changes.
