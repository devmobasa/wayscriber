# AGENTS.md

## Scope
- Applies to repository scripts under `tools/`.

## Architecture
- Scripts support build, install, lint/test, versioning, packaging, release tags, package repository generation, daemon reload, and dependency fetching.
- Scripts should resolve the repository root and work from any starting directory.

## Invariants
- Preserve release/version/package semantics, including packaging-only hotfix behavior.
- Keep `tools/lint-and-test.sh` aligned with CI. Both call `./tools/run-cargo-consumer.py`,
  so alignment means editing `tools/cargo-lanes.json`, not the callers.
- `tools/cargo-lanes.json` is the only place a Cargo package/feature matrix is declared.
  A new raw `cargo` command in an entry point must become a lane operation or an
  `allowed_non_lane_cargo` entry with a reason; `./tools/check-cargo-lanes.py` fails otherwise.
- `check-rust-source-coverage.py` consumes the manifest's `source-coverage` vectors, so it
  cannot drift from the lint/test matrix. Those vectors have no modern libadwaita lane;
  the compensating rule is that no Rust source file may be reachable only under `adw-modern`.
- Every rule in `check-cargo-lanes.py` must keep a fixture in `tools/fixtures/cargo-lanes/`
  that proves it rejects what it claims to reject.
- Same obligation for `check-aur-templates.py`: a gate a healthy tree cannot exercise
  (the unignored proof, the `.SRCINFO` section counts) keeps a `--self-test` fixture.
- Avoid platform-specific assumptions unless the script is explicitly platform-specific.

## Coupled Changes
- Version and packaging scripts must stay aligned with `tools/README.md`, `packaging/`, `.github/`, `Cargo.toml`, and release docs.
- Install/reload scripts may affect setup docs and daemon service behavior.

## Validation
- Run changed scripts directly when safe.
- Run `./tools/lint-and-test.sh` for changes to lint/test behavior.
- Use `git diff --check` for docs/script-only edits.
