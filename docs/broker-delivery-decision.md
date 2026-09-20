# M2-D broker delivery decision

Date: 2026-09-20. Status: implementation decision for M2; delivery and measurement gates remain open.

## Boundary and artifacts

Add private workspace package `broker/` (`wayscriber-process-broker`) with library `wayscriber_process_broker` and executable `wayscriber-broker`. The dependency graph is `wayscriber` → broker library ← broker executable; the broker package must never depend on the main `wayscriber` library. The new binary runs the existing server loop directly; the public `wayscriber` command no longer routes a broker child through `run_from_env()`. Keep a small forwarding module at the old internal path so existing callers do not change together with the transport move. The configurator still depends on the main crate with defaults disabled.

The package contains only `anyhow`, `libc`, `serde`, and `serde_json` as direct runtime dependencies. Extract the random ID/token, boot-clock/deadline, broker environment names, and exact trusted-update-URL policy from their current owners as dependency-light shared values. Keep the daemon, update client, GTK, rendering, and session modules outside the broker. Source-size and dependency checks must inspect both default and all-feature ELF maps, not infer isolation from this graph. `cargo build --locked --release --workspace --bins` builds the complete source cohort; a narrower `cargo build --release -p wayscriber --bin wayscriber` is incomplete and must fail clearly at runtime rather than silently using a broker found elsewhere.

Both binaries carry an internal broker protocol generation and a build-cohort identity derived from the broker wire/manifest implementation. After exec, the broker returns a bounded hello with those values before any helper request. The client rejects a mismatch and closes/waits for the child. Token authentication remains independent of compatibility. The cohort check is an accidental-mismatch guard; executable ownership and mode checks remain the trust boundary.

## Lookup and installation

For relocatable installs, put `wayscriber` and `wayscriber-broker` in one versioned executable directory; the public command is a symlink to that directory's `wayscriber`. Distro packages can install the pair together in `/usr/bin`, and Nix can keep both in one immutable output's `bin` directory. Resolve the companion beside the canonical running executable, validate its owner/mode and cohort, and open it **before** raw clone. Execute the opened descriptor to avoid a path replacement between validation and exec. Never choose a same-named binary from `PATH`. A missing or mismatched companion fails startup with an actionable message. The raw-clone child keeps only the prepared exec path, argv/envp, and declared descriptors.

The shell/C# installers, release tarball, Arch, deb/rpm, and Nix recipes must install the complete pair and update expected-file, dependency, and uninstall checks together. Service, shortcut, and configurator launch targets stay the public `wayscriber` or `wayscriber-configurator` command. The broker must never become an About, overlay, or configurator target.

## Upgrade state table

| State | Required behavior |
| --- | --- |
| Fresh complete cohort | Public command resolves its sibling and hello succeeds. |
| Missing, stale, or mismatched sibling | Fail closed before helper work; terminate and reap any attempted broker. |
| Duplicate names on `PATH` | Ignore them for companion resolution. |
| Old daemon and broker alive during file replacement | Existing owned broker stays paired; no new helper may be launched through a new cohort by fallback path. If the old executable is no longer available, activation fails with restart instructions. |
| New command after complete replacement | Starts only its complete new cohort. |
| Interrupted mixed replacement | No mismatched helper execution; restore complete old or new set. |
| Rollback | Restore both binaries and symlink as one unit, then manually restart the daemon. |
| Uninstall | Remove only installer-owned cohorts and links; stop/restart remains an operator action. |

The first transition from a pre-split daemon cannot guarantee seamless activation across replacement because the already-running old process lacks the new cohort check. Document a maintenance sequence: stop that daemon, install the complete pair, then start the new daemon. Do not auto-restart a user's service. Later upgrades may use the same sequence; a fail-closed old daemon is acceptable during an unplanned in-place replacement, but must be tested and diagnosed rather than silently launching new code. This trades an explicit restart requirement for a simpler, safe package transaction.

Before M2-I, freeze an install-layout matrix with tests for source build, local prefix, tarball relocation, Arch, deb/rpm, Nix, service/shortcut, and old/new live-process upgrade. Update the Python and C# process-site audits in the same code change. Do not claim M2 accepted until the whole-tree M0-I benefit exceeds a predeclared structural threshold and default/all-feature package checks pass.
