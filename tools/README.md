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
  - Updates Cargo.toml, configurator/Cargo.toml, Cargo.lock, PKGBUILD, .SRCINFO
  - Auto-increments patch version if no version specified
  - Supports MAJOR.MINOR.PATCH.HOTFIX for packaging-only hotfix releases
  - Usage: `./tools/bump-version.sh [--dry-run] [new_version]`

- **create-release-tag.sh** - Create git tag (local only)
  - Creates annotated tag `v<version>` without pushing
  - Requires clean working tree
  - Usage: `./tools/create-release-tag.sh <version>` (X.Y.Z or X.Y.Z.N)

- **publish-release-tag.sh** - Create and push git tag
  - Creates annotated tag and pushes to origin
  - Auto-detects version from Cargo.toml if not specified
  - Usage: `./tools/publish-release-tag.sh [--version X.Y.Z[.N]] [--dry-run]`

- **release.sh** - Full release workflow
  - Bumps version, commits changes, creates tag, and pushes
  - All-in-one release script for maintainers
  - Usage: `./tools/release.sh <version> [--force-tag]`

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
- `bump-version.sh` + commit + `create-release-tag.sh` + push = `release.sh`

Consider using `release.sh` for full releases, or the individual scripts for more control.
