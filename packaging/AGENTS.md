# AGENTS.md

## Scope
- Applies to packaging manifests, desktop files, service unit, icons, and package metadata under `packaging/`.

## Architecture
- Package manifests describe the wayscriber binary and configurator package outputs.
- `PKGBUILD` and `.SRCINFO` represent Arch/AUR packaging metadata.
- `aur/wayscriber-configurator/{PKGBUILD,.SRCINFO}.tmpl` is the template pair for the external `wayscriber-configurator` AUR package. That repository is not part of this checkout, so the templates are the only reviewable copy of its recipe; `tools/update-aur-from-manifest.sh` renders and installs them on release.
- `nixpkgs/package.nix` mirrors the recipe owned by nixpkgs; it is a submission draft, not a build input.
- Desktop files, icons, and `wayscriber.service` define installed desktop and daemon integration.

## Invariants
- Keep `package.wayscriber.yaml`, `package.configurator.yaml`, `PKGBUILD`, `.SRCINFO`, desktop files, service unit, icons, and release scripts aligned.
- A new system library needed by an output both recipes build — that is, the `wayscriber` binary — must reach `nixpkgs/package.nix` and `flake.nix` together; the nixpkgs bot bumps versions but never build inputs.
- `nixpkgs/package.nix` ships the `wayscriber` binary only, so a system library only the configurator needs (libadwaita and anything else that arrives with it) belongs in `flake.nix` alone and must not be added to the nixpkgs recipe.
- The configurator's libadwaita cargo feature floor (`v1_4` in `configurator/Cargo.toml`) must stay buildable on the release runner's Ubuntu LTS (24.04 ships libadwaita 1.5); raise it only together with a runner/base-image bump and the workflow dependency lists.
- `adw-modern` is never a configurator default feature. It is opt-in per channel and nothing may make it reachable from the default feature closure.
- deb and rpm builds never pass `adw-modern`. Those artifacts are the baseline channel; they declare `libadwaita-1-0 (>= 1.4)` and `libadwaita >= 1.4`, and the release workflow asserts both on the produced packages.
- The direct libadwaita cargo edge stays on `v1_4` while Ubuntu 24.04 is supported. `adw-modern` is the only thing that raises it, and only to `v1_7`.
- A channel that enables `adw-modern` must pair it with a runtime floor of at least 1.7: `packaging/PKGBUILD` and the configurator AUR template declare `libadwaita>=1.7`, and `flake.nix` enables the feature only when the channel's libadwaita is new enough.
- Packaging hotfix versions may differ from Cargo versions only according to the existing versioning policy.
- Do not change daemon service semantics without checking daemon runtime and configurator daemon setup.

## Coupled Changes
- Packaging changes may require `tools/`, `.github/`, setup docs, `src/systemd_user_service.rs`, `src/shortcut_hint.rs`, and configurator daemon setup updates.

## Validation
- Run `tools/check-version-consistency.sh` and `tools/test-package-repo-layout.sh` for package/version changes.
- Run `tools/check-nixpkgs-recipe.py` when dependencies, default features, or Nix build inputs change.
- Run `tools/check-aur-templates.py` for any change to `aur/wayscriber-configurator/`.
- Run `tools/test-release-packaging.sh` for changes to package manifests, `PKGBUILD`, or the AUR templates.
- On a machine with makepkg, run `tools/check-srcinfo-canonical.py` for `PKGBUILD`/`.SRCINFO` edits. Regenerate a stale `.SRCINFO` (`tools/bump-version.sh` for the checked-in pair); never relax the comparison.
- Run `git diff --check` for metadata-only edits.
