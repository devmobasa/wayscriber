# Native font concurrency diagnostic

This standalone C program exercises Pango/Cairo with fresh per-thread layouts,
surfaces and contexts, overlapping font-map teardown with other threads' draws.
It uses more than ten distinct font files to exercise native font cache eviction.
It does not initialize GTK, connect to Wayland, or start Wayscriber.

Prerequisites: a C compiler, pthreads, pkg-config, Cairo, Pango/PangoFT2,
Fontconfig development files, fontconfig tools, GNU timeout, and at least eleven
installed scalable font files. Fonts and their order affect reproduction; the
harness records both selected descriptions and resolved files. A fontconfig
package version is not a complete font-set description.

From any working directory, run:

```sh
/path/to/wayscriber/tools/diagnostics/native-fonts/run.sh /tmp/native-font-check
```

Optional arguments after the results directory are threads, waves, iterations
per thread, and maximum font files. Defaults are `24 20 96 64`, bounded to 45
seconds. Extra iterations vary by thread to overlap teardown. Results retain
the executable, versions, complete font inventory, draw log and exit status.
Status 0 means this workload completed, 124 means timeout, 139 typically means
SIGSEGV, and 2 means a missing font/argument prerequisite. Preserve the executable
and matching system libraries alongside any core before debugging it. On
systemd machines, `coredumpctl info` and `coredumpctl debug` can inspect the core.

## Evidence on 2026-09-07

Recovered the original 2026-09-05 harness, previously kept only in temporary
files, without changing its workload. A fresh `-O1 -g` build with GCC reproduced
SIGSEGV in the first wave at `24 20 96 64`, before the 45-second limit:

- Cairo 1.18.4, Pango 1.58.2, FreeType 2.14.3 (header version), Fontconfig 2.18.3.
  FreeType's pkg-config/libtool version was 26.6.20; it is not the release version.
- 64 distinct source font files, including Noto families and locally installed
  Maple Mono. Resolved files were logged before the crash.
- The current crashing stack starts inside `libcairo.so.2`, through
  `cairo_scaled_font_glyph_extents`, `cairo_scaled_font_text_extents`, PangoCairo,
  HarfBuzz shaping, and Pango layout. The core reports signal 11, SEGV_MAPERR.
- No Wayscriber code or shared application drawing objects are in this harness.

This reproduces a current native-library failure. Earlier cores implicated
Cairo font-cache eviction and a temporary locking patch was explored, but this
run does not establish that mechanism or validate that patch. No library or
application fix is claimed. A passing rerun or serial run would not prove the
race fixed. Keep native diagnosis separate from Rust regression results and
from compositor verification.
