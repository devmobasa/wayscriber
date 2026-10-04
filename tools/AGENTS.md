# AGENTS.md

## Scope
- Applies to repository scripts under `tools/`.

## Architecture
- `wayscriber.cs` is the C# entry point used by CI and exposes parity commands for local development, installation, packaging, and releases.
- Keep its entry point small, command modules under `csharp/Commands/`, infrastructure under `csharp/Infrastructure/`, and module lists explicit in `csharp/includes.cs`.
- Build the C# file apps once, then invoke them with `dotnet run tools/<app>.cs --no-build -- ...`.
- Each repository check has one implementation. Source ownership invariants (shared dependencies, process sites, config writers) are Rust tests in `tests/repository_guards` and run in every `cargo test`. Release, packaging, build-metadata, and source-coverage checks are C# commands. Do not add Python (`tests/repository_guards/no_python.rs` enforces this), and do not add another copy of a check.
- The remaining shell tools stay usable without .NET. Production C# commands must not invoke them. The C# test app may execute the retained shell contracts (`test-package-repo-layout.sh`, `test-release-packaging.sh`) and a copy of `lint-and-test.sh` against fake `dotnet` and `cargo` commands.
- Scripts support build, install, lint/test, packaging, package repository generation, daemon reload, and dependency fetching. Versioning and release tags are C# commands only (`version bump`, `version check`, `release create-tag`, `release publish-tag`).
- Scripts should resolve the repository root and work from any starting directory.
- Shell tools must not redirect to `wayscriber.cs`, except `lint-and-test.sh`: it is the complete gate and runs the C# checks, so it requires .NET.
- Version check, bump, and release-tag regressions live in the C# tests (`csharp-tests/VersionConsistencyTests.cs`, `VersionReleaseCommandTests.cs`).
- C# helper formatting follows `tools/.editorconfig`: spaced parentheses and braced guards, with LF endings required by `.gitattributes`.
- Separate logical steps in C# functions with a blank line. Keep closely related validation, setup, execution, state checks, and result mapping together, and add spacing when the purpose changes.
- Assign or deserialize a value first, then use a separate braced null guard. Do not embed `throw` in an assignment, return, or conditional expression.
- Name literals that encode a process exit, environment variable, command route, external executable, package channel, file mode, timeout, size limit, or shared repository path.
  Keep cross-cutting names in `csharp/Infrastructure/ToolConstants.cs` and command-specific values beside their owning command. Leave one-use diagnostics and syntax tokens inline.
- Review changed C# code visually after formatting; mechanical formatting alone does not verify clear logical grouping.
- Keep cyclomatic complexity at or below 20 per method. `CA1502` is an error, with the threshold in `CodeMetricsConfig.txt` wired through `Directory.Build.props`.

## Invariants
- Keep C# commands and any retained shell script behavior aligned. Add fixtures to `wayscriber.tests.cs` when either changes.
- Invoke external programs from C# with `ProcessStartInfo.ArgumentList`; do not invoke a shell interpreter or assemble shell command strings.
- Preserve release/version/package semantics, including packaging-only hotfix behavior.
- Keep `tools/lint-and-test.sh` aligned with CI.
- The canonical gate serializes Rust test harnesses as a native-font race workaround and runs the ignored context-menu and board-picker render regressions separately under both feature configurations. Preserve their assertions and document the workaround separately from any native-library fix.
- Keep `check rust-source-coverage` aligned with the workspace's all-feature and
  no-default-feature target matrix; intentional exceptions must be narrow and documented.
- Avoid platform-specific assumptions unless the script is explicitly platform-specific.

## Coupled Changes
- Version and packaging scripts must stay aligned with `tools/README.md`, `packaging/`, `.github/`, `Cargo.toml`, and release docs.
- Install/reload scripts may affect setup docs and daemon service behavior.
- `install.sh` and the website `arch-install.sh` must refuse a second unmanaged prefix
  (`/usr` vs `/usr/local`) unless the operator opts into replacing the other copy.

## Validation
- Run changed scripts directly when safe.
- Run `./tools/lint-and-test.sh` for changes to lint/test behavior.
- Use `git diff --check` for docs/script-only edits.
