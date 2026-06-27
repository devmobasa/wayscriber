# AGENTS.md

## Scope
- Applies to packaging manifests, desktop files, service unit, icons, and package metadata under `packaging/`.

## Architecture
- Package manifests describe the wayscriber binary and configurator package outputs.
- `PKGBUILD` and `.SRCINFO` represent Arch/AUR packaging metadata.
- Desktop files, icons, and `wayscriber.service` define installed desktop and daemon integration.

## Invariants
- Keep `package.wayscriber.yaml`, `package.configurator.yaml`, `PKGBUILD`, `.SRCINFO`, desktop files, service unit, icons, and release scripts aligned.
- Packaging hotfix versions may differ from Cargo versions only according to the existing versioning policy.
- Do not change daemon service semantics without checking daemon runtime and configurator daemon setup.

## Coupled Changes
- Packaging changes may require `tools/`, `.github/`, setup docs, `src/systemd_user_service.rs`, `src/shortcut_hint.rs`, and configurator daemon setup updates.

## Validation
- Run `tools/check-version-consistency.sh` and `tools/test-package-repo-layout.sh` for package/version changes.
- Run `git diff --check` for metadata-only edits.
