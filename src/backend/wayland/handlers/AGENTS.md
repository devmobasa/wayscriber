# AGENTS.md

## Scope
- Applies to Wayland/Smithay protocol handlers under `handlers/`.

## Architecture
- Handler files translate protocol callbacks into `WaylandState`, `InputState`, capture, surface, and render operations.
- Pointer, keyboard, tablet, touch, output, layer, registry, buffer, screencopy, seat, SHM, and XDG behavior is split by protocol area.
- Translation helpers such as keyboard keysym mapping should stay testable and isolated.

## Invariants
- Keep handlers thin; do not bury durable business logic in protocol callbacks.
- Preserve coordinate transforms, modifier synchronization, seat/device lifetimes, frame/callback ordering, and tablet feature gating.
- Avoid blocking protocol callback paths.

## Coupled Changes
- Input event changes may require `src/input/` state tests.
- Tablet changes must stay coherent with `tablet-input`.
- Screencopy/portal changes may affect frozen, zoom, and capture behavior.

## Validation
- Add focused tests near translation helpers where possible.
- Run clippy and relevant input/backend tests for protocol behavior changes.
