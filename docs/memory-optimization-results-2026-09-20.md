# Memory optimization results — 2026-09-20

This record covers the measured changes from the memory-optimization v2 plan. Whole application-tree proportional set size (PSS) includes the application and its process broker. The objectives of 20–30 MiB idle PSS and 20–30% lower active PSS remain hypotheses, not achieved results.

## Bounded async runtimes

Commits `6ee235e9` and `96e78010` bound daemon listener runtimes to their dedicated current-thread drivers, give the active capture runtime one explicit worker, and add opt-in full-run latency/slot measurements. The complete repository validation script passed after each code commit.

Five matched standalone drawing pairs compared an instrumentation-only baseline built from `53d122a5` with `96e78010`. Both were default-feature release builds. Each fresh run used a synthetic empty session, a nested Wayland compositor, 10 seconds of settle, then 100 pen strokes of 120 points at 5 ms intervals. The application and broker were sampled at 1 Hz. Peak process-tree threads fell from **53 to 22** in every pair. Median paired PSS saving was **0.15 MiB** (range **−1.05 to +1.97 MiB**), below the predeclared 1 MiB worthwhile threshold. Both artifacts had 8.5 ms p95 input-handler-to-submit upper bounds, 8.6–8.7 ms p99 upper bounds, 11,900 eligible samples per run, and no pending or dropped samples. CPU time was within 0.03 seconds per paired run. This supports the bounded worker policy and shows no measured drawing regression, but does not establish an active resident-memory saving.

An isolated `--freeze` smoke test completed the capture suppression frame, used compositor screencopy, restored the overlay, and displayed the frozen state. It does not cover every capture/export or cancellation path.

## Existing buffer-count option

Five fresh-process pairs compared `performance.buffer_count = 3` and `2` with the same `96e78010` release binary and the fixed 120 FPS drawing workload. Median paired saving from two buffers was **−0.89 MiB** (range **−2.68 to +0.51 MiB**), below the predeclared +2 MiB threshold. All ten runs touched one painted slot, reached one peak in-flight slot, and had no deferral episodes. Input-to-commit p95/p99 stayed within the declared tolerances. One additional pair each with vsync on and uncapped rendering also touched only one slot and had no deferrals. The compositor released buffers promptly in this fixture, so delayed-release ownership remains untested. **The default remains three buffers.**

These measurements used an integer-scale nested output and synthetic input. They do not establish presentation latency, compositor/GPU memory, larger or fractional-output behavior, repeated close/reopen retention, or the full active scenario matrix. The private test session and disposable data did not alter the existing user service or drawings. Raw per-process samples, logs, screenshots, and exact artifact hashes are retained outside the repository.
