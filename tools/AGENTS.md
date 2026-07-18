# AGENTS.md

## Scope
- Applies to repository scripts under `tools/`.

## Architecture
- Scripts support build, install, lint/test, versioning, packaging, release tags, package repository generation, daemon reload, and dependency fetching.
- Scripts should resolve the repository root and work from any starting directory.

## Invariants
- Preserve release/version/package semantics, including packaging-only hotfix behavior.
- Keep `tools/lint-and-test.sh` aligned with CI.
- Keep `check-rust-source-coverage.py` aligned with the workspace's all-feature and
  no-default-feature target matrix; intentional exceptions must be narrow and documented.
- Avoid platform-specific assumptions unless the script is explicitly platform-specific.

## Coupled Changes
- Version and packaging scripts must stay aligned with `tools/README.md`, `packaging/`, `.github/`, `Cargo.toml`, and release docs.
- Install/reload scripts may affect setup docs and daemon service behavior.

## Validation
- Run changed scripts directly when safe.
- Run `./tools/lint-and-test.sh` for changes to lint/test behavior.
- Use `git diff --check` for docs/script-only edits.
