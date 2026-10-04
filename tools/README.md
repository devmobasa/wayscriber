# Tools

Helper scripts for development, installation, packaging, and release workflows.

## C# automation

CI uses the file-based app at `tools/wayscriber.cs`. Some commands below exist
only there (the repository checks, the code-health report, and the version and
release-tag commands); the shell tools listed remain available for contributors
who do not have .NET installed. Production C# commands never call those scripts.
The repository uses no Python.

The exact SDK is pinned by `global.json` and installed by GitHub Actions through
`actions/setup-dotnet`. Nix does not provide .NET. Local users who choose the C#
route install the pinned SDK separately. Build the file app once, then reuse that
build:

```bash
dotnet build tools/wayscriber.cs
dotnet run tools/wayscriber.cs --no-build -- --help
```

The C# regression suite uses the same pattern:

```bash
dotnet build tools/wayscriber.tests.cs
dotnet run tools/wayscriber.tests.cs --no-build
```

For a one-command C# source installation, use the executable file app. It
builds the C# entry point as needed, then builds and installs Wayscriber through
the same `install app` command:

```bash
./tools/install.cs
```

Installer options can be passed directly, for example
`./tools/install.cs --replace-other`. Use `./tools/install.cs configurator` to
install only the configurator. The standalone `./tools/install.sh` remains
available when .NET is not installed. Use `./tools/install.cs help` to show the
C# installation commands; `--help` is reserved by `dotnet run` when the file is
launched through its shebang.

The regression suite also executes the retained package-repository and
release-packaging shell contracts, and a copy of `lint-and-test.sh` against fake
`dotnet` and `cargo` commands. Production C# commands do not invoke those shell
scripts.

`./tools/lint-and-test.sh` is the complete local gate and needs the .NET SDK pinned
by `global.json`; without it the gate stops before running anything. It builds the
C# apps, then runs the same steps as `ci lint-and-test`: the C# repository checks,
C# formatting, the C# regression suite (with the two retained shell contracts), then
the Cargo format, lint, build, and test steps.
Without .NET, `cargo test --workspace --all-features` still runs the Rust source
guards in `tests/repository_guards`, but it is not the complete gate.

C# equivalents of the shell tools include `dev build`, `dev test`, `dev fetch`,
`ci gtk-widgets`, `install app`, `install configurator`, `package build`, and
`aur update`. `ci lint-and-test` is what `tools/lint-and-test.sh` runs after building
the C# apps. `version bump`, `version check`, and the `release` tag commands have no
shell version. Run `--help` for the complete command list and options.

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

- **report code-health** - Report local maintainability metrics
  - Reports Rust files over 500 lines, functions over 120 lines, production unwrap/expect/panic/unsafe markers, selected allowances, and direct `fs::write` usage
  - Does not fail on reported findings; intended for baseline visibility before adding quality gates
  - Usage: `dotnet run tools/wayscriber.cs --no-build -- report code-health`

- **check rust-source-coverage** - Reject Rust sources outside the supported Cargo module graph
  - Uses current rustc dep-info from all-target/all-feature and no-default-feature checks
  - Runs as a hard gate in local and GitHub CI
  - Usage: `dotnet run tools/wayscriber.cs --no-build -- check rust-source-coverage`

- **tests/repository_guards** - Rust source guards that run in every `cargo test`
  - `config_writers.rs` rejects `config.toml` write capability outside the configurator's Save and the overlay's pinned narrow editors
  - `process_sites.rs` keeps process creation inside the process broker and audits the broker's post-fork child stub
  - `shared_dependencies.rs` keeps the shared domain and config validation layers free of upward crate paths, using the syntax corpus beside it
  - `no_python.rs` rejects Python files and links to them, Python project and lock files, Python shebangs, and Python interpreter or package names in the `.rs`, `.cs`, `.props`, `.targets`, `.sh`, `.nix`, `.toml`, YAML, `.service`, `.desktop`, and extensionless files it reads; other files are checked by name, and its known limits are listed at the top of the file
  - Each guard carries regression fixtures for the escapes it forbids
  - Usage: `cargo test --test repository_guards`

- **reload-daemon.sh** - Restart running daemon
  - Kills and restarts the daemon to pick up config/code changes
  - Usage: `./tools/reload-daemon.sh`

## Installation

- **install.sh** - Full installation script
  - Builds and installs binary to `/usr/bin` (or `$WAYSCRIBER_INSTALL_DIR`)
  - Refuses a second copy under `/usr/bin`, `/usr/local/bin`, or `~/.local/bin` unless
    `--replace-other` is passed or you confirm on a TTY
  - Sets up config directory with example config
  - Optionally configures systemd service or Hyprland autostart
  - Usage: `./tools/install.sh [--replace-other]`

- **install-configurator.sh** - Install configurator only
  - Builds and installs wayscriber-configurator, its desktop entry, and icons
  - Uses the matching `share` directory or `$WAYSCRIBER_DATA_DIR` for application data
  - Usage: `./tools/install-configurator.sh`

- **fetch-all-deps.sh** - Prefetch dependencies
  - Fetches all crates for offline/frozen builds
  - Usage: `./tools/fetch-all-deps.sh`

## Version & Release

- **version bump** - Bump version numbers
  - Checks offline dependency resolution before changing version files; run `dev fetch` (or `./tools/fetch-all-deps.sh`) if the cache is incomplete.
  - Updates Cargo.toml, configurator/Cargo.toml, the workspace Cargo.lock, PKGBUILD, and .SRCINFO
  - Updates only workspace packages in the lockfile, offline; existing dependency versions stay locked
  - flake.nix package version follows Cargo.toml automatically
  - Auto-increments patch version if no version specified
  - Use this in the same change as a user-visible overlay/settings/config toggle, or immediately
    before tagging that release, so `--version` is not identical to the last shipped crate
  - Supports MAJOR.MINOR.PATCH.HOTFIX for packaging-only hotfix releases
  - Rolls every version file back if the result fails `version check`
  - Usage: `dotnet run tools/wayscriber.cs --no-build -- version bump [--dry-run] [X.Y.Z[.N]]`

- **version check** - Check release metadata alignment
  - Verifies Cargo manifests, the workspace lockfile, packaging metadata, flake version sourcing, and that the flake compares the selected Rust toolchain to Cargo.toml rust-version
  - Keeps the configurator's libadwaita 1.4 floor aligned across Cargo, deb, rpm, PKGBUILD, and `.SRCINFO`
  - With `--release-version X.Y.Z[.N]`, rejects tags that do not match Cargo or an explicit packaging hotfix of Cargo
  - Usage: `dotnet run tools/wayscriber.cs --no-build -- version check [--release-version X.Y.Z[.N]]`

Packaging-only hotfix policy:
- Normal releases use one version everywhere: Cargo, package metadata, Git tags, and artifacts all use `X.Y.Z`.
- Hotfix releases may use `X.Y.Z.N` only when the Cargo version is still `X.Y.Z`. In that case, `packaging/PKGBUILD`, `packaging/.SRCINFO`, release artifacts, and AUR metadata use `X.Y.Z.N`; Cargo manifests and `flake.nix` stay on `X.Y.Z`.
- Repo `packaging/PKGBUILD` and `packaging/.SRCINFO` are templates and keep `sha256sums=('SKIP')` because the final GitHub tag archive checksum can only be computed after the tag exists. AUR automation writes the real checksum into external AUR metadata.
- Release builds set `WAYSCRIBER_RELEASE_VERSION`, so packaged binaries report the release artifact version. Nix builds follow Cargo and report `X.Y.Z` unless the Cargo version itself is bumped.

- **release create-tag** - Create git tag (local only)
  - Creates annotated tag `v<version>` without pushing
  - Requires clean working tree
  - Runs version consistency checks before tagging
  - Usage: `dotnet run tools/wayscriber.cs --no-build -- release create-tag X.Y.Z[.N]`

- **release publish-tag** - Create and push git tag
  - Creates annotated tag and pushes to origin
  - Auto-detects version from Cargo.toml if not specified
  - Runs version consistency checks before tagging
  - Usage: `dotnet run tools/wayscriber.cs --no-build -- release publish-tag [--version X.Y.Z[.N]] [--dry-run]`

See [Releasing](../docs/RELEASING.md) for validation, website release notes, and the
final update-manifest publication step. Pushing a tag does not update the website notice.

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
  - Live `https://wayscriber.com/arch-install.sh` is what CI fetches. Dual-prefix flags in a
    local website checkout are not checked until that file is published.
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

- **check nixpkgs-recipe** - Check the nixpkgs build declares what the default features need
  - Uses locked Cargo metadata
  - Maps every direct normal Cargo dependency, including target-specific dependencies, to the nixpkgs system packages it links
  - Keeps required native inputs, including the GTK application wrapper, aligned between the recipe and flake
  - Fails when a Linux default-feature dependency is missing from `packaging/nixpkgs/package.nix` or `flake.nix`
  - Fails on any new direct normal dependency until its system requirements are declared
  - Runs as a hard gate in GitHub CI and before release packaging
  - Usage: `dotnet run tools/wayscriber.cs --no-build -- check nixpkgs-recipe`

## AUR (Arch User Repository)

- **wayscriber assets emit** - Desktop asset recipe generator used by CI
  - Reads both package YAML manifests, validates asset paths, and requires mode 0644 as a plain YAML integer.

- **update-aur.sh** - Interactive AUR update
  - Updates PKGBUILD, tests build locally, pushes to AUR
  - Prompts for confirmation at each step
  - Usage: `./tools/update-aur.sh`

- **update-aur-from-manifest.sh** - CI-friendly AUR update
  - Remains a standalone shell implementation for local use and does not require .NET.
  - Regenerates managed desktop asset blocks, including incomplete blocks from older updater versions.
  - Updates multiple AUR packages using checksums from manifest.json
  - Adds and validates desktop launchers and icons for the main source/binary packages and configurator
  - Designed for CI automation after artifacts are built
  - Requires the configurator AUR clone unless `--no-configurator` is passed explicitly
  - Supports `--source-sha256` for offline/recovery runs
  - Previews and validates every selected recipe in an isolated copy before modifying checkouts, then pushes sequentially
  - Usage: `./tools/update-aur-from-manifest.sh --version <ver> --manifest dist/manifest.json --push`

---

## Notes

All scripts work from any location in the project.

### Potential Overlaps

The release tag commands have overlapping functionality:
- `release create-tag` + push = `release publish-tag`

Use the individual commands in sequence for full releases when you need explicit control over each step.
