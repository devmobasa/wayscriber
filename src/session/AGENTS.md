# AGENTS.md

## Scope
- Applies to session persistence, named sessions, primary file validation, options, locks, snapshots, storage, artifacts, catalog metadata, clear/inspect flows, and path safety.
- Parent guidance here covers split roots such as `artifacts.rs` and `catalog.rs`.

## Architecture
- `options/` builds persistence targets and output identity behavior.
- `snapshot/` captures, applies, loads, saves, compresses, and recovers session state.
- `storage/` handles clear/inspect operations and stored-session data.
- `artifacts.rs` and `artifacts/` own sidecar/artifact movement and rollback.
- `catalog.rs` and `catalog/` own named-session identity and metadata.

## Invariants
- Preserve lock behavior, backup/recovery behavior, clear boundaries, rollback on artifact moves, and named-session catalog identity.
- Reject symlinks, directories, and special files where forbidden by named-session rules.
- Runtime session open/save-as/clear behavior must validate paths before mutating active state.

## Coupled Changes
- Session changes may affect `src/backend/wayland/session.rs`, input session preflight, daemon named-session switching, configurator session catalog operations, and `src/paths/`.
- Snapshot format changes must consider backward compatibility and recovery behavior.

## Validation
- Add focused tests under `src/session/tests/`, `src/session/snapshot/`, `src/session/storage/`, `src/session/artifacts/`, or `src/session/catalog/`.
- Use targeted session test filters for persistence changes.
- Run full local CI for changes that affect storage, locks, named sessions, or snapshot compatibility.
