# AGENTS.md

## Scope
- Applies to screenshot capture, image/document delivery, file save, clipboard delivery, portal support, and capture tests under `src/capture/`.

## Architecture
- `manager.rs` bridges synchronous Wayland state and async capture/delivery work.
- `pipeline.rs` owns capture, image delivery, document delivery, save, and clipboard flows.
- `dependencies.rs` provides traits for testable capture sources, savers, and clipboard implementations.
- `sources/` chooses compositor-specific or portal capture sources.

## Invariants
- Preserve cancellation semantics; user cancellation is not a hard failure.
- Keep async capture work out of Wayland event-loop blocking paths.
- Preserve desktop backdrop and portal behavior across feature gates.

## Coupled Changes
- Capture changes may affect Wayland frozen/zoom paths, notifications, canvas export, clipboard behavior, docs, and tests.

## Validation
- Add focused tests under `src/capture/tests/`.
- Run targeted capture tests for manager/pipeline/source changes.
- Run full local CI for broad async or feature-gate changes.
