# V6 evidence review

Date: 2026-09-21.

| Item | Reviewed revision |
| --- | --- |
| App branch | `perf/memory-optimization` |
| App handoff | `2ca34484fea3e8f12b1cf50e400530863dd9289d` |
| Unchanged application source | `2cad646f` |
| Evidence repository | `3a07011f1c9148655249467804e028cfa0040bd5` |

## Decision

**One P2 finding affects the evidence checker and the repeated Built-in
acceptance claim.** No application-source change or new application defect was
identified. The new evidence does prove real resize and fractional-scale
transitions with held old buffers in the single-cycle Built-in and GTK runs.
The GTK repeated transition ordering also checks out.

The repeated Built-in runs do not establish that every transition started with
three held main-surface buffers. The first resize of cycle two, in both vsync
populations, selected two buffer IDs that had already been reused by the toolbar.
Correct this claim and the selector before calling that repeated stress complete.

The user's deferral of hardware, IME, tray, cohort, and performance checks is
accepted as the current scope decision. Those cells remain unproven. There is
no need to reopen them as part of this narrow correction. Keep the architecture,
three buffers, renderer default, and supported **12.3%** overall saving.

## F4 — P2: old-buffer selection survives ID reuse on another surface

**Locations:**
[current_buffers](../../../../docs/memory-optimization/v6_transition_probe.py),
lines 66–73, and `fill_slots`, lines 128–138. The affected claims are in the
[acceptance report](../../../../docs/memory-optimization/v6-acceptance-2026-09-21.md),
lines 58–63, row 69, and the Built-in vsync discussion at lines 83–87.

`current_buffers()` returns every distinct buffer ID attached to the main
surface since the last pool creation. It does not retire an entry when that
buffer is released/destroyed or when its ID is recreated for another surface.
`fill_slots()` intersects that historical list with the compositor's latest
`held` status for each numeric ID. A newly held toolbar buffer can therefore
count as a still-held old main buffer.

`new_frame_overlap()` only searches for old releases/destruction from
`before_transition` onward. It cannot catch a release, destruction, and ID reuse
that occurred **before** that boundary. The existing three correlation tests
cover reuse after the boundary, so all eight diagnostic tests pass despite this
case.

### Reproduced in the committed raw evidence

| Run | Affected transition | Selected IDs | Current held main buffer | Misclassified toolbar IDs |
| --- | --- | --- | --- | --- |
| `builtin-strict-repeat` | Index 3: first resize in cycle two | `24`, `39`, `51` | `51` | `24`, `39` |
| `builtin-vsync` | Index 3: first resize in cycle two | `37`, `41`, `54` | `54` | `37`, `41` |

For example, the
[strict repeat app trace](../../../../docs/memory-optimization/v6-evidence/builtin-strict-repeat/app.log.gz)
contains these events before transition index 3:

```text
[3944791.208][rs] <- wl_buffer@24.release, ()
[3944791.216][rs] -> wl_buffer@24.destroy()
[3944810.265][rs] -> wl_shm_pool@58.create_buffer(wl_buffer@24, 1531392, 1227, 104, 4908, 0)
[3944811.423][rs] -> wl_surface@41.attach(wl_buffer@24, 0, 0)
```

The main surface is `wl_surface@26`; `wl_surface@41` is the toolbar. Buffer 39
follows the same pattern. The report's later release/destruction checks succeed
for the new toolbar lifetimes, so those checks cannot establish the advertised
three-main-buffer precondition. The app still performs a real generation
transition with one old main buffer held; this observation is not an app failure.

A [small headless reproducer](v6-review-id-reuse-repro.py) demonstrates both the
incorrect selection and the overlap checker accepting it. It mocks the UI command
runner and launches no application or compositor. At the reviewed evidence head:

```sh
python3 -B docs/temp/memory-optimization/v6-review-id-reuse-repro.py
```

### Requested correction

1. Track each buffer's current creation lifetime, pool generation, surface, and
   commit/release/destruction state. Retire previous identity on destruction or
   recreation. Distinguish client connections where traces combine them; PID
   plus numeric object ID alone is not a complete ownership key.
2. Select three currently committed, unreleased main-surface buffer lifetimes
   immediately before the transition. Check the new frame's **commit** before
   those same lifetimes release. Keep the late-release and geometry checks.
3. Add a regression where an old main buffer is destroyed and its ID becomes a
   held toolbar buffer before `before_transition`. Also reject an attachment
   without its corresponding commit.
4. Reanalyze existing evidence and repeat only the affected Built-in repeated
   cases with vsync off and on in an authorized isolated session. Update the
   matrix/index. If those reruns are deferred, mark their precondition unproven
   and retain the valid single-cycle result.

No production rendering change is requested by this finding. A full Rust gate
is unnecessary for an evidence-only repair.

## What the saved evidence still proves

The independent review follows each selected ID's most recent creation,
main-surface attachment, new-frame commit, and later release/destruction. It
checked 31 transitions and 93 selected buffer lifetimes across seven saved
runs. Four selected lifetimes belong to toolbar buffers, as listed above.

| Saved run | Transitions satisfying the three-current-main-buffer precondition | Disposition |
| --- | ---: | --- |
| Built-in strict, one cycle | 3/3 | Held-buffer resize/scale ordering supported. |
| GTK strict, one cycle | 3/3 | Ordering supported. |
| Built-in strict, two cycles | 5/6 | Index 3 has one current main buffer held. Correct the repeated full-pressure claim. |
| GTK strict, two cycles | 6/6 | Ordering and recorded geometry checks supported. |
| Built-in vsync, two cycles | 5/6 | Same selection issue at index 3. |
| GTK vsync, two cycles | 6/6 | Ordering supported; retain the documented final geometry/settled-window exclusion. |
| Built-in 30-second hold | 1/1 | One held-buffer resize supported. |

Each successfully checked main buffer is released after the new main frame is
committed, then destroyed. The strict runs contain actual configure and preferred
scale events. Fixture delivery/cancellation totals reconcile with zero pending
releases at teardown. Both processes exit normally in these saved runs.

All 2,276 recorded PSS samples reconcile with their per-process counters. The
app/broker pair is present after readiness; each run's initial pre-readiness
sample contains just the starting app. The repeat cycle-window medians recompute
to 44,976 → 45,204 KiB for Built-in and 50,428 → 50,520 KiB for GTK. These numbers
remain valid observations, but the Built-in workload had a weaker pressure
precondition at one transition. They do not prove savings or long-term retention.

The capture-timeout trace records the dropped callback, a 1,004 ms timeout,
restoration, and a successful retry ready after 8 ms. The region-picker logs and
before/after images support Escape cancellation and overlay restoration. The
mixed-output logs support switching to scale 2 and back. The zoom logs and
inspected screenshots support the reported limited controls. Their documented
limits remain appropriate.

## Two scope clarifications

- The transition driver requests `GSK_RENDERER=cairo` for GTK sessions. Label
  that setup in the acceptance report. These GTK runs are not evidence for the
  unchanged default renderer's GPU/resource behavior; the logs do not independently
  identify the actual GSK renderer. No default-renderer retest is required to
  repair F4.
- The saved mixed-output screenshot visibly wraps/clips parts of the status HUD
  and places the zoom controls over its area on the narrow logical output. Its
  pass should mean output-switch/geometry behavior, not complete visual layout
  acceptance. This is an observed UI limitation in that image; no attribution to
  the memory changes or new live reproduction is claimed here.

## Validation and next step

Fresh checks in this review:

- Confirmed that `2ca34484` adds three documentation files and leaves application
  source unchanged from `2cad646f`.
- Matched the local release binary to the recorded
  `d637b854f1097faa133f468cf3f3ef007ce5df1ec12e5d51a70db068f6da9b1c` hash.
- Reran the five accounting and three transition tests: **8 passed**.
- Rebuilt the final C fixture with `-Wall -Wextra -Werror`; its hash exactly
  matches `741e9f1526bda9acbc8d4b3f04458d00f5532242efc05aa0403a9a902ae5b9fe`.
- Parsed the modified Python probes, rechecked protocol order and PSS sums,
  inspected the saved images, and reproduced F4 without a native session.
- Read-only service inspection showed the installed service active/running.

The [review data](v6-review-evidence.json) records per-run findings and input
hashes. The full Rust gate was not rerun because no application source changed.
No installed app, service, native session, original evidence file, or historical
plan was modified by this review. The review artifacts are local and uncommitted.

**Next implementer task:** repair F4 and its evidence claim, then stop this
acceptance iteration at the agreed scope. Further GTK lifetime/renderer profiling
remains an optional separate phase under [V6 section B](implementation-plan-v6.md).
No larger rewrite, renderer-default change, new overall savings claim, or release
action follows from this review.
