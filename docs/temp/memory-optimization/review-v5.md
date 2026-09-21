# V5 implementation and evidence review

Date: 2026-09-21.

| Item | Reviewed revision |
| --- | --- |
| App branch | `perf/memory-optimization` |
| App head | `2cad646f44ac5e7ebf4ad74d4ae936a7857d8eb3` |
| Incremental code review | `67f16b5f..2cad646f` |
| Evidence repository | `9b9568beddc6f955f17551df59b3d7e256d67f23` |

## Decision

No new blocking application-code issue was found in the three latest commits.
The [V4 installer finding F3](review-v4.md) is closed. Keep the existing
architecture, three Wayland buffers, Auto toolbar selection, and renderer policy.
The selected memory implementation can stop expanding in scope.

Acceptance is still incomplete. The most valuable next check is **resize or
preferred-scale change while buffers from the old geometry remain held**.
Complete the other applicable release cells listed below before describing the
original acceptance plan as passed. Missing device coverage must stay explicitly
open or receive a documented release-scope decision.

More memory savings are plausible. The strongest measured lead is GTK renderer
resource use; a whole-app rewrite has no supporting evidence. The
[V6 assignment](implementation-plan-v6.md) separates acceptance from that optional
investigation and gives both a stopping point.

The app and evidence trees were clean at review start. This review changed only
new files in this ignored planning directory. It did not run native UI checks,
install software, alter the host service, stage files, or publish a release.

## Code assessment

| Commit | Assessment |
| --- | --- |
| `0edb30d8` | Shell and C# probes recognize an unavailable user bus while retaining refusal for unexpected inspection failures and transitional service states. Service-free installation is supported again. Locale-controlled parsing, disposable shell fixtures, C# discovery tests, and rollback tests cover the repaired boundary. |
| `a8167bd2` | Both Nix recipes include `coreutils` with the capture/clipboard helpers in the service and app-wrapper PATH. This supplies `cat` used by the clipboard helper. The broker remains a separate ELF companion; admission and cohort checks were not weakened. |
| `2cad646f` | Both event-loop timeout selection and render admission check slot availability. A pending redraw remains pending when all slots are occupied. Release processing makes the slot available again, and dispatch returns to the render pass. Geometry changes can create a new pool without reusing old submitted storage. |

The buffer change was traced through
[timeout selection](../../../src/backend/wayland/backend/event_loop/mod.rs),
[render admission](../../../src/backend/wayland/backend/event_loop/render.rs),
[dispatch](../../../src/backend/wayland/backend/helpers.rs), and
[pool/slot ownership](../../../src/backend/wayland/surface.rs).
The pinned SCTK 0.20.0 implementation decrements its active-buffer count when
processing the release event. Runtime, capture, and interaction deadlines still
participate in timeout selection; separate toolbar rendering remains reachable
when the main surface cannot acquire a slot.

`active_slot_count()` describes the current generation. A three-slot peak alone
does not bound all outstanding generations or GTK allocations. Pool destruction
also does not prove that compositor backing storage has disappeared: buffers
retain the pool's storage under the
[Wayland protocol](https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_shm_pool).
This is why the remaining lifetime test is useful even though source ownership
and the existing fixed-geometry checks look sound.

## Evidence assessment

### Installed and native coverage has materially improved

The latest [Nix follow-up](../../../../docs/memory-optimization/v5-buffer-pressure-follow-up-2026-09-21.md)
records a root-flake build at **`2cad646f`**, followed by an installed wrapped GTK
overlay, paired broker, full-screen capture, clipboard publication, and overlay
restoration in an isolated nested session. This supersedes the earlier
socket-less attempt and the earlier clipboard failure. Sandboxing was disabled
inside the rootless build container. The separate nixpkgs submission recipe is
still an unbuilt draft; the root-flake result does not certify it.

The [native report](../../../../docs/memory-optimization/v5-native-acceptance-2026-09-21.md)
identifies the source release artifact at the same app head. It records drawing,
toolbar hide/show, a real GTK popover, ordinary text entry, capture,
freeze/unfreeze, one-output scale changes, configurator Save followed by a new
Built-in overlay, and PNG/PDF export. This reviewer inspected the reports and
saved measurements; those visual checks were not repeated during this review.

Independent recalculation of the native trace confirms 80 complete capture
samples, 29 helper observations, a sampled peak of **288,654 KiB**, and the
preceding toolbar-shown median of **182,284 KiB**. The capture sampling interval
was at most about 28.6 ms. This is a transient workload observation, not an
old/new memory comparison. The mislabeled popup sample and premature Cairo
capture sampling remain excluded, as the report requires.

### Controlled release proves a useful, limited case

The [release fixture](../../../../docs/memory-optimization/v5-delayed-release-fixture-2026-09-21.md)
delays `wl_buffer.release` independently of other compositor events. Its source
was inspected and rebuilt with warnings treated as errors. The retained source
hash matches the report. Resource-destruction listeners cancel pending timers;
the two-second run ends with 231 held, 116 delivered, 115 cancelled, and zero
outstanding releases across the nested compositor's clients.

The saved Built-in and GTK event records support capture starting while all
three main slots were held. Their barriers became ready after approximately
1,763 and 1,792 ms; capture completed and suppression was restored. The PNG hash
matches the report. These results support recovery under fixed geometry.

The attempted nested resize did **not** deliver a new configure to the app's
layer surface. Its unchanged pool generation makes it an invalid exercise of
old-generation retirement. Exact GTK buffer ownership is also unmeasured. Do
not generalize the fixture's global counters to an app-only or GTK-only count.

### Renderer measurements are reproducible diagnostics

This review checked all 40 native runs and all 400 samples: renderer identity,
unchanged geometry, complete counters, app/broker membership, and the sum of
per-process PSS. Five alternating pairs exist for each comparison in each cache
population. The app hash matches the native report. Recalculated paired results:

| Vulkan minus challenger, native idle | Cold median | Cold range | Warm median | Warm range |
| --- | ---: | ---: | ---: | ---: |
| OpenGL | 8.14 MiB | 5.45–8.86 | 73.61 MiB | 71.44–74.01 |
| Cairo | 104.19 MiB | 102.82–106.11 | 103.59 MiB | 102.42–104.78 |

The [recalculation](v5-review-renderer-reanalysis.json) retains pair-level values.
These are short native idle populations on one driver/output setup. They do not
prove another saving in the original default workload. Active drawing latency,
CPU cost, GPU/compositor attribution, and allocation ownership are still missing.
The OpenGL cold/warm difference remains a cache-associated observation without a
proven causal owner.

**Minor measurement correction:**
[the probe](../../../../docs/memory-optimization/v5_renderer_probe.py), line 119,
uses the upper middle observation for ten samples. A conventional median averages
the two middle observations. Only cold run 0 changes, by **1 KiB**; the cold
OpenGL paired median becomes 8.1357421875 MiB instead of 8.13671875 MiB. All table
values rounded to two decimals and the review decision stay unchanged. Use
`statistics.median` when next editing the probe; no experiment rerun is needed
for this correction.

There is also a useful next measurement: these runs settle for three seconds
and sample for roughly one second. Current
[GTK documentation](https://docs.gtk.org/gtk4/running.html#gsk-cache-timeout)
describes a default 15-second GPU-renderer cache collection timeout. The current
local GTK development version is 4.22.4; record the actual runtime version for
each future artifact. The short windows do not establish long-idle retention.
Measure through the normal cache lifetime and after interaction before deciding
whether a resource can be released earlier. The documentation is a reason to
measure, not evidence that this particular PSS difference will disappear.

## Remaining acceptance, with scope kept explicit

| Area | Current state / next proof |
| --- | --- |
| Source installer without user bus; rollback | F3 closed. Disposable parity and rollback regressions pass. Earlier installed upgrade/rollback evidence remains applicable. |
| Root Nix build and installed capture/clipboard | Passed in the documented container/nested environment at current head. Sandbox-enabled build and nixpkgs submission are separate claims. |
| Main-buffer delayed release, redraw and capture | Narrow fixed-geometry case passed. No repeated optimization is needed here without a new failure. |
| Resize/scale with old buffers held | **First remaining targeted check.** Prove actual configure/scale delivery, a new generation, correct late releases, and settled resource use. |
| GTK buffer/resource lifetime | Toolbar interaction under delayed release passed; exact GTK ownership and lifetime attribution remain open. |
| Native broader matrix | Simultaneous mixed outputs, physical tablet, IME composition, tray watcher actions, capture cancellation/failure cleanup, and zoom variants remain open. Ordinary text is not IME proof. |
| Live upgrade coexistence | Old overlay/new cohort overlap remains open; a cold start/rollback does not establish it. |
| Performance acceptance | Matched drawing latency, startup proxy, CPU/wakeups, repeated-cycle retention, and applicable scenario budgets still need an explicit disposition under the v2 guide. Individual barrier times do not establish p95/p99. |

The historical [V5 follow-up](../../../../docs/memory-optimization/v5-follow-up-2026-09-21.md)
and `README-v5.md` contain older open-status wording. Use the later native,
buffer-pressure, and controlled-release reports for the cells they supersede.
Do not repeat completed checks merely because an older checklist is stale.

## Further gains: ranked opportunities

1. **GTK renderer/resource lifetime.** This has the strongest measured lead.
   First establish longer-lived and active-stage costs, then locate an owner
   with allocation/lifetime evidence. A small lifetime or renderer-policy
   prototype may follow. Keep the default until compatibility, latency, CPU,
   peak memory, and driver coverage support changing it. The existing Built-in
   toolbar is already a user choice for its separately measured benefits.
2. **Transient scale generations.** The native transition created a temporary
   6400×3600 three-buffer pool: 276,480,000 bytes, about 263.7 MiB of capacity.
   This is not measured RAM. Add generation and residency attribution to the
   required held-buffer transition test. Only a measured peak or stall warrants
   a geometry/event-ordering change. Preserve legitimate configure handling and
   never recycle storage still owned by the compositor.
3. **Portal freeze/zoom decode overlap.**
   [The shared portal decoder](../../../src/capture/sources/frozen.rs) currently
   holds decoded PNG bytes while building another ARGB vector; both portal
   freeze and zoom call it. This is a concrete, bounded candidate if a forced
   portal workload shows meaningful overlap. Evaluate reuse of
   [the existing conversion code](../../../src/image_decode.rs), with pixel,
   alpha, stride, format, and cancellation parity. Native grim capture does not
   measure this path, and capacity alone does not quantify its PSS benefit.

A new renderer engine, wholesale toolkit rewrite, broker removal, arbitrary
cache limits, and another two-buffer default experiment are not justified by
the current evidence. Preserve the successful changes. The original **12.3%**
same-workload result remains the supported overall figure; optional toolbar,
scratch, capture, and renderer measurements must not be added to it. The
20–30% aspiration is not a requirement to keep rewriting until a number is met.

## Reviewer validation

Fresh, headless checks in this review:

- `cargo test --locked -p wayscriber --all-features --lib backend::wayland::backend::event_loop:: -- --test-threads=1`: **40 passed**.
- `cargo test --locked -p wayscriber --all-features --lib backend::wayland::surface::tests -- --test-threads=1`: **4 passed**.
- C# test-app build and `RepositoryContractTests`: **39 passed**, including the
  disposable shell service-state and rollback fixtures.
- Evidence process-accounting unit tests: **5 passed**.
- Nix recipe checker and shell syntax checks passed.
- Controlled-release C fixture compiled with `-Wall -Wextra -Werror`.
- Native renderer and capture counter recalculations passed as described above.

The saved full `./tools/lint-and-test.sh` log was inspected; the implementer's
reported full gate was not rerun for this documentation-only review. These
checks do not replace live desktop, package, or release acceptance.
