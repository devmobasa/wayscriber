# Board paper implementation and validation

Issue: [#389](https://github.com/devmobasa/wayscriber/issues/389).

Cartesian, isometric lines, and isometric dots share a procedural paper source.
All shipped board templates still default to None. Background opacity,
transparent-board behavior, and screenshot source provenance keep their existing
owners. This document records implementation decisions and headless evidence;
it does not claim a live Wayland or GTK interaction test.

## Final local checks

`./tools/lint-and-test.sh` passed on 2026-09-12, including packaging and source
audits, 75 C# tooling tests, formatting, builds, and strict workspace Clippy in
both feature configurations. The all-feature suite passed 4,582 root and 491
configurator tests; no-default-features passed 4,422 root and 490 configurator
tests. CLI, daemon fixture, UI integration, documentation, and the separately
invoked retained context-menu/board-picker render tests also passed.

The headless appearance sheet was inspected at 420×300 and its bounds tested
at 420×300 and 900×700. Poppler PDF raster comparisons ran on this machine.
No live Wayland overlay or GTK configurator was launched, and no GitHub Actions
run was triggered by the feature-branch pushes. The performance measurements
below are separate opt-in runs, not timings collected during the CI suite.

## State and recovery

Appearance lives with the board, outside drawing history. The picker owns an
identity-bound draft and applies a field-level patch once. The small appearance
sheet has the same control geometry for painting and mouse input. Invalid raw
spacing remains editable, and Cancel or an invalidated identity cannot commit.
Draft differences are computed from final values, so changing a field and then
reverting it preserves later external changes. Preview updates and dismissal
damage the sheet's screen rectangle without publishing a board or session edit.

Each `BoardState` owns its immutable configured/template appearance seed. Keeping
this seed on the identity-bearing state, instead of a second manager map, lets
ordinary duplicate/delete/restore operations carry it with the board. Session
replacement resets retained states from those seeds before applying saved paper.
Saved tools restore last, including on a restore to the already-active board.

Format 7 stores appearance with drawing data. Explicit paper edits also persist
on otherwise empty boards. Merely capturing an empty configured template cannot
claim recovery priority. History trimming drops non-explicit appearance when no
page data remains; explicit empty paper survives. Invalid appearance metadata
falls back to the seed without discarding valid drawings. Existing newer-format
preservation and clear-marker handling remain in use.

## Renderer selection

The full-viewport recording candidate was rejected before broad integration.
At 1920×1080, spacing 8 and 200 erasers, preliminary isometric-line medians were
about 405 ms for the viewport recording versus 26 ms for a compact vector tile.
Isometric dots similarly measured about 258 ms versus 34 ms. These preliminary
numbers came from the candidate comparison and are not the final raster timings.
The viewport recording walks visible grid geometry on each eraser replay, failing
the plan's adoption threshold (more than both 20% and 1 ms slower).

The selected source repeats a compact Cartesian or isometric tile. Raster
consumers generate a small raster tile once per paper paint/replay pass; PDF
uses an integer-unit recording tile and remains vector without erasers. Source
phase is reduced by whole tile periods at far-away coordinates. Erasers reuse
the exact source and phase used to paint the background. Blur still uses the
existing captured-image rules; procedural paper does not pretend to be a capture.

The raster tile caps density at 4×. At spacing 200 its maximum backing allocation
is 4,435,200 bytes (1386×800×4), independent of viewport and eraser count. The pan
cache retains its existing 256 MiB cap. Allocation counts were not instrumented;
reported surface byte counts are explicit proxies, not process peak RSS.

## Source benchmark

[Full measurements](board-paper-source-performance.csv), taken headlessly on an
AMD Ryzen 9 7950X, Cairo 1.18.4, Rust 1.98.1 test profile. One warmup followed by
five measurements; median is the middle sample and reported p95 is the maximum
of those five samples. No separate build or test was run concurrently by this
implementation during measurement. This is a local regression comparison, not
a hardware-independent frame-rate guarantee.

Both viewport sizes use scale 1 (logical size equals device size), a far negative
origin, 80 fixed annotation lines, alternating circular/rectangular erasers with
three points and size 32, and 0/20/200 committed erasers. Raster cases add one
provisional eraser; PDF cases do not. Timing includes target/source construction,
annotation replay, erasers, and PDF finalization where applicable.

| Pattern | Spacing | 1920×1080 median / p95 | 3840×2160 median / p95 |
|---|---:|---:|---:|
| Cartesian | 8 | 9.85 / 9.87 ms | 16.25 / 16.67 ms |
| Cartesian | 40 | 8.78 / 8.79 ms | 14.69 / 14.78 ms |
| Isometric lines | 8 | 18.27 / 20.11 ms | 30.58 / 30.62 ms |
| Isometric lines | 40 | 15.78 / 15.80 ms | 28.81 / 29.32 ms |
| Isometric dots | 8 | 18.15 / 18.24 ms | 30.07 / 30.14 ms |
| Isometric dots | 40 | 16.04 / 16.18 ms | 27.18 / 27.27 ms |

The table shows 200 committed erasers plus one provisional stroke. Ordinary
panning uses the existing committed-layer cache, so it does not pay this full
replay cost on every frame. None retains the existing solid paint path.

PDF files without erasers retain vector pattern resources (checked against
`/Subtype /Image`). Cairo's eraser compositing causes fallback raster work,
already present for plain solid backgrounds. Patterns increase fallback PDF
size and CPU cost; the CSV records both rather than presenting these pages as
wholly vector.

## Regression coverage

- Config defaults, enum/schema, spacing bounds, transparent normalization,
  template inheritance, and comment/unknown-field preservation on guarded saves.
- Negative and far-negative coordinates, fractional/2× scale, tile boundaries,
  Cairo path/state preservation, and circular/rectangular eraser replay.
- Direct versus baked pan rendering, pattern/spacing cache invalidation, PNG
  eraser pixels, immutable exports, and plain PDF margins with vector paper.
  Actual PDFs are also rasterized with Poppler when available to check paper
  phase and eraser restoration against the raster renderer.
- Draft Cancel/no-op/invalid input, conflict and identity checks, field-level
  merge, pen preservation on grid-only edits, and unchanged drawing history.
- Empty-board persistence, exact optional pen restoration, legacy appearance
  seeds, named replacement, and backup recovery/history-trimming distinctions.

## Integrated pan and PNG consumers

[Consumer measurements](board-paper-consumer-performance.csv) use the actual
`CanvasLayerCache::ensure`/`blit` and PNG export entry points. The matrix includes
both zero and far-negative origins, both sizes, None plus all three patterns,
spacing 8/40, and 0/20/200 committed erasers. Warmup/sample counts, scale, shapes,
and brushes match the source benchmark. PNG includes encoding. A warm pan sample
checks cache validity and blits into an already allocated destination; it does
not construct the paper again.

| Consumer (patterned boards) | 1920×1080 p95 range | 3840×2160 p95 range |
|---|---:|---:|
| Cold pan-cache bake | 3.60–42.47 ms | 9.10–39.03 ms |
| Warm pan-cache check and blit | 0.19–1.73 ms | 1.94–3.34 ms |
| PNG render and encoding | 34.73–305.44 ms | 138.62–660.61 ms |

Ranges include all tested origins, spacings, patterns, and eraser counts, so
an occasional slower local sample is retained rather than removed. Live pan
reuse stays a blit. Export time is worker-side work, not event-loop rendering.
The simultaneous surface allocation proxy is approximately 30.6 MiB at 1080p
and 107.6 MiB at 4K for the benchmark's retained bake, blit destination, and PNG
target; it excludes codec scratch and the separately bounded paper tile.

To reproduce the opt-in measurements:

```sh
cargo test -p wayscriber --all-features --lib board_grid_backdrop_performance -- --ignored --nocapture --test-threads=1
cargo test -p wayscriber --all-features --lib board_grid_consumers_performance -- --ignored --nocapture --test-threads=1
```

Set `WAYSCRIBER_GRID_COMPARE_VIEWPORT=1` on the first command to include the
rejected viewport-recording candidate. These benchmarks are opt-in so ordinary
CI does not spend time measuring machine-dependent rendering latency.
