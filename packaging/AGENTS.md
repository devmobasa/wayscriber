# AGENTS.md

## Scope
- Applies to packaging manifests, desktop files, service unit, icons, and package metadata under `packaging/`.

## Architecture
- Package manifests describe the wayscriber binary and configurator package outputs.
- `PKGBUILD` and `.SRCINFO` represent Arch/AUR packaging metadata.
- `nixpkgs/package.nix` mirrors the recipe owned by nixpkgs; it is a submission draft, not a build input.
- Desktop files, icons, and `wayscriber.service` define installed desktop and daemon integration.

## Invariants
- Keep `package.wayscriber.yaml`, `package.configurator.yaml`, `PKGBUILD`, `.SRCINFO`, desktop files, service unit, icons, and release scripts aligned.
- A new system library needed by an output both recipes build — that is, the `wayscriber` binary — must reach `nixpkgs/package.nix` and `flake.nix` together; the nixpkgs bot bumps versions but never build inputs.
- `nixpkgs/package.nix` ships the `wayscriber` binary only, so a system library only the configurator needs (libadwaita and anything else that arrives with it) belongs in `flake.nix` alone and must not be added to the nixpkgs recipe.
- The configurator's libadwaita cargo feature floor (`v1_4` in `configurator/Cargo.toml`) must stay buildable on the release runner's Ubuntu LTS (24.04 ships libadwaita 1.5); raise it only together with a runner/base-image bump and the workflow dependency lists.
- Packaging hotfix versions may differ from Cargo versions only according to the existing versioning policy.
- Do not change daemon service semantics without checking daemon runtime and configurator daemon setup.

## Coupled Changes
- Packaging changes may require `tools/`, `.github/`, setup docs, `src/systemd_user_service.rs`, `src/shortcut_hint.rs`, and configurator daemon setup updates.

## Validation
- Run `tools/check-version-consistency.sh` and `tools/test-package-repo-layout.sh` for package/version changes.
- Run `tools/check-nixpkgs-recipe.py` when dependencies, default features, or Nix build inputs change.
- Run `git diff --check` for metadata-only edits.
