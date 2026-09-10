# AGENTS.md

## Scope
- Applies to repository scripts under `tools/`.

## Architecture
- `wayscriber.cs` is the C# entry point used by CI and exposes parity commands for local development, installation, packaging, and releases.
- Keep its entry point small, command modules under `csharp/Commands/`, infrastructure under `csharp/Infrastructure/`, and module lists explicit in `csharp/includes.cs`.
- Build the C# file apps once, then invoke them with `dotnet run tools/<app>.cs --no-build -- ...`.
- Existing shell and Python tools remain standalone fallbacks for contributors without .NET. Production C# commands must not invoke them. The C# parity test app may execute the retained standalone release contracts so both implementations stay covered in CI.
- Scripts support build, install, lint/test, versioning, packaging, release tags, package repository generation, daemon reload, and dependency fetching.
- Scripts should resolve the repository root and work from any starting directory.
- The shell and Python fallbacks must remain usable without .NET and must not redirect to `wayscriber.cs`.
- Version-bump regressions live in `test-release-packaging.sh` and `wayscriber.tests.cs`.
- C# helper formatting follows `tools/.editorconfig`: spaced parentheses and braced guards, with LF endings required by `.gitattributes`.
- Separate logical steps in C# functions with a blank line. Keep closely related validation, setup, execution, state checks, and result mapping together, and add spacing when the purpose changes.
- Assign or deserialize a value first, then use a separate braced null guard. Do not embed `throw` in an assignment, return, or conditional expression.
- Name literals that encode a process exit, environment variable, command route, external executable, package channel, file mode, timeout, size limit, or shared repository path.
  Keep cross-cutting names in `csharp/Infrastructure/ToolConstants.cs` and command-specific values beside their owning command. Leave one-use diagnostics and syntax tokens inline.
- Review changed C# code visually after formatting; mechanical formatting alone does not verify clear logical grouping.
- Keep cyclomatic complexity at or below 20 per method. `CA1502` is an error, with the threshold in `CodeMetricsConfig.txt` wired through `Directory.Build.props`.

## Invariants
- Keep C# commands and standalone script behavior aligned. Add parity fixtures to `wayscriber.tests.cs` when either implementation changes.
- Invoke external programs from C# with `ProcessStartInfo.ArgumentList`; do not invoke a shell interpreter or assemble shell command strings.
- Preserve release/version/package semantics, including packaging-only hotfix behavior.
- Keep `tools/lint-and-test.sh` aligned with CI.
- The canonical gate serializes Rust test harnesses as a native-font race workaround and runs the ignored context-menu and board-picker render regressions separately under both feature configurations. Preserve their assertions and document the workaround separately from any native-library fix.
- Keep `check-rust-source-coverage.py` aligned with the workspace's all-feature and
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
