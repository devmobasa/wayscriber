# V6: finish acceptance, then decide whether to pursue more savings

Date: 2026-09-21. Starting app head: `2cad646f`.
Basis: [V5 review](review-v5.md). Measurement rules remain in the
[v2 acceptance guide](measurement-and-acceptance-v2.md).

## Recommended next assignment

Complete **A**, the remaining acceptance work. Keep the existing architecture,
Auto toolbar selection, renderer default, and three buffers. Implement a fix
only if a check demonstrates a defect. Do not reopen resolved JPEG, toolbar
search, installer, or Nix clipboard work.

**B** is an optional new optimization investigation. It is not a prerequisite
for keeping the measured 12.3% improvement. This document does not authorize a
desktop interruption, source installation, commit, or release.

## A1. Prove generation changes under delayed release

Extend the existing controlled fixture in an isolated session. It must cause
an actual layer-surface configure and/or preferred-scale event on the app's
surface. A nested window changing size without an app configure is an invalid
run. Reuse the existing fixture where possible; do not build a general-purpose
compositor unless the targeted test requires it.

Record a timeline with client/surface/buffer identity, generation, logical and
raster geometry, pool capacity, submit, release, and destruction. Scope GTK
toolbar resources separately from the main surface and other compositor clients.
Prefer fixture instrumentation; any app counters should be bounded and placed
at ownership transitions.

1. Fill all three main slots and hold their releases for a known interval.
2. Deliver a verified logical resize with old buffers still held. Repeat for a
   preferred-scale transition, including integer/fractional event ordering.
3. Show that new geometry renders, pending damage is repainted, and old storage
   is never reused. Deliver old releases after new frames and verify that they
   cannot clear a newer callback/throttle or invalidate the new pool.
4. Exercise redraw and capture during the transition, with Built-in and GTK
   toolbars. Test ordinary vsync and bounded uncapped redraw. Separately test
   cancellation or a hold past the capture deadline so restoration is proved.
5. Drain releases, wait for normal settling, and repeat a fixed bounded cycle
   sequence. Record app/broker/helper PSS, FD/thread/child trends, scoped peak
   where available, and compositor/GPU observations separately. Track retired
   generations; the current generation's three-slot counter is insufficient.

**Pass:** actual protocol/geometry transition recorded; old and new generations
overlap as intended; no stale pixels, lost redraw, invalid release, busy spin,
stuck capture, or growing retained resources after repeated settling. Expected
live buffers may remain while a surface is displayed; account for them instead
of requiring every live counter to be zero. Destroyed test clients must leave
no fixture timers or buffer resources outstanding.

**If it fails:** retain the failing sequence, make the smallest ownership or
dispatch repair, add a regression, and rerun the affected cells plus the source
gate. If it passes, close the cell and stop changing the buffer implementation.

The 6400×3600 transient generation is a measurement target in this same test.
Only optimize configure coalescing or pool creation if its resident peak or
latency is material. Preserve valid scale/configure semantics and release
ownership; do not infer a 263.7 MiB RAM saving from its capacity.

## A2. Close the applicable release matrix

Carry forward successful evidence with its exact artifact and environment.
The root flake already built and ran installed capture/clipboard at `2cad646f`.
The native source overlay already passed the interactions recorded in V5.

Remaining cells are simultaneous mixed outputs, tablet, IME composition, live
tray watcher actions, capture cancellation/failure, zoom variants, live old/new
cohort coexistence, and the v2 latency/CPU/retention budgets. Exercise the changed
path in each case; ordinary text, cold rollback, and individual capture-barrier
times do not substitute for the corresponding missing proof.

For unavailable hardware or unavailable measurement support, record **open**
with its reason and impact. A release owner may explicitly narrow the supported
release claim or defer a cell; the implementer must not silently mark it passed.
Native interaction belongs in an explicitly authorized desktop session after
the disposable setup and test steps are prepared.

Use one current matrix with links to evidence. Fix the minor even-sample median
calculation when next touching the probe, reusing the existing raw data. Preserve
historical reports; make the latest index clearly supersede stale open-status
wording. Avoid repeating successful tests solely to refresh an old document.

**Acceptance stopping point:** A1 passes; applicable release cells pass or have
an explicit recorded release-scope decision; any resulting fixes pass the source
gate. Report the achieved saving as 12.3%, with its original workload. Do not
claim that every original release criterion passed if some were deferred.

## B. Optional next memory investigation: GTK ownership

Choose this only if more active-overlay savings remain a product priority.
The native Cairo idle difference, about 104 MiB on the tested setup, is a reason
to investigate. It is not an established release gain or a predicted saving on
other hardware.

### B1. Establish lifetime before changing code

Keep the same identified app, GTK/driver versions, geometry, scene, toolbar, and
config. Preserve separate cold and copied-warm cache populations. Follow the
v2 five-pair alternating protocol and predeclare a worthwhile MiB benefit plus
latency/CPU/peak budgets.

Extend the time series through normal idle cache collection and after a real
popup, drawing, hide/show, capture, and close. Use the guide's 10-second settle,
30-second idle, and 60-second drawing starting durations; extend the observation
to cover the installed GTK's cache lifetime. The previous roughly four-second
idle windows are insufficient for long-idle retention. Do not change cache
timeouts while establishing this baseline.

Collect eligible input-to-submit latency populations and startup proxies; do
not aggregate overlapping rolling percentiles. Capture app/broker/helpers and
keep GPU/compositor bytes separate. Verify that the intended popup or capture
actually occurred during its measurement window. Measure after repeated use so
a first-frame saving cannot hide a higher peak or later retention.

Use mappings to select the dominant category, then obtain allocation or
resource-lifetime evidence for that category. GTK's
[profiling support](https://docs.gtk.org/gtk4/running.html#profiling) can provide
renderer/frame timing; allocator and driver evidence answer different ownership
questions. If a platform cannot provide reliable GPU bytes, keep that limitation
explicit rather than treating absent counters as zero.

### B2. Select at most one bounded prototype

Choose from the evidence:

- A specific GTK/GSK allocation persists beyond its useful lifetime: adjust
  that lifetime and preserve GTK capture/popup behavior.
- A renderer has consistently lower total resource cost with accepted active
  latency and compatibility: evaluate a narrowly scoped renderer policy before
  any default change. Verify supported drivers and fallback behavior. Use the
  existing diagnostic selector during experiments; a new config feature is not
  necessary to obtain measurements.
- No recoverable owner or acceptable renderer tradeoff is demonstrated: stop.
  Keep the existing policy and the already available Built-in choice.

Retain a candidate only after paired improvement above the declared noise and
benefit floor, acceptable active/first-use/warm behavior, and affected feature
parity. A rejected or inconclusive candidate is a valid completed investigation.
No recurring benchmark round is required without a specific unresolved question.

## C. Lower-priority fallback: portal image conversion

If portal freeze/zoom is a meaningful workload, force and verify that acquisition
route and measure compressed bytes, decoded pixels, converted pixels, crop
storage, and settled image lifetime. The current shared portal decoder allocates
a PNG output buffer and a second ARGB vector. Evaluate the existing owned
conversion helper only if that overlap exceeds a predeclared benefit floor.

Preserve color/alpha rounding, byte order, checked dimensions, stride, format
support, output provenance, cancellation, and scale/rotation/crop behavior.
Compare operation peaks and latency. The native grim capture peak is not evidence
of portal conversion savings. Stop if the measured improvement is too small.

## Handoff

Deliver one current acceptance matrix, raw evidence for newly closed cells,
identified artifacts, validation results, and an explicit disposition:
**acceptance complete**, **acceptance incomplete**, or **optional optimization
evaluated and retained/rejected/inconclusive**. Preserve the distinction between
those outcomes. Avoid another broad architecture plan unless a measured owner
cannot be addressed within the current design.
