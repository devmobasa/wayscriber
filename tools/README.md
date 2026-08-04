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

- **cargo-lanes.json** - The Cargo build/lint/test matrix, as data
  - Declares the lanes (`root-full`, `config-baseline`, `workspace-minimal`, `config-modern`), the ordered operations each consumer runs, the entry points that must call the driver, and the non-lane Cargo commands that stay raw
  - Every operation stores its complete Cargo argv, so flags such as `--all-targets` or `-D warnings` belong to one consumer and never leak into another. `clean` stays lenient on purpose
  - `config-modern` is the only lane that enables `adw-modern`; it needs libadwaita 1.7 or newer, so only the Arch job runs it
  - Change the matrix here, never in a caller

- **cargo_lanes.py** - Loader for `cargo-lanes.json`
  - Stdlib-only Python (3.10 or newer), imported by the driver and the checkers
  - Validates the schema strictly and returns typed lanes, consumers, operations, and entry-point expectations
  - Not a command; it is imported, not run

- **run-cargo-consumer.py** - Run one consumer's Cargo operations
  - Runs the operations in manifest order, labels each one, streams its output, and stops at the first failure
  - Usage: `./tools/run-cargo-consumer.py <consumer>` (run with no argument to list the consumers)

- **check-cargo-lanes.py** - Guard the configurator feature graph and the Cargo entry points
  - Feature and floor guard: reads live `cargo metadata` and asserts the declared feature edges, that the `default` closure cannot reach `adw-modern`, that each lane resolves exactly the libadwaita floor it declares (this catches a transitive dependency raising the floor), and that every declared configurator feature is enabled by some lane's resolved closure
  - Floor resolution is metadata-only, so the modern lane is verified on machines with no libadwaita 1.7 runtime
  - Entry-point contract: each entry point must call the driver exactly once per routed consumer, must not contain the raw commands the manifest replaced, and must still contain the allowlisted non-lane Cargo commands
  - `--self-test` replays the stored fixtures in `tools/fixtures/cargo-lanes/` instead of the working tree
  - Runs as a hard gate in `tools/lint-and-test.sh` and GitHub CI
  - Usage: `./tools/check-cargo-lanes.py [--self-test]`

- **fixtures/cargo-lanes/** - Inputs for the `check-cargo-lanes.py` self-test
  - `feature-cases.json` and `metadata/` hold `cargo metadata`-shaped documents for one healthy case and the five negative cases (default gaining `adw-modern`, `adw-modern` moving to `v1_8`, the direct libadwaita edge moving to `v1_5`, an unrouted declared feature, and a transitive dependency raising the baseline floor)
  - `entry-point-cases.json` and `entry-points/` hold workflow texts for one complete case and five broken ones
  - Each case states whether it must pass or fail, and a failing case states the message it must produce
  - See `tools/fixtures/cargo-lanes/README.md`

- **check-rust-source-coverage.py** - Reject Rust sources outside the supported Cargo module graph
  - Uses current rustc dep-info from the `source-coverage` vectors in `cargo-lanes.json` (`root-full`, `config-baseline`, `workspace-minimal`), so this gate cannot drift away from the lint/test matrix
  - Those vectors have no modern lane, because Ubuntu cannot compile it. The compensating rule is that no Rust source file may be reachable only under `adw-modern`; a modern-only file would show up here as uncovered
  - Runs as a hard gate in local and GitHub CI
  - Usage: `./tools/check-rust-source-coverage.py`

- **check-config-writers.py** - Reject config-write capability outside the configurator's Save
  - Scans `src/` and `configurator/src/` for the `config.toml` write primitives, exempting test sources and inline `#[cfg(test)]` items
  - Allows only `src/config/document.rs`, `src/config/io.rs`, and `configurator/src/app/io.rs`
  - Also checks those files keep the capability narrow (one public save, `pub(super)` primitives)
  - Runs as a hard gate in `tools/lint-and-test.sh`
  - Usage: `./tools/check-config-writers.py`

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
  - Builds a pinned gtk4-layer-shell static archive in a private prefix
  - Verifies the release ABI and Wayland interposition symbols before stripping
  - Builds release binaries and packages into tar/deb/rpm with retained license notices
  - Generates checksums.txt and manifest.json
  - Usage: `./tools/package.sh [--version <ver>] [--formats tar,deb,rpm]`

- **check-arch-installer-manifest.sh** - Check direct Arch installer compatibility
  - Strictly parses the installer's static allowlist as data; it never executes the installer, and unsupported manifest syntax fails closed
  - Requires the archive file set, modes, and service command to match what the installer accepts
  - Runs against the deployed installer during release packaging
  - Usage: `./tools/check-arch-installer-manifest.sh --installer FILE --archive FILE`

  When the tarball file manifest changes, build and check the new tarball locally, deploy
  the matching website installer, and only then push the `v*` tag. Until the tag publishes
  the new release, the updated live installer can fail closed against the previous release;
  keep that compatibility window short. A tag pushed before the installer deployment fails
  the release job's direct Arch installer check.

- **build-package-repos.sh** - Build apt/rpm repositories
  - Assembles Debian (apt) and Fedora (dnf/yum) repos from built packages
  - Handles GPG signing for packages and repo metadata
  - Usage: `./tools/build-package-repos.sh`
  - Env: `ARTIFACT_ROOT`, `OUTPUT_ROOT`, `GPG_PRIVATE_KEY_B64`, etc.

## nixpkgs

Wayscriber is packaged in `nixpkgs`, where version bumps are opened
automatically by the nixpkgs-update bot. The bot only rewrites the version and
hashes, so build-level changes still need a pull request from us. See
`packaging/nixpkgs/README.md`.

- **check-nixpkgs-recipe.py** - Check the nixpkgs build declares what the default features need
  - Uses locked Cargo metadata, so it works with Python 3.10 without an extra TOML parser
  - Maps every direct normal Cargo dependency, including target-specific dependencies, to the nixpkgs system packages it links
  - Keeps required native inputs, including the GTK application wrapper, aligned between the recipe and flake
  - Fails when a Linux default-feature dependency is missing from `packaging/nixpkgs/package.nix` or `flake.nix`
  - Fails on any new direct normal dependency until its system requirements are declared
  - Runs as a hard gate in GitHub CI and before release packaging
  - Usage: `./tools/check-nixpkgs-recipe.py`

## AUR (Arch User Repository)

- **update-aur.sh** - Interactive AUR update
  - Updates PKGBUILD, tests build locally, pushes to AUR
  - Prompts for confirmation at each step
  - Usage: `./tools/update-aur.sh`

- **update-aur-from-manifest.sh** - CI-friendly AUR update
  - Updates multiple AUR packages using checksums from manifest.json
  - Designed for CI automation after artifacts are built
  - `wayscriber` and `wayscriber-bin` are patched in place. `wayscriber-configurator` is different: its whole recipe is rendered from the checked-in template pair, validated, and only then installed over the clone
  - The configurator channel is required. A missing `--config-dir` is a hard error unless `--no-configurator` says the skip is deliberate
  - `pkgrel` is read before the script enters a clone, so a relative `--config-dir` (which is what release CI passes) resolves against the caller's directory rather than against the clone
  - `--source-sha256` / `AUR_SOURCE_ARCHIVE_SHA256` supply the tag archive checksum instead of downloading it; offline fixtures use this, and every download failure is fatal and named
  - Usage: `./tools/update-aur-from-manifest.sh --version <ver> --manifest dist/manifest.json --push`

- **packaging/aur/wayscriber-configurator/** - Template pair for the external configurator AUR package
  - `PKGBUILD.tmpl` and `.SRCINFO.tmpl` are the only reviewable copy of a recipe that otherwise lives in a repository this checkout does not contain
  - Tokens `@VERSION@`, `@PKGREL@`, and `@SOURCE_SHA256@` are substituted at release time
  - `.SRCINFO.tmpl` is generated: render `PKGBUILD.tmpl`, run `makepkg --printsrcinfo` on the render, then put the tokens back. Never hand-edit it
  - `packaging/**` is gitignored, so the whole parent chain is re-included in `.gitignore`; `check-aur-templates.py` proves that with a `git check-ignore --no-index` exit-1 gate. `--no-index` is the load-bearing half: the pair is tracked, and for a tracked file the default form reports nothing whatever the ignore rules say

- **check-aur-templates.py** - Guard the external configurator AUR recipe
  - Renders the checked-in templates with fixture values and asserts: no unresolved token, the configurator build command passes `--features adw-modern` and no other build command does, `depends` contains `libadwaita>=1.7`, `gtk4`, and `libxkbcommon`, `makedepends` contains `cargo`, the `.SRCINFO` structure is well formed, and the two files agree on every field `.SRCINFO` can express
  - Agreement alone is not the test: the live external recipe agreed with its own `.SRCINFO` while declaring none of the GTK4 dependencies, so the required set is asserted outright
  - Reads both files with a conservative parser and never runs `bash eval` on a recipe
  - Structure means exactly one `pkgbase = wayscriber-configurator` and one `pkgname = wayscriber-configurator` section and nothing else: a duplicated section repeats fields the PKGBUILD already declares, so the agreement check cannot see it while makepkg reads a second package out of the recipe
  - `--pair DIR` validates an already-rendered pair; `update-aur-from-manifest.sh` uses it on its temporary render before anything reaches a clone
  - `--self-test` replays the fixtures for the rules a healthy tree cannot exercise: a throwaway repository whose tracked template pair is ignored because the `.gitignore` negation chain was deleted, and a rendered `.SRCINFO` with a package section repeated. Both must be rejected, and the ignore fixture also asserts that the default `git check-ignore` still accepts it, since that disagreement is what `--no-index` exists for
  - Needs no makepkg, so it runs as a hard gate in `tools/lint-and-test.sh` and hosted Ubuntu CI
  - Usage: `./tools/check-aur-templates.py [--pair DIR | --self-test]`

- **check-srcinfo-canonical.py** - Compare a checked-in .SRCINFO with what makepkg generates
  - Runs the real `makepkg --printsrcinfo` on the PKGBUILD and compares. Parsed field-multiset equality blocks; a byte-level difference is a warning that names the regenerate task, so a rolling makepkg serialization change cannot fail unrelated pull requests
  - Repeat `--token NAME=VALUE` to compare the AUR template pair without contacting the AUR
  - A relative `--pkgbuild`/`--srcinfo` is read from the repository root, so the documented commands work from any subdirectory; an absolute path is used as given
  - makepkg refuses to run as root, so `--builder-user` runs it through `runuser` in a directory owned by that account
  - Needs makepkg, so it runs in the `Configurator modern (Arch)` job (for both `packaging/PKGBUILD` and the template pair), not in the portable lint script
  - Usage: `./tools/check-srcinfo-canonical.py --pkgbuild FILE --srcinfo FILE [--token NAME=VALUE] [--builder-user USER]`

- **pkgbuild_meta.py** - Reader for PKGBUILD and .SRCINFO metadata
  - Stdlib-only Python (3.10 or newer), shared by both AUR checkers
  - Accepts only the declarative subset the repository's recipes use and raises on anything else, so an unsupported construct fails loudly instead of being silently dropped
  - Not a command; it is imported, not run

---

## Notes

All scripts work from any location in the project.

### Potential Overlaps

The release/tag scripts have overlapping functionality:
- `create-release-tag.sh` + push = `publish-release-tag.sh`

Use the individual scripts in sequence for full releases when you need explicit control over each step.
