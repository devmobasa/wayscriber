# Tools

Helper scripts for development, installation, packaging, and release workflows.

## Development

- **build.sh** - Build wayscriber release binary
  - Runs `cargo build --release --bins`
  - Usage: `./tools/build.sh`

- **run.sh** - Run daemon for development
  - Runs the release binary in daemon mode with `RUST_LOG=info`
  - Usage: `./tools/run.sh`

- **test.sh** - Run test suite
  - Runs `cargo test --workspace`
  - Usage: `./tools/test.sh`

- **code-health-report.sh** - Report local maintainability metrics
  - Reports Rust files over 500 lines, functions over 120 lines, production unwrap/expect/panic/unsafe markers, selected allowances, and direct `fs::write` usage
  - Does not fail on reported findings; intended for baseline visibility before adding quality gates
  - Usage: `./tools/code-health-report.sh`

- **check-rust-source-coverage.py** - Reject Rust sources outside the supported Cargo module graph
  - Uses current rustc dep-info from all-target/all-feature and no-default-feature checks
  - Runs as a hard gate in local and GitHub CI
  - Usage: `./tools/check-rust-source-coverage.py`

- **reload-daemon.sh** - Restart running daemon
  - Kills and restarts the daemon to pick up config/code changes
  - Usage: `./tools/reload-daemon.sh`

## Installation

- **install.sh** - Full installation script
  - Builds and installs binary to `/usr/bin` (or `$WAYSCRIBER_INSTALL_DIR`)
  - Sets up config directory with example config
  - Optionally configures systemd service or Hyprland autostart
  - Usage: `./tools/install.sh`

- **install-configurator.sh** - Install configurator only
  - Builds and installs wayscriber-configurator
  - Usage: `./tools/install-configurator.sh`

- **fetch-all-deps.sh** - Prefetch dependencies
  - Fetches all crates for offline/frozen builds
  - Usage: `./tools/fetch-all-deps.sh`

## Version & Release

- **bump-version.sh** - Bump version numbers
  - Updates Cargo.toml, configurator/Cargo.toml, the workspace Cargo.lock, PKGBUILD, and .SRCINFO
  - flake.nix package version follows Cargo.toml automatically
  - Auto-increments patch version if no version specified
  - Supports MAJOR.MINOR.PATCH.HOTFIX for packaging-only hotfix releases
  - Usage: `./tools/bump-version.sh [--dry-run] [new_version]`

- **check-version-consistency.sh** - Check release metadata alignment
  - Verifies Cargo manifests, the workspace lockfile, packaging metadata, and flake version sourcing
  - With `--release-version X.Y.Z[.N]`, rejects tags that do not match Cargo or an explicit packaging hotfix of Cargo
  - Usage: `bash tools/check-version-consistency.sh [--release-version X.Y.Z[.N]]`

Packaging-only hotfix policy:
- Normal releases use one version everywhere: Cargo, package metadata, Git tags, and artifacts all use `X.Y.Z`.
- Hotfix releases may use `X.Y.Z.N` only when the Cargo version is still `X.Y.Z`. In that case, `packaging/PKGBUILD`, `packaging/.SRCINFO`, release artifacts, and AUR metadata use `X.Y.Z.N`; Cargo manifests and `flake.nix` stay on `X.Y.Z`.
- Repo `packaging/PKGBUILD` and `packaging/.SRCINFO` are templates and keep `sha256sums=('SKIP')` because the final GitHub tag archive checksum can only be computed after the tag exists. AUR automation writes the real checksum into external AUR metadata.
- Release builds set `WAYSCRIBER_RELEASE_VERSION`, so packaged binaries report the release artifact version. Nix builds follow Cargo and report `X.Y.Z` unless the Cargo version itself is bumped.

- **create-release-tag.sh** - Create git tag (local only)
  - Creates annotated tag `v<version>` without pushing
  - Requires clean working tree
  - Runs version consistency checks before tagging
  - Usage: `./tools/create-release-tag.sh <version>` (X.Y.Z or X.Y.Z.N)

- **publish-release-tag.sh** - Create and push git tag
  - Creates annotated tag and pushes to origin
  - Auto-detects version from Cargo.toml if not specified
  - Runs version consistency checks before tagging
  - Usage: `./tools/publish-release-tag.sh [--version X.Y.Z[.N]] [--dry-run]`

## Packaging

- **package.sh** - Build distribution packages
  - Builds release binaries and packages into tar/deb/rpm
  - Generates checksums.txt and manifest.json
  - Usage: `./tools/package.sh [--version <ver>] [--formats tar,deb,rpm]`

- **build-package-repos.sh** - Build apt/rpm repositories
  - Assembles Debian (apt) and Fedora (dnf/yum) repos from built packages
  - Handles GPG signing for packages and repo metadata
  - Usage: `./tools/build-package-repos.sh`
  - Env: `ARTIFACT_ROOT`, `OUTPUT_ROOT`, `GPG_PRIVATE_KEY_B64`, etc.

## AUR (Arch User Repository)

- **update-aur.sh** - Interactive AUR update
  - Updates PKGBUILD, tests build locally, pushes to AUR
  - Prompts for confirmation at each step
  - Usage: `./tools/update-aur.sh`

- **update-aur-from-manifest.sh** - CI-friendly AUR update
  - Updates multiple AUR packages using checksums from manifest.json
  - Designed for CI automation after artifacts are built
  - Usage: `./tools/update-aur-from-manifest.sh --version <ver> --manifest dist/manifest.json --push`

---

## Notes

All scripts work from any location in the project.

### Potential Overlaps

The release/tag scripts have overlapping functionality:
- `create-release-tag.sh` + push = `publish-release-tag.sh`

Use the individual scripts in sequence for full releases when you need explicit control over each step.
