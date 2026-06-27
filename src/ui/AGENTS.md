# AGENTS.md

## Scope
- Applies only to child modules under `src/ui/`.
- The sibling root `src/ui.rs` is governed by `src/AGENTS.md`.

## Architecture
- Owns Cairo-rendered overlay UI pieces: status, help overlay, command palette, context menu, board picker, properties panel, radial menu, onboarding card, color picker popup, toasts, tour UI, and primitives.
- `toolbar/` owns input-side toolbar model/snapshot/apply plumbing, distinct from runtime backend toolbar rendering.

## Invariants
- Keep layout calculations deterministic and avoid text overlap on small surfaces.
- Prefer shared constants/primitives where present.
- Keep UI rendering side-effect-light; durable state changes belong in input/backend owners.

## Coupled Changes
- UI changes may affect `src/input/`, `src/backend/wayland/state/render/`, action metadata, keybindings, toolbar rendering, docs, and tests.

## Validation
- Add focused layout/render tests for complex UI paths where possible.
- Run targeted UI/input tests for command palette, board picker, help overlay, or toolbar behavior.
