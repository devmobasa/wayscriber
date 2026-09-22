# Held-buffer integration check

Run `bash tools/integration/held-buffers/test-held-buffers.sh` after building the app and
broker. The [dedicated CI workflow](../../../.github/workflows/held-buffers.yml)
builds with no optional desktop features and runs this check on Ubuntu 24.04.
The test uses a disposable Sway headless backend, runtime directory, and app
config/data. It does not connect to the user's desktop or service.

The C fixture delays `wl_buffer.release` and reverses the deadlines for the
three selected main-surface buffers. The Python trace tracks creation offsets,
pool generations, main-surface commits, releases, and destruction so reused
protocol IDs and toolbar buffers cannot be counted as held main slots. The
driver requires all three slots to be occupied, then changes the actual output
from 1280×720 to 1600×900 and to fractional scale 200/120. It checks new pool
creation and a new main frame commit before any selected old release, followed
by server delivery, client destruction, reordered releases, and another drawn
frame. Every wait has a deadline; compositor shutdown must leave no pending
release. Four trace-parser unit tests cover ID reuse and false overlap.

This checks protocol buffer and server-resource lifetime. It does not measure
RSS/PSS or directly sample the pixel storage of a held server buffer. On
failure, the driver prints the isolated `/tmp/wb*` log directory and leaves it
for inspection.

The virtual-pointer protocol XML is copied from `wayland-protocols-wlr 0.3.12`
and retains its upstream copyright and permission notice. The fixture and
driver are derived from the V6 acceptance probes, reduced to this regression.
