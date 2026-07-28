# Configuration Guide

## Overview

wayscriber supports customization through a TOML configuration file located at:
```
~/.config/wayscriber/config.toml
```

All settings are optional. If the configuration file doesn't exist or settings are missing, sensible defaults will be used.

### Configured defaults and runtime UI preferences

`config.toml` is the authored source for configured defaults. Some direct overlay customizations are
saved separately so moving through the UI does not rewrite unrelated configuration:

- top/side toolbar pin and minimized state;
- the top strip's display form reached with the cycle keybinding or the micro chip;
- the top toolbar's dragged position;
- the side toolbar's dragged position (a side drag also records the top strip's reconciled
  horizontal offset, because moving the palette can change where the strip rests);
- the active side pane and collapsed side sections;
- individual toolbar item visibility and toolbar item order; and
- per-board pin state.

The generated file is `$XDG_DATA_HOME/wayscriber/runtime-ui.toml`, normally
`~/.local/share/wayscriber/runtime-ui.toml`. It is not a second configuration file and should not
be hand-edited. The configurator labels affected controls as configured defaults. On startup, and
after a same-process config/session reload, Wayscriber treats those configured values as seeds and
applies any retained runtime overrides on top. If a configured seed changes to match an override,
the redundant override is removed.

The current runtime-state format is version 1. Unknown fields in a supported version are preserved
across writes, including unknown fields attached to a retained override. A newer version is loaded
read-only and is never downgraded automatically. Resetting a newer or invalid file requires an
explicit confirmation; the exact source bytes are moved to a recovery artifact before the reset,
and the Settings panel shows the artifact's complete path.

Runtime-state writes are conditional on the exact inspected source and parent-directory identity.
Wayscriber does not overwrite an externally changed file, follow or replace the final path when it
is a symlink, or continue through a retargeted parent. If another writer wins, its freshly inspected
state becomes authoritative. Every accepted runtime preference change settles as persisted,
superseded by a reset, won by the external source, changed after a claimed write, or failed.

When persistence is uncertain, the Settings panel blocks further runtime preference mutations and
offers incident-scoped actions: retry the pending save, discard pending changes and use the current
disk state, or preserve an invalid file and reset after confirmation. Recovery can be cancelled
while it is read-only; if a write has already started, Wayscriber waits for its real completion
before reinspection. Diagnostic and recovery-artifact paths are shown without truncating their
contents.

If runtime-state inspection or its writer cannot start at all, the Settings panel reports
persistence as unavailable instead of offering recovery actions that cannot run. Runtime-only
toolbar and board changes remain process-only in that mode and leave the authored configuration
unchanged; a toolbar dragged or a display mode cycled in that mode applies for the current run and
returns to its configured default on the next start.

#### The configurator is the only writer

`config.toml` is an authored input. Pressing **Save** in the graphical configurator is the only
thing in Wayscriber that writes it. The overlay, the daemon, the tray, startup, validation, and
shutdown read and interpret the file and never create, replace, rewrite, touch, chmod, or back it
up — not when a shortcut is invalid, not when two shortcuts collide, not when `config_revision` is
old, not when a value is out of range, and not when the file is missing or read-only. Starting,
using, and quitting Wayscriber leaves the file's bytes, size, mode, owner, and modification time
exactly as you left them.

The practical consequence: an overlay control that changes a configured default applies to the
current run only, and says so. Restart and the configured value comes back. Where a durable change
is wanted, the control offers a route into the configurator at the matching screen — press
<kbd>F11</kbd>, or use the toast's action button — and the change becomes durable when you Save
there.

When the configurator edits an existing file, it preserves TOML comments, section order, compatible
value formatting, and unrecognized settings. Unrecognized paths produce a configurator warning but
remain in the file for forward compatibility. Known values are still validated for the running
session, and aliases are written under their canonical names. The configurator tracks the exact
loaded contents rather than relying on modification time; if the file is created, deleted,
retargeted through a symlink, or changed by another editor, reload it before saving. A save does
not expand omitted, unchanged defaults; when a setting that was omitted is edited, only that
changed setting and its required table path are added.
A save writes only what you changed: a value that loading clamped, normalized, deduplicated, or
reset keeps the text you authored, so editing one preference can never rewrite settings you did not
touch. Validation results are not exempt from that rule and never reach the file at all. A pending
revision migration reaches it only when you review and apply the proposal in the configurator and
then Save — see [`[keybindings]`](#keybindings---custom-keybindings).
The first save for a missing file is sparse as well: it writes the revision marker and only values
changed from the built-in defaults.
One deliberate consequence: a value the file holds out of range is clamped for the running session
but keeps its authored text on disk, and re-entering the clamped value in the configurator is a
zero-delta save that writes nothing — the file keeps the out-of-range text until you set the field
to some other value or edit it by hand.
Every section follows the same rule for unrecognized keys, including `[export]`, `[export.pdf]`,
and `[export.pdf.labels]`: a typo there is reported and kept, never dropped from the file, and it
does not stop the rest of the configuration from loading.

#### Backups

A configurator Save copies the previous contents to a timestamped `config.toml.<timestamp>.bak`
next to `config.toml` before writing. That copy is the recovery path: to undo a save, copy the
newest `.bak` back over `config.toml`. Nothing prunes them, so delete the ones you no longer want.

There is no longer any other backup. Earlier releases kept a rolling copy under
`$XDG_STATE_HOME/wayscriber/config-backups/` to protect writes made by the running overlay and the
tray; those writers are gone, so nothing creates, reads, or prunes that directory any more. If you
have one from an older release it is left untouched as your own recovery data — the files are
ordinary TOML copies, and you can keep or delete the directory as you like.

#### Who writes what, when

Three stores and three mechanisms cover every preference Wayscriber saves for you:

- **Configurator Save** — the only write to `config.toml`, and only when you press Save. It leaves a
  timestamped `.bak` beside the file.
- **Runtime-UI writer** — a guarded, conditional write to `runtime-ui.toml`; see the list at the top
  of this section for what lives there and why.
- **Session autosave** — the session snapshot, not a configuration file at all.

| You do this | It is saved to | By |
| --- | --- | --- |
| Drag the top or side toolbar | `runtime-ui.toml` | Runtime-UI writer |
| Cycle the top strip full ⇄ micro (<kbd>F2</kbd> or the micro chip) | `runtime-ui.toml` | Runtime-UI writer |
| Pin, unpin, or minimize a toolbar | `runtime-ui.toml` | Runtime-UI writer |
| Switch the side pane or collapse a side section | `runtime-ui.toml` | Runtime-UI writer |
| Hide, show, or reorder an individual toolbar item | `runtime-ui.toml` | Runtime-UI writer |
| Pin a board | `runtime-ui.toml` | Runtime-UI writer |
| Switch layout mode (Simple/Full) in the overlay | Nothing — this run only | Configurator → UI → Toolbar for the default |
| Toggle a toolbar section from Settings | Nothing — this run only | Configurator → UI → Toolbar Visibility for the default |
| Switch icons ⇄ text labels | Nothing — this run only | Configurator → UI → Toolbar for the default |
| Toggle the status bar, its interactivity, or one of its items; the board/page badges, floating badge, or zoom chip | Nothing — this run only | Configurator → UI → Status Bar for the default |
| Toggle click highlight or the highlight-tool ring | Nothing — this run only | Configurator → UI → Click Highlight for the default |
| Toggle the input HUD | Nothing — this run only | Configurator → UI → Input HUD for the default |
| Toggle the Step section, delay sliders, tool preview, preset toasts, extra colors, or context-aware UI | Nothing — this run only | Configurator → History or UI → Toolbar for the default |
| Save or clear a preset slot in the overlay | Nothing — this run only | Configurator → Presets to keep it |
| Recolor a quick color swatch in the overlay | Nothing — this run only | Configurator → Drawing to keep it |
| Rename, recolor, add, or delete a board | The session file, for boards marked `persist` | Configurator → Boards for the templates a new session starts from |
| Change a shortcut | Not editable in the overlay | Configurator → Keybindings |
| Toggle session resume from the tray menu | Not editable from the tray | Tray → "Session persistence settings…" opens Configurator → Session |
| Press Save in the graphical configurator | `config.toml` | Configurator Save (with a timestamped `.bak`) |
| Change pen color, thickness, tool, or font size | Session file | Session autosave (needs `restore_tool_state`) |

Drawings, boards, pages, and per-page pan offsets belong to the session file (see `[session]`).
Everything else — zoom, freeze, presenter mode, light mode, and any configured default you changed
from the overlay — is live state for the run and is not saved at all.

If the graphical configurator can read the file but cannot parse its TOML or known value types, it
opens a clearly marked repair draft using built-in defaults. Saving that draft first creates a
backup of the unreadable source, retains unknown keys that can be separated safely when the TOML
structure itself was parseable, and replaces the unreadable known configuration. A transient reload
error leaves the last good document and unsaved draft in place; its revision guard still prevents
overwriting a changed file.

## Configuration File Location

The configuration file should be placed at:
- Linux: `~/.config/wayscriber/config.toml`
- The directory will be created automatically when you first create the config file. If the config
  path is a dangling symlink, missing parent directories for its final target are created as well.

## Example Configuration

See `config.example.toml` in the repository root for a complete example with documentation.

## Configuration Sections

### `[drawing]` - Drawing Defaults

Controls the default appearance of annotations.

```toml
[drawing]
# Default pen color
# Options: "red", "green", "blue", "yellow", "orange", "pink", "white", "black"
# (named colors resolve to the tuned quick color palette, e.g. "red" = #F5333F)
# Or #RRGGBB hex: "#FFB3BA" (or #RRGGBBAA for alpha: "#FFB3BA80")
# Or RGB array: [255, 0, 0] (or RGBA: [255, 0, 0, 128])
default_color = "red"

# Default pen thickness in pixels (1.0 - 50.0)
default_thickness = 3.0

# Default eraser size in pixels (1.0 - 50.0)
default_eraser_size = 12.0

# Default eraser mode ("brush" or "stroke")
default_eraser_mode = "brush"

# How the blur tool obscures its region by default
# "gaussian"  - softens detail (historical behavior)
# "pixelate"  - coarse mosaic of averaged blocks; block size follows the tool size
# "secure"    - collapses the region to one averaged color; no detail survives
# "black-out" - opaque black fill; needs no captured background
default_blur_style = "gaussian"

# Default marker opacity multiplier (0.05 - 0.90). Multiplies the current color alpha.
marker_opacity = 0.32

# Default fill state for fill-capable shape tools
default_fill_enabled = false

# Default side count for the Regular Polygon tool (3 - 12)
polygon_sides = 5

# Default font size for text mode (8.0 - 72.0)
# Can be adjusted at runtime with <kbd>Ctrl+Shift++</kbd>/<kbd>Ctrl+Shift+-</kbd> or <kbd>Shift</kbd> + scroll
default_font_size = 32.0

# Font rendering defaults
font_family = "Sans"
font_weight = "bold"
font_style = "normal"
text_background_enabled = false

# Hit-test tuning + undo retention
hit_test_tolerance = 6.0
hit_test_linear_threshold = 400
undo_stack_limit = 100

# Drag gesture tool mapping
# Flat drag fields accept only drag-bindable tools. Freeform polygon is
# selectable from the toolbar picker but is not valid here.
drag_tool = "pen"
shift_drag_tool = "line"
ctrl_drag_tool = "rect"
ctrl_shift_drag_tool = "arrow"
tab_drag_tool = "ellipse"

# Ordered quick colors used by shortcuts, toolbar swatches, and radial menu.
# The first eight entries map to R/G/B/Y/O/P/W/K; if fewer are configured by
# hand, missing shortcut positions use built-in defaults and help-overlay badges
# follow those shortcut-backed entries. Extra entries have no shortcut action
# binding. Explicit extra entries appear in toolbar/radial palette UIs, capped
# to the first 24 rendered colors. Use known color names, #RRGGBB hex (or
# #RRGGBBAA to carry alpha), or RGB/RGBA arrays. The hex values below are the
# tuned built-in defaults; named colors ("red", "green", ...) resolve to these
# same tuned values, so named entries, the default pen color, and board
# auto-adjust pens all match these swatches.
#
# Right-clicking a swatch in the overlay opens the color picker for that slot
# and applies the accepted color to the current run, keeping the slot's label
# and shortcut; this list is unchanged. Edit the durable palette in the
# configurator's Drawing screen. That picker's "Default" button loads the color
# shipped for the slot again (built-in slots only), still requiring OK.
[[drawing.quick_colors]]
label = "Red"
color = "#F5333F"

[[drawing.quick_colors]]
label = "Green"
color = "#2EC27E"

[[drawing.quick_colors]]
label = "Blue"
color = "#3584E4"

[[drawing.quick_colors]]
label = "Yellow"
color = "#F6D32D"

[[drawing.quick_colors]]
label = "Orange"
color = "#FF7800"

[[drawing.quick_colors]]
label = "Pink"
color = "#C061CB"

[[drawing.quick_colors]]
label = "White"
color = "#FFFFFF"

[[drawing.quick_colors]]
label = "Black"
color = "#241F31"

[[drawing.quick_colors]]
label = "Cyan"
color = "#00FFFF"

[[drawing.quick_colors]]
label = "Purple"
color = "#9966CC"

[[drawing.quick_colors]]
label = "Gray"
color = "#666666"

# Example custom entry:
# [[drawing.quick_colors]]
# label = "Blush"
# color = "#FFB3BA"

# Optional per-button override. Right/middle keep their built-in behavior
# unless configured. Use "default" for a button's built-in behavior.
[drawing.drag_tools.left]
drag_tool = "pen"
shift_drag_tool = "pen"
shift_drag_color = "red"

[drawing.drag_tools.right]
drag_tool = "pen"
drag_color = "blue"

[drawing.drag_tools.middle]
drag_tool = "default"
```

**Color Options:**
- **Named colors**: `"red"` (`#F5333F`), `"green"` (`#2EC27E`), `"blue"` (`#3584E4`), `"yellow"` (`#F6D32D`), `"orange"` (`#FF7800`), `"pink"` (`#C061CB`), `"white"` (`#FFFFFF`), `"black"` (`#241F31`) — named colors resolve to the tuned quick color palette
- **Hex strings**: `"#RRGGBB"` such as `"#FFB3BA"`, or `"#RRGGBBAA"` such as `"#FFB3BA80"` to carry alpha. Other hex-like strings such as `"#GG0000"` or `"#12345"` keep config-load compatibility but fall back to red with a warning; the configurator rejects them for quick color fields.
- **RGB arrays**: `[255, 0, 0]` for red, `[0, 255, 0]` for green, etc. A fourth component sets alpha: `[255, 0, 0, 128]`.
- **Alpha**: colors are opaque unless an alpha component says otherwise, and opaque colors are written back in the three-component form they have always used — so adding alpha never rewrites an existing palette. The marker and highlighter multiply their own opacity on top of any color alpha rather than replacing it.

**Quick Colors:**
- `[[drawing.quick_colors]]` entries define an ordered palette.
- The first eight entries are selected by <kbd>R</kbd>/<kbd>G</kbd>/<kbd>B</kbd>/<kbd>Y</kbd>/<kbd>O</kbd>/<kbd>P</kbd>/<kbd>W</kbd>/<kbd>K</kbd>; missing first-eight entries fall back to built-in defaults.
- The built-in defaults use the tuned hex palette shown above (`#F5333F`, `#2EC27E`, `#3584E4`, `#F6D32D`, `#FF7800`, `#C061CB`, `#FFFFFF`, `#241F31`). Named colors resolve to the same tuned values, so `default_color = "red"`, named quick color entries, and board auto-adjust pen colors all select the matching swatch.
- The implicit default toolbar palette also preserves Cyan, Purple, and Gray as expanded toolbar colors, while the radial menu keeps the original first-eight color ring.
- Extra entries have no quick-color action binding; explicit extra entries appear in toolbar and radial palette UIs, capped to the first 24 colors.
- Help overlay badges are shown for the first eight shortcut-backed entries only.
- The screen eyedropper is available with <kbd>I</kbd>, from the toolbar color section, from the color picker popup, and from the command palette. Rebind `keybindings.colors.pick_screen_color` if you prefer another shortcut. It samples the captured desktop currently visible through Wayscriber; on a transparent board it can briefly use screen freeze when no captured image exists.

**Runtime Adjustments:**
- **Pen thickness**: Use <kbd>+</kbd>/<kbd>-</kbd> keys or scroll wheel (range: 1-50px)
- **Eraser size**: Use <kbd>+</kbd>/<kbd>-</kbd> keys or scroll wheel when eraser tool is active (range: 1-50px)
- **Eraser mode**: Use <kbd>Ctrl+Shift+E</kbd> to toggle brush vs stroke erasing
- **Blur style**: Run **Cycle Blur Style** from the command palette to step through blur → pixelate → secure → black out (unbound by default; bind `cycle_blur_style`)
- **Marker opacity**: Use <kbd>Ctrl+Alt</kbd> + <kbd>↑</kbd>/<kbd>↓</kbd>
- **Regular polygon sides**: Use the side toolbar Sides control (range: 3-12)
- **Font size**: Use <kbd>Ctrl+Shift++</kbd>/<kbd>Ctrl+Shift+-</kbd> or <kbd>Shift</kbd> + scroll (range: 8-72px)

**Defaults:**
- Color: Red
- Thickness: 3.0px
- Eraser size: 12.0px
- Eraser mode: Brush
- Marker opacity: 0.32
- Fill enabled: false
- Polygon sides: 5
- Font size: 32.0px
- Font family/weight/style: Sans / bold / normal
- Text background: false
- Hit-test tolerance: 6.0px (linear threshold: 400)
- Undo stack limit: 100
- Drag mapping: Drag=Pen, Shift+Drag=Line, Ctrl+Drag=Rect, Ctrl+Shift+Drag=Arrow, Tab+Drag=Ellipse

### `[arrow]` - Arrow Geometry

Controls the appearance of arrow annotations.

```toml
[arrow]
# Minimum arrowhead length in pixels. The head also scales with stroke width
# (three times the thickness), so this acts as the floor for thin strokes.
length = 20.0

# Arrowhead half-angle in degrees (15-60). Smaller is a sharper, narrower head.
angle_degrees = 24.0

# Place the arrowhead at the end of the line instead of the start
head_at_end = false
```

**Defaults:**
- Length: 20.0px
- Angle: 30.0°
- Head at end: false (head at the start)

### `[presets]` - Quick Tool Slots

Configure 3-5 tool presets that you can apply via hotkeys or the toolbar strip.

Saving or clearing a slot from the overlay changes it for the current run only — the toast says so
and offers an **Edit** action that opens the configurator's Presets screen, where a slot can be
changed durably.

```toml
# Spotlight tool: dims the whole overlay except the regions you draw, so
# attention lands where you point. Select the tool from the toolbar or bind
# `select_spotlight_tool`.
[spotlight]
# How strongly the area outside every spotlight is dimmed (0.1 - 0.95)
dim_opacity = 0.6

# Fraction of each spotlight radius spent fading out at the edge (0.0 - 0.9).
# 0.0 gives a hard-edged opening.
feather = 0.35

[presets]
slot_count = 5

[presets.slot_1]
name = "Red pen"
tool = "pen"
color = "red"
size = 3.0
marker_opacity = 0.32
fill_enabled = false
font_size = 32.0
text_background_enabled = false
arrow_length = 20.0
arrow_angle = 30.0
arrow_head_at_end = true
show_status_bar = true

# Optional full per-tool profile captured by newly saved presets.
[presets.slot_1.tool_settings]
eraser_size = 18.0

[presets.slot_1.tool_settings.pen]
color = "red"
size = 3.0

[presets.slot_1.tool_settings.line]
color = "green"
size = 6.0

[presets.slot_1.tool_settings.rect]
color = "blue"
size = 4.0

[presets.slot_1.tool_settings.ellipse]
color = "orange"
size = 4.0

[presets.slot_1.tool_settings.arrow]
color = "yellow"
size = 5.0

[presets.slot_1.tool_settings.blur]
color = "black"
size = 12.0

[presets.slot_1.tool_settings.marker]
color = "yellow"
size = 20.0

[presets.slot_1.tool_settings.step_marker]
color = "white"
size = 28.0
```

**Required fields:** `tool`, `color`, `size`  
**Optional fields:** `tool_settings`, `eraser_kind`, `eraser_mode`, `marker_opacity`, `fill_enabled`, `font_size`, `text_background_enabled`, `arrow_length`, `arrow_angle`, `arrow_head_at_end`, `polygon_sides`, `show_status_bar`, `drag_tools`

When `tool_settings` is present, applying the preset restores the full drawing profile for all
tools, including StepMarker size and Eraser size, then activates `tool`. Legacy presets without
`tool_settings` keep the old behavior and apply only `color`/`size` to the selected `tool`.
The top-level `color` and `size` are retained for compatibility, readability, and toolbar previews.

### `[history]` - Undo/Redo Playback

Controls delayed undo/redo playback and the optional Step section in the toolbar.

```toml
[history]
# Delay between steps for undo-all/redo-all (50 - 5000 ms)
undo_all_delay_ms = 1000
redo_all_delay_ms = 1000

# Show the Step section in the toolbar
custom_section_enabled = false

# Delay between steps for custom undo/redo (50 - 5000 ms)
custom_undo_delay_ms = 1000
custom_redo_delay_ms = 1000

# Number of steps to run in custom undo/redo (1 - 500)
custom_undo_steps = 5
custom_redo_steps = 5
```

**Notes:**
- `undo_all_delay_ms` / `redo_all_delay_ms` drive the "Undo all (delay)" and "Redo all (delay)" toolbar actions.
- `custom_section_enabled` reveals the Step buttons in the side toolbar; those buttons use the custom delays and step counts above.

### `[performance]` - Performance Tuning

Controls rendering performance and smoothness.

```toml
[performance]
# Number of buffers for rendering (2, 3, or 4)
# 2 = double buffering (low memory)
# 3 = triple buffering (recommended, smooth)
# 4 = quad buffering (ultra-smooth on high refresh displays)
buffer_count = 3

# Enable vsync frame synchronization
# false lowers drawing latency; true prevents tearing and limits rendering to display refresh rate
enable_vsync = false

# Max FPS when VSync is disabled (0 = unlimited)
# 120 keeps pen latency low without uncapped CPU usage; set to 0 only for profiling
max_fps_no_vsync = 120

# UI animation frame rate (0-240; 0 = unlimited)
# Higher values smooth UI effects at the cost of more redraws
ui_animation_fps = 30
```

**Buffer Count:**
- **2**: Double buffering - minimal memory usage, may flicker on fast drawing
- **3**: Triple buffering - recommended default, smooth drawing
- **4**: Quad buffering - for high-refresh displays (144Hz+), ultra-smooth

**VSync:**
- **false** (default): Capped by `max_fps_no_vsync`; lower drawing latency, with possible tearing
- **true**: Synchronizes with display refresh rate, no tearing, but input-to-commit latency is bounded by refresh cadence

**Max FPS (VSync off):**
- **120** (default): Low-latency drawing without uncapped redraw loops
- **60**: Lower CPU/GPU use, but latency may feel closer to one 60 Hz frame interval
- **144/165/240+**: Use when it matches your display and the machine handles the extra rendering work
- **0**: Unlimited; mostly for profiling because it can spin CPU/GPU hard

**UI Animation FPS:**
- **30** (default): Smooth enough for most effects
- **0**: Unlimited (renders every frame while animations are active)
- Values through **240** improve smoothness at the cost of extra redraws; larger values are clamped

**Defaults:**
- Buffer count: 3 (triple buffering)
- VSync: false
- Max FPS (VSync off): 120
- UI animation FPS: 30

**Tradeoff:**
Disabling vsync improves input latency but may allow tearing and higher CPU/GPU usage. On weaker
PCs, laptops, or battery-sensitive setups, restore `enable_vsync = true` or lower
`max_fps_no_vsync` if you notice heat, fan noise, battery drain, or compositor smoothness issues.

**Measurement note:**
With `WAYSCRIBER_PERF_LOG=1`, the `perf.input_to_paint_latency proxy=input_to_wayland_commit`
line reports an input-to-Wayland-commit proxy metric. It measures from input sample receipt inside
the app to Wayland surface commit. It is not photons-on-screen display latency; compositor
scheduling, display scanout, and hardware can add more latency outside Wayscriber.

In local continuous-drawing measurements, 120 FPS low-latency mode held p95 around 8-9 ms and
p99 around 8-9 ms for this proxy metric. Isolated max spikes existed, but p99 stayed under 16 ms.

### `[tray]` - System Tray

Controls the main system tray icon. Changes take effect after restarting the daemon.

```toml
[tray]
# Options: "auto", "symbolic", "colored"
icon_style = "auto"
```

- `auto` (default) uses a theme-adaptive symbolic icon on supported desktops and colored fallback pixmaps on known-incompatible tray hosts, including Omarchy/Quickshell, Noctalia/Quickshell, and COSMIC.
- `symbolic` always requests the theme-adaptive icon. The tray host chooses its visible color.
- `colored` always publishes the yellow, scale-aware fallback pixmaps.

`WAYSCRIBER_TRAY_FORCE_PIXMAP=1` takes precedence over this setting and also disables named menu icons for compatibility with tray hosts that render them incorrectly.

The tray menu reads the configuration to draw itself and changes none of it. Its **Session
persistence settings…** entry launches the configurator on the Session screen, where the change is
made and saved.

### `[updates]` - Update Notifications

Wayscriber never installs updates. This section controls only whether it *tells you* that a newer release exists and points at the instructions for your install method.

```toml
[updates]
check = true         # ask wayscriber.com whether a newer release exists
notify = true        # one desktop notification per release
interval_hours = 24  # minimum 1, maximum 720
```

- `check` (default `true`) lets the daemon fetch `https://wayscriber.com/latest.json` once per interval and compare its version to this build. The request carries no Wayscriber or user identifier, no Wayscriber version, and no query parameters; the HTTP client's version is suppressed too. It goes out through whichever of `curl` or `wget` is installed, so it uses the system CA store and proxy settings, with client config files and Wget's `.netrc` credential lookup disabled. One check is one request — an installed client that fails is not retried with the other one — and the response is cut off past 64 KiB.
- `notify` (default `true`) shows at most one desktop notification per release, suppressed while the overlay is active. With it off, the notice still appears in the About window and the tray menu.
- `interval_hours` (default `24`) is clamped to 1–720 hours.

The result is cached in `$XDG_CACHE_HOME/wayscriber/update-check.json` (normally `~/.cache/wayscriber/update-check.json`); deleting it just makes the next check look like the first one. The cache records the last attempt and the last *success* separately: attempts drive the interval, successes drive the "checked N ago" line, and a newer failed attempt is reported with it, so a failed check neither makes a stale result look verified nor hides itself behind a true-but-older age. Failed explicit checks (`--check-update`, About's "Check now") also count toward the interval, since the request was already made. If the cache cannot be written, the interval is still enforced in memory for the life of the process.

If the config file exists but cannot be parsed, the background check does not run: Wayscriber cannot confirm this section, so it assumes the stricter setting until the file is valid again.

Ways to switch it off, strongest first:

1. Build with `WAYSCRIBER_NO_UPDATE_CHECK=1` — the check is compiled out and nothing at runtime can re-enable it (for distributions that forbid outbound version checks).
2. Export `WAYSCRIBER_DISABLE_UPDATE_CHECK=1` — overrides `check` for that run. The documented falsey words (`0`, `false`, `no`, `off`, `disable`, `disabled`, empty) leave the check on; any other value opts out. `wayscriber --check-update` still works, since asking for a check is consent.
3. Set `check = false` here.

`wayscriber --check-update` prints the installed version, the newest release, and the update instructions URL without installing anything.

### `[ui]` - User Interface

Controls visual indicators, overlays, and UI styling.

```toml
[ui]
# Overlay chrome theme
# Options: "auto", "dark", "light" ("auto" currently resolves to dark)
theme = "auto"

# Reduce UI motion (disable animations)
# Options: "auto", "on", "off"
reduced_motion = "auto"

# Show the status bar and its configured contents
show_status_bar = true

# Allow clicking status bar segments to open their related controls;
# set false for a display-only status bar whose clicks pass through
status_bar_interactive = true

# Status-bar contents. Each item can be hidden independently. Visible items
# keep this fixed order; narrow layouts may compact labels and temporarily
# shed items without changing these choices. Mode badges such as
# FROZEN/ZOOM/PAN are separate.
# The active output appears only when an output label is available.
active_output_badge = true

# Selection dimensions appear only while one or more shapes are selected.
show_status_selection_info = true

# Show board label in the status bar
show_status_board_badge = true

# Show page counter in the status bar
show_status_page_badge = true

# Show the current color dot
show_status_color = true

# Show the active tool name
show_status_tool = true

# Show the active tool size as a separate segment
show_status_size = true

# Show transient text/highlight context indicators when applicable
show_status_context_indicators = true

# Show a clickable status-bar hint chip (e.g. "F9 Toolbar") while every
# toolbar surface is hidden; set false if you run toolbar-less on purpose
show_toolbar_hint = true

# Show the Help shortcut segment
show_status_help = true

# Show the About/version segment
show_status_about = true

# Master visibility for the floating board/page badge. The
# toggle_floating_badge palette/keyboard action flips it for the current run
# only; this value is the default it starts from.
show_floating_badge = true

# Also show the floating board/page badge when the status bar is visible
show_floating_badge_always = false

# Show a small "FROZEN" badge when frozen mode is active
show_frozen_badge = false

# Filter help overlay sections based on enabled features
help_overlay_context_filter = true

# Command palette action toast duration (ms)
command_palette_toast_duration_ms = 1500

# Status bar position
# Options: "top-left", "top-right", "bottom-left", "bottom-right"
status_bar_position = "bottom-left"

# Preferred output name for GNOME fallback (xdg-shell) overlays
#preferred_output = "eDP-1"

# Enable output-cycling shortcuts on layer-shell compositors
multi_monitor_enabled = true

# Request fullscreen for the GNOME fallback overlay (disable if opaque)
#xdg_fullscreen = false

# Behavior when GNOME fallback (xdg-shell) loses keyboard focus
# Options: "exit", "stay" (default on Ubuntu/GNOME)
#xdg_focus_loss_behavior = "exit"

# Mouse button that toggles radial menu
# Options: "middle", "right", "disabled"
radial_menu_mouse_binding = "middle"

# Status bar styling
[ui.status_bar_style]
font_size = 21.0
padding = 15.0
bg_color = [0.0, 0.0, 0.0, 0.85]     # Semi-transparent black [R, G, B, A]
text_color = [1.0, 1.0, 1.0, 1.0]    # White
dot_radius = 6.0

# Help overlay styling
[ui.help_overlay_style]
font_size = 14.0
font_family = "Noto Sans, DejaVu Sans, Liberation Sans, Sans"
line_height = 22.0
padding = 32.0
bg_color = [0.09, 0.1, 0.13, 0.92]   # Deep slate background
border_color = [0.33, 0.39, 0.52, 0.88] # Muted steel border
border_width = 2.0
text_color = [0.95, 0.96, 0.98, 1.0] # Near-white

# Click highlight styling (visual feedback for mouse clicks)
[ui.click_highlight]
enabled = false
show_on_highlight_tool = false
radius = 24.0
outline_thickness = 4.0
duration_ms = 750
fill_color = [1.0, 0.8, 0.0, 0.35]
outline_color = [1.0, 0.6, 0.0, 0.9]
use_pen_color = true  # Existing highlights update immediately when you change pen color
force_in_light_mode = true  # Force-enable click highlights when entering light mode

# Input HUD (on-screen keystrokes and clicks)
[ui.input_hud]
enabled = false
mode = "auto"                 # auto | overlay | system
position = "bottom-center"    # top-left | top-center | top-right |
                              # center-left | center | center-right |
                              # bottom-left | bottom-center | bottom-right
show_mouse = true
show_bare_modifiers = true
display_ms = 1600
fade_ms = 350
max_entries = 6
combine_repeats = true
font_size = 18.0

# Context menu visibility
[ui.context_menu]
enabled = true
```

**Status Bar:**
- Shows current color, pen thickness, and active tool
- Press <kbd>F1</kbd>/<kbd>F10</kbd> to toggle help overlay
- Fully customizable styling (fonts, colors, sizes)

**Position Options:**
- `"top-left"`: Upper left corner
- `"top-right"`: Upper right corner
- `"bottom-left"`: Lower left corner (default)
- `"bottom-right"`: Lower right corner

**Theme & Motion:**
- **Theme**: `theme` selects the overlay chrome theme — `"auto"` (default), `"dark"`, or `"light"`. `"auto"` currently resolves to dark chrome; `"light"` takes effect progressively as overlay surfaces adopt the runtime theme (until then it also renders dark).
- **Reduced motion**: `reduced_motion = "on"` disables overlay chrome animations (toast and flash fades render instantly; coverage extends to more surfaces as they adopt the shared animation envelopes). `"off"` keeps full motion. `"auto"` (default) is reserved for a future desktop-portal query of the system reduce-motion preference and currently behaves like `"off"` (full motion).

**UI Styling:**
- **Font sizes**: Customize text size for status bar and help overlay
- **Colors**: All RGBA values (0.0-1.0 range) with transparency control
- **Layout**: Padding, line height, dot size, border width all configurable
- **Click highlight**: Enable presenter-style click halos with adjustable radius, colors, and duration; by default the halo follows your current pen color (set `use_pen_color = false` to keep a fixed color)
- **Input HUD**: `ui.input_hud` shows a live row of keystroke/click chips for demos and screencasts (see `[ui.input_hud]` below)
- **Highlight tool ring**: `show_on_highlight_tool = true` keeps a persistent halo visible while the highlight tool is active
- **Light mode**: `force_in_light_mode = true` preserves the default behavior of enabling click highlights on light mode entry; set it to `false` to keep the current click highlight state
- **Context menu**: `ui.context_menu.enabled` toggles right-click / keyboard menus
- **Output focus**: `multi_monitor_enabled` controls output-cycling shortcuts; `active_output_badge` shows the current monitor in the status bar
- **GNOME fallback**: `preferred_output` pins the xdg-shell overlay to a specific monitor; `xdg_fullscreen` requests fullscreen instead of maximized; `xdg_focus_loss_behavior` controls whether losing focus closes (`exit`) or keeps (`stay`) the overlay
- **Radial menu trigger**: `radial_menu_mouse_binding` selects which mouse button opens radial menu (`middle` default, `right`, or `disabled`)

**Multi-monitor behavior:**
- Use `focus_prev_output` / `focus_next_output` (default: <kbd>Ctrl+Alt+Shift+←</kbd>/<kbd>Ctrl+Alt+Shift+→</kbd>) to move overlay focus between outputs.
- Toolbar surfaces and status bar follow the active output when focus changes.
- Output switching is blocked while capture, frozen, or zoom is active/in progress; finish or exit those modes first.
- Command palette (`Ctrl+K` or `Ctrl+Shift+P`) includes hidden aliases, so searching `monitor` or `display` finds output actions.
- For GNOME/xdg fallback, set `preferred_output` (or env override `WAYSCRIBER_XDG_OUTPUT`) to pin the overlay to a specific monitor.

**Defaults:**
- Theme: auto (currently dark)
- Reduced motion: auto (full motion)
- Show status bar: true
- Interactive status bar segments: true
- All status bar content items: true
- Show frozen badge: false
- Position: bottom-left
- Radial menu mouse trigger: middle
- Status bar font: 21px
- Help overlay font: 14px
- Semi-transparent dark backgrounds with muted borders

### `[ui.input_hud]` - Input HUD (keystrokes and clicks)

A live row of keycap-style chips showing what you press, for demos and
screencasts. Toggle it with `toggle_input_hud` (default
<kbd>Ctrl+Shift+K</kbd>) or from the command palette; the Settings popover has
an **Input HUD** checkbox. The runtime toggle applies to the current run only;
`ui.input_hud.enabled` is the default it starts from, edited in the
configurator. A config file that already bound <kbd>Ctrl+Shift+K</kbd> to
another action keeps it and starts the HUD unbound — see the
[`[keybindings]` notes](#keybindings---custom-keybindings) — so pick a free
shortcut for `toggle_input_hud` if you want a key for it.

Chips appear on the right and push older chips left. Key chords use the same
names the keybinding config and help overlay print (`Ctrl+Shift+Z`, `Space`,
`Esc`, `F10`, `↑`); mouse and scroll events use rounded pills (`Click`,
`Right Click`, `Scroll ↑`). Holding a key coalesces into a counter
(`Backspace ×7`) when `combine_repeats` is on. Each chip holds for
`display_ms` after its last press, then fades over `fade_ms`.

| Field | Type | Default | Notes |
|---|---|---|---|
| `enabled` | bool | `false` | Start with the HUD on |
| `mode` | enum | `"auto"` | `auto`, `overlay`, or `system` |
| `position` | enum | `"bottom-center"` | Nine screen anchors (3×3 grid) |
| `show_mouse` | bool | `true` | Show buttons and scroll |
| `show_bare_modifiers` | bool | `true` | Show lone Ctrl/Shift/Alt taps |
| `display_ms` | u64 | `1600` | Hold before fading (200–30000) |
| `fade_ms` | u64 | `350` | Fade duration (0–5000) |
| `max_entries` | usize | `6` | Simultaneous chips (1–16) |
| `combine_repeats` | bool | `true` | Coalesce repeats into `×N` |
| `font_size` | f64 | `18.0` | Chip label size (6–72) |

**Input sources.** A Wayland client only receives input delivered to its *own*
surfaces, so what the HUD can see depends on the mode:

- `"overlay"` (works everywhere, no permissions): shows only the keys, clicks,
  and scrolls Wayscriber itself receives. That covers the primary presenter
  workflow — you are drawing on screen while talking. During Light Mode
  passthrough there is nothing to report, because input goes to the app
  underneath.
- `"system"`: a reader thread on libinput/evdev shows *all* input on the seat,
  including what flows to the app underneath during passthrough or while the
  overlay is hidden. This requires a build with the `input-monitor` cargo
  feature (opt-in, not in the default feature set) **and** read access to
  `/dev/input`, which normally means `input` group membership:

  ```bash
  sudo usermod -aG input "$USER"   # then log out and back in
  ```

  When either requirement is missing, the HUD falls back to overlay mode and
  shows a warning toast naming the actual cause — an unreadable `/dev/input`
  gets the group guidance above, while an empty seat, an uncompilable keyboard
  layout, or a read error each say so instead. The same fallback happens when
  the seat has no readable keyboard, pointer, or tablet device, so system mode
  never sits there silently reporting nothing. Capture follows the session's
  own seat (`XDG_SEAT`, defaulting to `seat0`), and devices are attributed to
  seats through udev exactly as libinput does it, so on a multi-seat machine
  another seat's hardware never counts as yours.

  Switching to system capture is a handshake: the HUD keeps reporting overlay
  input until the reader has opened the seat and found a usable device, and
  only then announces `Input HUD: system-wide input`. Nothing is lost or
  double-reported in between, and a seat that turns out to be unusable simply
  stays on overlay with the warning above.
- `"auto"` (default): system-wide capture when it is available, overlay-only
  otherwise, with no warning. The fallback is silent however it is reached —
  missing permissions, an empty seat, or a reader that fails after starting —
  and only the log records why; enabling the HUD still toasts which source you
  ended up with.

While the system source is active it reports every press once — the overlay
hooks stay silent, so nothing is shown twice.

**Privacy.** System mode sees every keystroke on the seat, including passwords
typed into other applications. This is inherent to the feature class (KeyCastr
and showmethekey have the same exposure). Mitigations shipped: the HUD is off
by default, system mode is an explicit opt-in on an opt-in build, one chord
toggles it off, and chip labels are render-only — they are never logged and
never written to a session file. Wayscriber cannot detect password fields from
this side of the compositor and does not pretend to.

**Known limitations.**
- System mode reads the keyboard layout from the environment
  (`XKB_DEFAULT_LAYOUT` and friends), which can differ from the compositor's
  live layout. Overlay mode always matches the compositor.
- GTK toolbar surfaces are separate windows, so clicks on them do not appear
  in overlay mode; system mode covers them.
- The two modes count held keys slightly differently. Overlay mode ticks the
  `×N` counter from Wayscriber's own auto-repeat, which deliberately excludes
  one-shot action keys such as <kbd>Enter</kbd> and <kbd>Tab</kbd>; system
  mode follows the keymap's own repeat flags, so those keys do count there.
- System mode enumerates devices once at startup, so a keyboard plugged in
  afterwards is picked up by libinput but a seat that was empty at startup
  falls back to overlay; toggle the HUD off and on to re-evaluate. Unplugging
  a keyboard while a key is held retires only that keyboard's held keys, so a
  modifier still down on another keyboard — and session state such as Caps
  Lock or the selected layout group — keeps working.
- Typing into Wayscriber's own text tool *is* shown (that is the point when
  demoing). IME-composed text and touch events are not shown in this release.
- Focus Mode hides chrome, not presentation aids: the HUD keeps rendering,
  the same decision click highlights use.

### `[presenter_mode]` - Presenter Mode

Control which UI elements presenter mode hides and how tools behave when it is active.

```toml
[presenter_mode]
hide_status_bar = true
hide_toolbars = true
toolbar_mode = "hidden"
hide_tool_preview = true
close_help_overlay = true
enable_click_highlight = true
enable_input_hud = false
tool_behavior = "force-highlight"
show_toast = true
```

`enable_input_hud = true` forces the input HUD on at presenter-mode entry and
restores the previous value on exit; while it is forced, the manual toggle is
ignored (the same contract `enable_click_highlight` follows).

**Toolbar mode options** (what `hide_toolbars` does to the top strip):
- `"hidden"` (default): hide the top strip along with the side toolbars
- `"micro"`: collapse the top strip to the 44px micro chip (active tool glyph in a ring of the current color); side toolbars still hide

**Tool behavior options:**
- `"keep"`: Leave the active tool unchanged
- `"force-highlight"`: Switch to highlight on entry, allow tool changes
- `"force-highlight-locked"`: Switch to highlight and lock tools while presenting

### Light Passthrough Mode

Light mode hides UI chrome and sets the overlay to click-through passthrough until drawing is explicitly enabled. `toggle_light_mode` defaults to <kbd>F6</kbd>, but that is a Wayscriber in-overlay shortcut: it works while the overlay still has focus. Once passthrough is active, normal keyboard and pointer input goes to the app underneath, so do not rely on in-overlay shortcuts as the way back out. Compositor/global shortcuts should call the daemon commands below for reliable control.

This mode requires compositor overlay support through layer-shell. It is disabled on the xdg fallback because regular app windows cannot reliably stay visible as click-through shell overlays while keyboard and pointer input go to apps underneath. On stock GNOME Wayland, Freeze may still work for still-image capture when portal capture is available, but it is not a live passthrough replacement. True passthrough would require a GNOME Shell extension companion.

For compositor/global shortcuts while passthrough is active, run:

```sh
wayscriber --light-toggle
wayscriber --light-draw-toggle
wayscriber --light-draw-on
wayscriber --light-draw-off
```

Use `--light-draw-on` on key/button press and `--light-draw-off` on release for a non-sticky draw-while-held shortcut. The raw `--daemon-action` form remains available for scripts.

### `[ui.toolbar]` - Floating Toolbars

Controls the top and side toolbars (<kbd>F9</kbd> toggles both; <kbd>F2</kbd> cycles the top strip full → micro → hidden).

```toml
[ui.toolbar]
# Toolbar frontend: "auto" (GTK4 bars where the compositor supports
# layer-shell toolbars, built-in bars elsewhere), "gtk", or "builtin"
backend = "auto"

# Toolbar layout preset: "simple" or "full"
# Legacy values: "regular" and "advanced" (both map to Full UI label)
layout_mode = "full"

# Optional per-mode overrides for toolbar sections
# Use true/false to override a section; omit to use the mode default.
#
# [ui.toolbar.mode_overrides.simple]
# show_presets = false
# show_actions_section = true
# show_actions_advanced = false
# show_zoom_actions = true
# show_pages_section = true
# show_boards_section = true
# show_step_section = false
# show_text_controls = true
#
# [ui.toolbar.mode_overrides.regular] # Full mode overrides
# show_presets = true
# show_actions_section = true
# show_actions_advanced = false
# show_zoom_actions = true
# show_pages_section = true
# show_boards_section = true
# show_step_section = false
# show_text_controls = true
#
# [ui.toolbar.mode_overrides.advanced] # Legacy mode overrides
# show_presets = true
# show_actions_section = true
# show_actions_advanced = true
# show_zoom_actions = true
# show_pages_section = true
# show_boards_section = true
# show_step_section = true
# show_text_controls = true

# Show top toolbar on startup (pinned)
top_pinned = true

# Show side toolbar on startup (pinned)
side_pinned = true

# Start toolbars minimized to their edge restore tabs
top_minimized = false
side_minimized = false

# Authored default display form of the top strip: "full" or "micro".
# "hidden" is accepted but treated as "full" (startup visibility is
# governed by top_pinned). Cycling the strip at runtime saves the chosen
# form to runtime-ui.toml instead of rewriting this value
top_display_mode = "full"

# Where the side-palette functions live: "pill" (default, supported) or
# "panel" ("pill" retires the side palette — drawing props live in the style
# pill, canvas management in the "Canvas…" overflow popover + bottom-right zoom
# chip + status-bar board picker, presets in the top presets island, and
# Session/Settings in top-strip overflow popovers; "panel" is the deprecated
# legacy escape hatch, scheduled for removal one release after the pill default)
side_layout = "pill"

# Side-palette pane restored at startup: "draw", "canvas", "session", or "settings"
# (legacy side_layout = "panel" only)
side_active_pane = "draw"

# Side-palette sections collapsed to their header row
collapsed_sections = []

# Use icons instead of text labels in toolbars
use_icons = true

# Scale factor for toolbar UI (icons + layout)
scale = 1.0

# Show extended color palette in the top toolbar
show_more_colors = false

# Show basic actions (undo/redo/clear) in the side toolbar
show_actions_section = true

# Show advanced actions (undo all, delay, freeze, etc.)
show_actions_advanced = false

# Show zoom actions (zoom in/out/reset/lock)
show_zoom_actions = true

# When the bottom-right zoom chip is shown
# Options: "always" (default; the chip is also the mouse entry point for
# zooming), "while-zoomed" (only while zoom is active — keeps the corner
# clean at 100%; zooming still starts via keyboard/scroll bindings)
zoom_chip_display = "always"

# Master visibility for the bottom-right zoom chip. The toggle_zoom_chip
# palette/keyboard action flips it for the current run only; this value is the
# default it starts from.
show_zoom_chip = true

# Show page controls section (prev/next/new/dup/del)
show_pages_section = true

# Show board controls section (prev/next/new/del)
show_boards_section = true

# Show presets section in the side toolbar
show_presets = true

# Show Step Undo/Redo section
show_step_section = false

# Keep text controls visible even when text is inactive
show_text_controls = true

# Deprecated compatibility mirror. Settings navigation is always reachable;
# this value is preserved for older Wayscriber versions but is otherwise ignored.
show_settings_section = true

# Show delayed undo/redo sliders in the side toolbar
show_delay_sliders = false

# Show the marker opacity slider at the bottom of the side toolbar even when the marker tool isn't selected
show_marker_opacity_section = false

# Enable context-aware UI that shows/hides controls based on the active tool
context_aware_ui = true

# Show preset action toast notifications on apply/save/clear
show_preset_toasts = true

# Show cursor tool preview bubble
show_tool_preview = false

# Authored default toolbar offsets (layer-shell/inline). Dragging a toolbar
# saves its position to runtime-ui.toml instead of rewriting these
top_offset = 0.0
top_offset_y = 0.0
side_offset = 0.0
side_offset_x = 0.0

# Force inline toolbars even when layer-shell is available
force_inline = false

# Modifier-click a toolbar action to capture a replacement shortcut.
# Values: "ctrl_shift", "ctrl_alt", "shift_alt", "ctrl_shift_alt", "disabled"
rebind_modifier = "ctrl_shift"

[ui.toolbar.items]
# Hide individual toolbar items or whole side sections by stable ID.
# Unknown IDs are warned about but preserved across toolbar saves.
# Section-level ids (side.group.*) are explicit overrides that beat the
# layout-mode baseline and survive mode switches.
hidden = [
  "top.utility.screenshot",
  "top.tool.blur",
  "top.utility.clear-canvas",
  "side.actions.undo-all",
  "side.group.presets",
]

# IDs explicitly shown, overriding the layout-mode baseline (e.g. presets
# kept visible in simple mode).
shown = []

[ui.toolbar.items.order]
# Optional order overrides. Empty lists use the built-in order.
# Known IDs omitted from a non-empty list append in the default order.
# Side section ordering uses runtime block representatives; detailed sections
# such as eraser-mode, polygon-sides, and font remain visibility-only IDs.
top_tools = [
  "top.tool.select",
  "top.tool.pen",
  "top.tool.marker",
  "top.tool.step-marker",
  "top.tool.eraser",
]
top_controls = [
  "top.utility.text",
  "top.utility.sticky-note",
  "top.utility.screenshot",
  "top.utility.clear-canvas",
  "top.utility.highlight",
]
side_sections = [
  "side.group.colors",
  "side.group.presets",
  "side.group.thickness",
  "side.group.actions",
  "side.group.pages",
  "side.group.boards",
  "side.group.settings",
]
```

**Behavior:**
- **Icon/text mode**: `use_icons` switches between compact icons and labeled buttons.
- **Scale**: `scale` multiplies toolbar UI sizing (useful for HiDPI when output scale=1).
- **Colors**: `show_more_colors` toggles the extended palette row.
- **Layout**: `layout_mode` picks a preset complexity level; `mode_overrides` lets you customize each mode.
- **Actions**: `show_actions_section` shows the basic actions row; `show_actions_advanced` reveals the extended actions.
- **Zoom actions**: `show_zoom_actions` toggles the zoom controls in the Canvas drawer.
- **Pages**: `show_pages_section` toggles the page navigation block.
- **Boards**: `show_boards_section` toggles the board navigation block.
- **Presets**: `show_presets` hides/shows the preset slots section.
- **Text controls**: `show_text_controls` keeps font size/family visible even when text isn’t active.
- **Multi-step undo/redo**: `show_step_section` hides/shows the Step Undo/Redo section.
- **Settings**: the Settings pane is always reachable. The serialized `show_settings_section` key and matching per-mode override are deprecated compatibility fields and no longer hide navigation.
- **Delays**: `show_delay_sliders` shows the timed undo/redo-all sliders in the side panel.
- **Marker opacity**: the marker opacity slider appears when the marker tool is active; `show_marker_opacity_section` keeps it visible even when using other tools.
- **Polygon tools**: Full mode shows Triangle, Parallelogram, Rhombus, Regular Polygon, and Freeform Polygon under the compact Polygons picker. Simple mode exposes them in the Shapes picker.
- **Context-aware UI**: `context_aware_ui` shows/hides tool-specific controls (colors, thickness, arrow labels, etc.) based on the active tool; disable to always show all controls.
- **Preset toasts**: `show_preset_toasts` enables toast confirmations for preset apply/save/clear.
- **Tool preview**: `show_tool_preview` toggles the cursor bubble.
- **Offsets**: `top_offset`, `top_offset_y`, `side_offset`, `side_offset_x` are the authored default toolbar positions. Dragging a toolbar saves its position as a runtime preference in `runtime-ui.toml` and leaves these untouched; editing one here again takes over from the saved drag. A side drag also records the top strip's reconciled horizontal offset, because moving the palette can change where the strip rests.
- **Force inline**: `force_inline` (or `WAYSCRIBER_FORCE_INLINE_TOOLBARS`) skips layer-shell toolbars.
- **Shortcut editing**: hold `rebind_modifier` while clicking a bindable toolbar action to capture a replacement shortcut. The command palette also exposes edit, unbind, and reset controls for each configurable action. Conflicting shortcuts are rejected without changing the saved configuration.
- **Backend**: `backend` (or `WAYSCRIBER_TOOLBAR_BACKEND`) picks the toolbar frontend. `auto` uses the GTK4 bars exactly where the built-in bars would own separate layer surfaces (layer-shell present, no forced inline, no overlay-layer canvas) and falls back to the built-in Cairo bars everywhere else, including at runtime if GTK fails to start. `gtk` warns when unsupported and then falls back; `builtin` always uses the Cairo bars.
- **Pinned**: `top_pinned`/`side_pinned` are the authored defaults for whether each toolbar opens on startup. Pinning or unpinning in the overlay saves to `runtime-ui.toml` and leaves these values alone.
- **Minimize**: the toolbar minimize button (the dash that replaced the X) collapses a bar to a small edge tab instead of hiding it, so there is always an on-screen way back; `top_minimized`/`side_minimized` are the authored defaults, and the state you leave a bar in survives restarts as a runtime preference in `runtime-ui.toml`. F9 still toggles full visibility.
- **Micro mode**: `cycle_toolbar_display` (default <kbd>F2</kbd>) cycles the top strip full → micro → hidden. Micro collapses the strip to one 44px round chip showing the active tool inside a ring stroked in the current color (ring width follows stroke thickness); clicking the chip restores the full strip. The full/micro form persists as a runtime preference in `runtime-ui.toml`, seeded by the authored `top_display_mode`; the hidden step is runtime-only like F9. Entering micro un-minimizes the strip; if a config sets both `top_minimized` and micro, the minimized restore tab wins.
- **Idle fade**: the top-strip islands dim to 55% opacity after ~4 seconds without drawing activity and restore when the pointer approaches the toolbar (or on the next stroke). Open top-strip menus, the minimized tab, and the micro chip never fade. With `[ui] reduced_motion` the fade snaps instantly instead of animating; there is no separate config key.
- **Side layout**: `side_layout` picks where the side-palette functions live, and the top-only re-homing is now complete. The default `"pill"` is the **supported layout**: the standalone side palette is fully retired — its surface is never created (layer-shell, inline fallback, or GTK) — and every pane has a concrete new home. Drawing properties (colors included) live in the top strip's contextual style pill; canvas management lives in the **"Canvas…" overflow popover** (opened from the top strip's `⋯` overflow — boards, pages, zoom, advanced, and step controls) plus the **bottom-right zoom chip** and the **status-bar board picker**; presets live in the **top-strip presets island**; and the Session/Settings panes live in popovers opened from the overflow menu (the "Session..." / "Settings..." entries; the popovers expose the same controls the panes did). `"panel"` is the **deprecated legacy escape hatch** restoring the classic four-pane side palette; it is deprecated and planned for removal one release after the pill default. Panel-mode users see a once-per-session notice pointing at these new homes. (The original plan document called this key `layout_mode = "panel"`, but `layout_mode` is an orthogonal complexity preset — Simple/Regular/Advanced — so the switch lives under its own `side_layout` key instead.)
- **Side panes**: `side_active_pane` restores the last side-palette pane (`draw`, `canvas`, `session`, `settings`); `collapsed_sections` remembers which sections are collapsed to their header row (e.g. `["colors", "step-undo"]`). Both are authored seeds: as you use the overlay it records the current pane and collapsed set in `runtime-ui.toml` rather than rewriting these keys. Unknown ids are ignored at runtime but preserved across saves. Both keys (and `side_pinned`/`side_minimized`) only take effect under the deprecated legacy `side_layout = "panel"`; under the default pill layout they are inert.
- **Session/Settings popovers**: under the default pill layout the top strip's overflow menu always carries "Session..." and "Settings..." entries (they also appear under the legacy panel layout — the popovers are transient quick surfaces, not a second pinned pane). Opening one closes the other and the overflow menu; Escape and clicking away dismiss it. Content taller than the popover cap scrolls internally.
- **Hidden items**: `ui.toolbar.items.hidden` removes known toolbar buttons/sections from sizing, drawing, and hit testing while preserving unknown future IDs.
- **Shown items**: `ui.toolbar.items.shown` pins sections visible against the layout-mode baseline. Together with `hidden` these are the single visibility store: the `show_*` booleans are written as read-only mirrors for older versions, and legacy configs fold into explicit overrides at load.
- **Layout modes are non-destructive presets**: switching Simple/Regular/Advanced re-baselines section visibility without erasing your explicit toggles; Advanced is selectable from the overlay's Settings pane. The section ids `side.group.actions-advanced`, `side.group.zoom-actions`, and `side.group.text-controls` carry the advanced/zoom/persistent-text overrides. Switching modes from the overlay re-baselines the current run only; set the durable `layout_mode` in the configurator. Sections you pinned through `items.shown`/`items.hidden` keep their override under every mode.
- **Item order**: `ui.toolbar.items.order.top_tools`, `top_controls`, and `side_sections` reorder supported toolbar items. `side_sections` orders runtime block representatives; `side.group.eraser-mode`, `side.group.polygon-sides`, and `side.group.font` can be hidden individually but are not independently orderable. Unknown future IDs and wrong-group IDs are ignored at runtime but preserved across saves.
- **Live customization**: the overlay Customize tab supports show/hide, move up/down, and drag reorder for supported groups. The configurator supports the same saved order with up/down controls.
- **Top strip items**: `top.group.quick-colors` (the swatch row + current-color chip) and `top.utility.undo`/`top.utility.redo` are hideable ids. `top.chrome.overflow` is a structural affordance that appears whenever its menu has content — which is always: the menu anchors Clear (`top.utility.clear-canvas`, unless that item is hidden), anything width pressure moves into it, and the non-hideable "Session..." / "Settings..." popover entries. The icon/text mode toggle lives in the Settings surface (the side palette's Settings pane under the legacy panel layout, the Settings popover under pill).
- **Clear canvas**: Clear lives in the top strip's overflow (⋯) menu. A plain click clears and shows a short "Cleared — Undo?" toast; Shift+click clears instantly without the toast. The `clear_canvas` keyboard action is always instant and shows no toast.
- **Recoloring a swatch**: right-clicking any quick-color swatch (style pill or side palette) opens the color picker bound to that palette slot, titled "Recolor &lt;slot&gt;". The swatch tracks the gradient live, OK applies the color to that slot for the current run, and Cancel/Escape restores it. To keep a recolored palette, set it in the configurator's Drawing screen — the popup's action button opens it there. The slot keeps its label and shortcut, so R still selects the red slot after you point it at a different red. Recoloring the swatch you are currently drawing with moves the live color with it; recoloring any other slot leaves your current color alone. Left-clicking a swatch still just selects it, and the leftmost chip still opens the picker for the active tool's own color.
- **Restoring a swatch's shipped color**: while recoloring a slot, the picker adds a **Default** button next to OK/Cancel that loads the color wayscriber ships for that slot. It stages the color like any other pick — the swatch previews it, OK applies it for the run, Cancel backs out — so it is not a separate destructive action. The button only appears for the eleven built-in slots; extra slots you added past them have no shipped default, and the tool-color picker never shows it. Restoring sets the built-in value in your palette rather than deleting the entry, so the slot keeps its identity.
- **Shapes popover options**: the Fill checkbox (`top.utility.fill`) remains available in the Shapes popover whenever that item is enabled, even while another tool is active, so it can configure the next fill-capable shape. The polygon side count appears only while Regular Polygon is active. These controls live in the popover instead of a permanently reserved mini-checkbox lane under the bar, keeping the bar 58px tall. The highlight-ring row still appears under the Highlight button, but only while the highlight tool is active.
- **Screenshot toolbar button**: `top.utility.screenshot` is hidden by default; remove it from `ui.toolbar.items.hidden` or enable it in the configurator/overlay customization to show it.

**Defaults:** all set as above.

### `[boards]` - Boards (Backgrounds + Names)

Configure multiple boards (each with its own pages) plus the special transparent overlay.

```toml
[boards]
max_count = 9
auto_create = true
show_board_badge = true
pan_enabled = true
show_pan_badge = true
persist_customizations = true
default_board = "transparent"

[[boards.items]]
id = "transparent"
name = "Overlay"
background = "transparent"
persist = true

[[boards.items]]
id = "whiteboard"
name = "Whiteboard"
background = { rgb = [0.992, 0.992, 0.992] }
# Tuned black #241F31; the built-in default bit-matches the "black" quick color
default_pen_color = { rgb = [0.141, 0.122, 0.192] }
auto_adjust_pen = true

[[boards.items]]
id = "blackboard"
name = "Blackboard"
background = { rgb = [0.067, 0.067, 0.067] }
default_pen_color = { rgb = [1.0, 1.0, 1.0] }
auto_adjust_pen = true

[[boards.items]]
id = "blueprint"
name = "Blueprint"
background = { rgb = [0.063, 0.125, 0.251] }
default_pen_color = { rgb = [0.902, 0.945, 1.0] }

[[boards.items]]
id = "corkboard"
name = "Corkboard"
background = { rgb = [0.420, 0.294, 0.165] }
default_pen_color = { rgb = [0.969, 0.890, 0.784] }
```

**Fields:**
- `max_count` — hard cap on total boards.
- `auto_create` — create a board when switching to an empty slot.
- `show_board_badge` — show board name/slot in the status bar.
- `pan_enabled` — allow panning on solid-color boards with <kbd>Space</kbd> + left-drag.
- `show_pan_badge` — show the pan hint in the status bar or as a floating badge.
- `persist_customizations` — **deprecated no-op**, still parsed so existing files load without a
  warning. Board renames, recolors, additions, and deletions belong to the running session (and are
  saved with it when the board sets `persist`); the list below is the set of templates a new session
  starts from, edited in the configurator. The key is ignored whatever you set it to and will be
  removed in a future release.
- `default_board` — board id to activate on startup.
- `items` — ordered list of boards; each board has:
  - `id` — stable identifier (used by keybindings and persistence).
  - `name` — display name in the UI.
  - `background` — `"transparent"` or `{ rgb = [..] }`.
  - `default_pen_color` — optional; if omitted and `auto_adjust_pen = true`, pen color is auto-contrasted.
  - `auto_adjust_pen` — auto-switch pen color on entry.
  - `persist` — include this board in session saves.

**Keybindings:**
- <kbd>Ctrl+Shift+1..9</kbd>: Switch board slots
- <kbd>Ctrl+Shift+Left/Right</kbd>: Previous/next board
- <kbd>Ctrl+Shift+N</kbd>: New board
- <kbd>Ctrl+Shift+Delete</kbd>: Delete board
- <kbd>Ctrl+Shift+B</kbd>: Board picker (inline rename/color)
- Aliases (configurable): <kbd>Ctrl+W</kbd> = whiteboard, <kbd>Ctrl+B</kbd> = blackboard, <kbd>Ctrl+Shift+T</kbd> = transparent

**Board Picker:**
- Modal list for switching, renaming, and recoloring boards.
- Inline edits apply to the active session, not to the templates in `config.toml`. Edit the
  templates in the configurator's Boards screen.

**Solid-board pan:**
- Hold <kbd>Space</kbd> and drag with the left mouse button to pan whiteboards and other solid-color boards.
- Transparent overlay does not pan; it stays anchored to the live screen.
- The canvas context menu includes **Reset Canvas Position** when board panning is enabled.
- The same right-click menu exposes **Zoom** → **Zoom In**, **Zoom Out**, and **Reset Zoom**.
- Right-click menus expose **Paste**; shape menus also expose **Copy** for the selected annotations.
- Pan offsets are stored per page, so each page keeps its own position.

**CLI Override:**
Use a board id with `--mode`:
```bash
wayscriber --active --mode whiteboard
wayscriber --active --mode blueprint
wayscriber --daemon --mode transparent
```

### `[board]` - Legacy Board Modes

This section is still recognized for backward compatibility. If `[boards]` is missing,
wayscriber will synthesize boards from `[board]`. New configurations should prefer `[boards]`.

### `[render_profiles]` - Render Color Profiles

Render profiles preview an alternate final color mapping without changing saved shapes or board
data. They are useful for print, projectors, grayscale-ish previews, and light/dark sharing
workflows.

```toml
[render_profiles]
# Optional profile id to preview on startup
# active = "print"
apply_to_canvas = true
apply_to_ui = true
export = "off"
# export_profile = "print"

[[render_profiles.profiles]]
id = "print"
name = "Print"
mappings = [
  { from = "#000000", to = "#FFFFFF" },
  { from = "#FFFFFF", to = "#000000" },
  { from = "#FFFF00", to = "#8B4513" },
  { from = "#00FF00", to = "#006400" },
]
```

**Behavior:**
- `id` is the stable identifier used by `active` and runtime profile switching.
- Profile entries are stored under `profiles`.
- `apply_to_canvas` controls board backgrounds, annotations, and canvas-space editor previews such as selections, hover rings, provisional strokes, text-edit previews, and click highlights.
- `apply_to_ui` controls screen-space Wayscriber UI chrome, status text, popups, command palette, and toolbars.
- `export` controls explicit canvas PNG export remapping: `off`, `active`, or `profile`.
- `export_profile` is used only when `export = "profile"`.
- `mappings` use exact RGB matches. Accepted input forms are `#RRGGBB`, `RRGGBB`, and `0xRRGGBB`; validation normalizes to `#RRGGBB`.
- Pixel alpha is preserved. Unmapped colors are unchanged.
- With both targets enabled, profiles apply to Wayscriber-rendered pixels: annotations, board backgrounds, UI chrome, toolbars, popups, embedded images, and frozen/zoom backgrounds when Wayscriber paints them.
- Set `apply_to_ui = false` to preview remapped canvas content while keeping screen-space UI text and controls in the normal theme.
- Profiles do not recolor the compositor-owned live desktop seen through a transparent overlay.
- Explicit canvas PNG export applies its resolved export profile to persisted Wayscriber canvas content only, uses the current panned board viewport, respects output scale, and excludes frozen/zoom desktop pixels.
- Board PDF export writes the active board or every board to a file with one PDF page per Wayscriber page. PDF export preserves board/page order and solid board backgrounds, but does not apply export render profiles.
- `[export.pdf]` controls PDF filename fallback, page size, orientation, fit mode, and optional page labels.
- Explicit canvas export and its clipboard-failure fallback save PNG data as `.png`; screenshot clipboard fallback still uses `[capture].format`.
- `[capture].enabled` disables compositor screenshot capture actions, not explicit export actions.
- Board PDF export is file-only; clipboard PDF export is not supported yet.

**Runtime actions:**
- `render_profile_next`
- `render_profile_previous`
- `render_profile_off`

**Canvas export actions:**
- `export_canvas_file`
- `export_canvas_clipboard`
- `export_canvas_clipboard_and_file`
- `export_board_pdf_file`
- `export_all_boards_pdf_file`

### `[capture]` - Screenshot Capture

Configures how screenshots are stored and shared.

```toml
[capture]
# Enable/disable capture shortcuts entirely
enabled = true

# Directory for saved screenshots (supports ~ expansion)
save_directory = "~/Pictures/Wayscriber"

# Filename template (strftime-like subset: %Y, %m, %d, %H, %M, %S)
filename_template = "screenshot_%Y-%m-%d_%H%M%S"

# Image format (currently "png")
format = "png"

# Copy captures to clipboard in addition to saving files
copy_to_clipboard = true

# Exit the overlay after any capture completes (forces exit for all capture types)
# When false, clipboard-only captures still auto-exit by default.
# Use --no-exit-after-capture to keep the overlay open for a run.
exit_after_capture = false
```

**Tips:**
- Set `copy_to_clipboard = false` if you prefer file-only captures.
- Clipboard-only shortcuts ignore the save directory automatically.
- `wl-clipboard`, `grim`, and `slurp` are installed automatically by deb/rpm/AUR packages. For source/tarball installs, add them manually; otherwise wayscriber falls back to `xdg-desktop-portal`.
- Use `--exit-after-capture` / `--no-exit-after-capture` to override exit behavior per run.

### `[export.pdf]` - PDF Export

Configures explicit PDF exports. If `filename_template` is omitted or blank, active-board PDF
exports reuse `[capture].filename_template` and save with a `.pdf` extension. All-board PDF exports
use `all_boards_filename_template`, then `filename_template`, then `[capture].filename_template`.

```toml
[export.pdf]
# filename_template = "board_%Y-%m-%d_%H%M%S"
# all_boards_filename_template = "boards_%Y-%m-%d_%H%M%S"
page_size = "viewport"       # viewport, a4, letter, custom
orientation = "auto"         # auto, portrait, landscape
fit = "viewport"             # viewport, fit-viewport-to-page, fit-content-to-page
transparent_background = "none" # none, desktop
custom_width = 800.0         # PDF points, used with page_size = "custom"
custom_height = 600.0
content_source_padding = 24.0 # source units, used with fit-content-to-page

[export.pdf.labels]
enabled = false
position = "bottom-center"   # top-left, top-right, bottom-left, bottom-right, bottom-center
content = "custom-template"  # custom-template, board-and-page, document-page, board-name, page-name
template = "{board_name} - {page_name} ({document_page}/{document_pages})"
font_family = "Sans"
font_size = 10.0
margin = 12.0
padding_x = 6.0
padding_y = 3.0
text_color = [0.1, 0.1, 0.1, 1.0]
background_enabled = true
background_color = [1.0, 1.0, 1.0, 0.85]
```

`fit = "viewport"` draws the viewport 1:1 without scaling. With the default
`page_size = "viewport"`, this preserves the legacy export. `fit-viewport-to-page` scales the
viewport into the configured page size. `fit-content-to-page` scales the page's padded annotation
bounds into the configured page size, falling back to the viewport for blank pages.

`transparent_background = "desktop"` is opt-in. It hides the overlay, captures the live desktop
visible on the active output, and uses that image behind transparent PDF pages. Solid boards keep
their configured background. If the desktop capture is denied or the active output cannot be
isolated, the PDF export fails and no file is saved.

Label templates support `{app_board}`, `{app_boards}`, `{export_board}`, `{export_boards}`,
`{page}`, `{pages}`, `{document_page}`, `{document_pages}`, `{board_name}`, and `{page_name}`.
Use `{{` and `}}` for literal braces. `content = "custom-template"` uses `template`; the other
content modes ignore it. Labels are drawn after canvas content, ellipsized to one line, and omitted
if the page is too small.

### `[tablet]` - Tablet/Stylus Input

Runtime toggles for tablet/stylus input (Wayland `zwp_tablet_v2`).

```toml
[tablet]
enabled = true
pressure_enabled = true
min_thickness = 1.0
max_thickness = 8.0
auto_eraser_switch = true
pressure_variation_threshold = 0.1
pressure_thickness_edit_mode = "disabled"
pressure_thickness_entry_mode = "pressure_only"
pressure_thickness_scale_step = 0.1

[tablet.stylus_button]
action = "toggle_radial_menu"

[tablet.stylus_button2]
# action = "undo"
```

**Notes:**
- Requires the `tablet-input` feature at build time (enabled in default release builds).
- Tablet input is enabled by default when the feature is compiled in; set `enabled = false` to opt out.
- `stylus_button` is the primary barrel button (`BTN_STYLUS` / 331); `stylus_button2` is the secondary barrel button (`BTN_STYLUS2` / 332).
- Barrel button `action` values use normal action names, such as `toggle_radial_menu`, `undo`, and `redo`. Omit `action` to leave a button unbound.

### `[session]` - Session Persistence

Optional on-disk persistence for your drawings. Enabled by default so sessions resume automatically.

```toml
[session]
persist_transparent = true
persist_whiteboard = true
persist_blackboard = true
persist_history = true
restore_tool_state = true
storage = "auto"
# custom_directory = "/absolute/path"
per_output = true
max_shapes_per_frame = 10000
max_file_size_mb = 50
compress = "auto"
auto_compress_threshold_kb = 100
backup_retention = 1
# max_persisted_undo_depth = 200
```

- `persist_*` — choose which boards survive restarts (`persist_transparent` for overlay, `persist_whiteboard`/`persist_blackboard` gate non-transparent boards for legacy compatibility)
- `persist_history` — when `true`, persist undo/redo stacks so that history survives restarts; set to `false` to save only visible drawings
- `restore_tool_state` — save pen colour, thickness, font size, arrow settings (including head placement), and status bar visibility; when `true`, the last-used tool state overrides config defaults at startup
- `storage` — `auto` (XDG data dir, e.g. `~/.local/share/wayscriber`), `config` (same directory as `config.toml`), or `custom`
- `custom_directory` — absolute path used when `storage = "custom"`; supports `~`
- `per_output` — when `true` (default) keep a separate session file for each monitor; set to `false` to share one file per Wayland display as in earlier releases
- `max_shapes_per_frame` — trims older shapes if a frame grows beyond this count when loading/saving
- `max_file_size_mb` — skips loading and writing session files beyond this size cap; image paste and autosave warn near the cap
- `compress` — `auto` (gzip files above the threshold), `on`, or `off`
- `auto_compress_threshold_kb` — size threshold for `compress = "auto"`
- `backup_retention` — how many rotated `.bak` files to keep (set to 0 to disable backups)
- `max_persisted_undo_depth` — optional cap for serialized history; default follows the runtime undo limit (set `persist_history = false` to skip history entirely)

> **Privacy note:** Session files are stored unencrypted. Clear the session directory or disable persistence when working with sensitive material.

The tray menu's **Session persistence settings…** entry opens the configurator on this section
rather than toggling the flags itself; the daemon reads `[session]` to draw its menu and never
writes it. For a single run, use `--resume-session` / `--no-resume-session` or
`WAYSCRIBER_RESUME_SESSION` instead.

Use the CLI helpers for quick maintenance:

- `wayscriber --session-info` prints the active storage path, file details, and shape counts.
- `wayscriber --clear-session` removes the session file, backup, and lock.
- `wayscriber --clear-tool-state` removes only the saved tool defaults from the session snapshot, preserving saved boards and history.
- `wayscriber --active --session-file ~/Documents/lecture-04.wayscriber-session` opens and saves a named session file directly.
- `wayscriber --freeze --session-file ~/Documents/lecture-04.wayscriber-session` starts frozen mode with that same named session target.
- `wayscriber --daemon --session-file ~/Documents/lecture-04.wayscriber-session` starts a daemon whose overlay activations use that named session target.
- `wayscriber --daemon-toggle --session-file ~/Documents/meeting.wayscriber-session` asks the running daemon to launch a hidden overlay with that named session target. If the overlay is already visible with a different target, hide it before switching.
- `wayscriber --session-info --session-file <path>`, `wayscriber --clear-session --session-file <path>`, and `wayscriber --clear-tool-state --session-file <path>` target only that named file.

Config values seed startup defaults. When `restore_tool_state = true`, the saved session tool state is applied after those defaults, so edits such as `[arrow] head_at_end = true` can appear ignored if the session snapshot still stores an older arrow setting. Run `wayscriber --clear-tool-state` (or add `--session-file <path>` for a named session) to make config defaults apply on the next startup without deleting saved boards. In a running overlay, Command Palette -> Reset Tool Defaults clears the saved layer for the active session and immediately applies config defaults to the current tools so the next autosave keeps those defaults.

The configurator Session tab exposes the same distinction for recent named sessions: Clear Tool State preserves saved boards/history while removing only persisted tool settings; Clear Saved Data removes saved session files. Offline catalog actions are disabled while an overlay, manually started daemon, or background service is active. Use the command palette for the active overlay session.

The overlay Session panel lives in the side toolbar's Settings drawer:

- `Open` loads an existing named session, saves dirty current data first when needed, and records the target in the recent catalog.
- `Save As` writes the current overlay to another named session and switches the active target. It appends `.wayscriber-session` when no extension is supplied and asks before replacing existing session artifacts.
- `Info` reports the active session file size, board shape counts, and history status.
- `Clear` writes a durable empty session boundary for the active target.
- Recent session rows reopen other named sessions. If a recent target is missing, Wayscriber removes that stale catalog entry after the failed open.
- `Manager` opens the configurator. Overlay Open/Save As dialogs use `zenity` or `kdialog`.

The configurator Session tab also shows recent named sessions from the catalog, recorded when named-session targets are opened or saved from the CLI, daemon, or overlay. It can rename catalog display labels, reveal file locations, and forget catalog metadata without touching files. Duplicate, Move, and Clear are disabled while an overlay, manually started daemon, or background service is active.

Session overrides and recovery:

- CLI flags: `--resume-session` forces persistence on, `--no-resume-session` forces it off for the current run. The environment variable `WAYSCRIBER_RESUME_SESSION=1/0` does the same.
- `--session-file` implies session persistence for that overlay run and conflicts with `--no-resume-session`. Named sessions use the exact selected file path; Wayscriber does not create missing parent directories or fall back to configured storage. Foreground/open targets reject directories, symlinks, and special files.
- Size fallback: if visible drawings fit but persisted undo/redo history would exceed save or restore safety limits, autosave saves the drawings and warns once per run that history was trimmed or omitted.
- Image paste guard: when a pasted image would push visible session data over `max_file_size_mb`, Wayscriber blocks the paste and points you to `[session] max_file_size_mb`; when only undo history is at risk, the paste is allowed with a warning.
- Load safety: compressed session files are checked against an internal expanded-size cap while saving and loading. If an existing file expands beyond that cap, wayscriber refuses to load it, leaves the primary session file unchanged, and avoids overwriting it until session data changes.
- Recovery: if a session file is corrupt or cannot be parsed/decompressed, wayscriber logs a warning, writes a `.bak` copy of the bad file, removes the corrupt file, and continues with defaults. Overrides above still apply after recovery.

For end-to-end CLI, overlay, and configurator flows, see [`examples/session-manager.md`](../examples/session-manager.md).

### `[keybindings]` - Custom Keybindings

Customize keyboard shortcuts for all actions. Each action can have multiple keybindings.
For multi-monitor, customize `focus_prev_output` and `focus_next_output` in this section.

#### How a shortcut you did not write is decided

A `[keybindings]` field your file spells out is yours: it is used exactly as authored, including an
explicit empty list, which means "unbound". A field your file omits is filled in from this build's
defaults, but only where the key is still free — if the default's key is already claimed by
something you did bind, the default stands down and the action starts without it. Wayscriber says so
at startup and in the configurator — "X is a default shortcut for Y, but your configuration binds X
to Z; the default stays inactive and nothing was changed" — and the file is not changed either way.
That is why adding a shortcut to a new release can never quietly take over one of yours.

Two shortcuts you both authored on the same key are a conflict, not a stand-down: the first in
traversal order keeps the key, the other loses it for the session, and the diagnostic names both so
you can fix the file. Invalid shortcut text is reported and ignored for the session; its text stays
on disk untouched.

#### Reviewing an older `config_revision`

`config_revision` records which generation of shipped defaults a file was written against. Loading
never advances it and never rewrites the file. Instead, opening the configurator with an older
revision shows a **Configuration update available** banner listing every proposed shortcut change as
before → after. **Apply Update** changes the configurator draft only; nothing reaches disk until you
press Save, which writes the reviewed changes together with the new revision. **Dismiss** hides the
offer for that configurator run. Saving an unrelated setting without applying leaves both your old
bindings and your old `config_revision` on disk.

The revisions so far: revision 1 split the command-palette and full-screen-capture defaults
(`Ctrl+K` / `Ctrl+Shift+P` for the palette, `Ctrl+Alt+F` for capture); revision 2 moved `F2` out of
`toggle_toolbar` into the new `cycle_toolbar_display`; revision 3 gave `toggle_input_hud` its
`Ctrl+Shift+K` default. Customized fields are preserved by every recipe — a proposal only ever
targets a field you left at the value the older generation shipped.

Applying a revision proposal is optional. Presence-aware resolution already keeps new defaults out
of the way of anything you bound, so an old revision is safe to leave alone indefinitely; applying
one is a tidy-up that records your decision in the file.

**Contributing:** changing or adding a default keybinding no longer requires a
`CURRENT_CONFIG_REVISION` bump. A default is only ever offered to an action a configuration omits,
and only where the key is free, so a new or moved default cannot land on a shortcut a user bound to
something else (#293, #315); the skipped-default diagnostic reports the stand-down instead.
What is still required: the new default must not collide with another shipped default
(`default_keybindings_have_no_conflicts` guards that), and
`default_bindings_match_the_checked_in_snapshot` holds a snapshot of every shipped default and fails
until the snapshot records the change deliberately. Bumping `CURRENT_CONFIG_REVISION`
(`src/config/core.rs`) plus a recipe in `Config::apply_keybinding_migrations`
(`src/config/validate/keybindings.rs`) remains available and optional, for when an old default is
worth proactively cleaning out of existing files through the configurator's review flow.

```toml
[keybindings]
# Exit overlay (or cancel current action)
exit = ["Escape", "Ctrl+Q"]

# Enter text mode
enter_text_mode = ["T"]

# Enter sticky note mode
enter_sticky_note_mode = ["N"]

# Clear all annotations on current canvas
clear_canvas = ["E"]

# Undo last annotation
undo = ["Ctrl+Z"]

# Redo last undone annotation
redo = ["Ctrl+Shift+Z", "Ctrl+Y"]

# Optional undo/redo batch actions
undo_all = []
redo_all = []
undo_all_delayed = []
redo_all_delayed = []

# Duplicate current selection
duplicate_selection = ["Ctrl+D"]

# Copy/paste selection
copy_selection = ["Ctrl+Alt+C"]
paste_selection = ["Ctrl+Alt+V"]

# Select all annotations
select_all = ["Ctrl+A"]

# Reorder selected annotations within the stack
move_selection_to_front = ["]"]
move_selection_to_back = ["["]

# Nudge selection (hold Shift for a larger step)
nudge_selection_up = ["ArrowUp"]
nudge_selection_down = ["ArrowDown"]
nudge_selection_left = ["ArrowLeft", "Shift+PageUp"]
nudge_selection_right = ["ArrowRight", "Shift+PageDown"]

# Nudge selection (large step)
nudge_selection_up_large = ["PageUp"]
nudge_selection_down_large = ["PageDown"]

# Move selection to horizontal edges (left/right)
move_selection_to_start = ["Home"]
move_selection_to_end = ["End"]

# Move selection to vertical edges
move_selection_to_top = ["Ctrl+Home"]
move_selection_to_bottom = ["Ctrl+End"]

# Delete selection
delete_selection = ["Delete"]

# Adjust pen thickness
increase_thickness = ["+", "="]
decrease_thickness = ["-", "_"]

# Adjust marker opacity (when using the marker tool)
increase_marker_opacity = ["Ctrl+Alt+ArrowUp"]
decrease_marker_opacity = ["Ctrl+Alt+ArrowDown"]

# Tool selection shortcuts (optional; keep empty to rely on modifiers)
select_selection_tool = ["V"]
select_pen_tool = ["F"]
select_marker_tool = ["H"]
select_step_marker_tool = []
select_eraser_tool = ["D"]
toggle_eraser_mode = ["Ctrl+Shift+E"]
cycle_blur_style = []              # blur -> pixelate -> secure -> black out
select_spotlight_tool = []         # dim everything except a region
select_line_tool = []
select_rect_tool = []
select_ellipse_tool = []
select_triangle_tool = []
select_parallelogram_tool = []
select_rhombus_tool = []
select_regular_polygon_tool = []
select_freeform_polygon_tool = []
select_arrow_tool = []
select_blur_tool = []
select_highlight_tool = []
toggle_highlight_tool = ["Ctrl+Alt+H"]

# Reset label counters
reset_arrow_labels = ["Ctrl+Shift+R"]
reset_step_markers = []

# Adjust font size
increase_font_size = ["Ctrl+Shift++", "Ctrl+Shift+="]
decrease_font_size = ["Ctrl+Shift+-", "Ctrl+Shift+_"]

# Boards
toggle_whiteboard = ["Ctrl+W"]
toggle_blackboard = ["Ctrl+B"]
return_to_transparent = ["Ctrl+Shift+T"]
focus_prev_output = ["Ctrl+Alt+Shift+ArrowLeft"]
focus_next_output = ["Ctrl+Alt+Shift+ArrowRight"]
board_1 = ["Ctrl+Shift+1"]
board_2 = ["Ctrl+Shift+2"]
board_3 = ["Ctrl+Shift+3"]
board_4 = ["Ctrl+Shift+4"]
board_5 = ["Ctrl+Shift+5"]
board_6 = ["Ctrl+Shift+6"]
board_7 = ["Ctrl+Shift+7"]
board_8 = ["Ctrl+Shift+8"]
board_9 = ["Ctrl+Shift+9"]
board_prev = ["Ctrl+Shift+ArrowLeft"]
board_next = ["Ctrl+Shift+ArrowRight"]
board_new = ["Ctrl+Shift+N"]
board_duplicate = ["Ctrl+Shift+D"]
board_delete = ["Ctrl+Shift+Delete"]
board_picker = ["Ctrl+Shift+B"]

# Page navigation
# Ubuntu/GNOME defaults avoid Ctrl+Alt workspace shortcuts (Ctrl+ArrowLeft/Right, Ctrl+PageUp/PageDown).
page_prev = ["Ctrl+Alt+ArrowLeft", "Ctrl+Alt+PageUp"]
page_next = ["Ctrl+Alt+ArrowRight", "Ctrl+Alt+PageDown"]
page_new = ["Ctrl+Alt+N"]
page_duplicate = ["Ctrl+Alt+D"]
page_delete = ["Ctrl+Alt+Delete"]

# Toggle help overlay
toggle_help = ["F10", "F1"]

# Toggle quick reference overlay
toggle_quick_help = ["Shift+F1"]

# Toggle status bar visibility
toggle_status_bar = ["F12", "F4"]

# Show/hide the floating board/page badge (unbound; also in the command palette)
toggle_floating_badge = []

# Show/hide the bottom-right zoom chip (unbound; also in the command palette)
toggle_zoom_chip = []

# Focus mode: hide all UI chrome at once, press again to restore exactly
# (unbound; also in the command palette)
toggle_focus_mode = []

# Toggle toolbars (show/hide top and side together).
# Note: F2 moved to cycle_toolbar_display; hiding is still reachable via
# the cycle, and explicit user configs keep whatever they bound.
toggle_toolbar = ["F9"]

# Cycle the top toolbar's display: full strip -> micro chip -> hidden
cycle_toolbar_display = ["F2"]

# Toggle presenter mode
toggle_presenter_mode = ["Ctrl+Shift+M"]

# Toggle light passthrough mode while the overlay has focus
toggle_light_mode = ["F6"]

# Optional in-overlay toggle between light drawing and passthrough.
# Once passthrough is active, use compositor/global shortcuts that call
# `wayscriber --light-draw-toggle`, `--light-draw-on`, or `--light-draw-off`.
toggle_light_mode_drawing = []

# Optional render color profile preview controls
render_profile_next = []
render_profile_previous = []
render_profile_off = []

# Toggle click highlight (visual mouse halo)
toggle_click_highlight = ["Ctrl+Shift+H"]

# Toggle the input HUD (on-screen keystrokes and clicks)
toggle_input_hud = ["Ctrl+Shift+K"]

# Toggle fill for fill-capable shapes
toggle_fill = []

# Optional keyboard binding to toggle radial menu at cursor
toggle_radial_menu = []

# Toggle selection properties panel
toggle_selection_properties = ["Ctrl+Alt+P"]

# Toggle context menu (keyboard alternative to right-click)
open_context_menu = ["Shift+F10", "Menu"]

# Launch the desktop configurator (requires wayscriber-configurator)
open_configurator = ["F11"]

# Open the About window (version, links, update status). Unbound by default;
# also available from the toolbar chrome, the Settings popover, the help
# overlay footer, and the command palette. Opening it closes the overlay,
# because About is a normal window and the overlay draws above those.
open_about = []

# Toggle command palette
toggle_command_palette = ["Ctrl+K", "Ctrl+Shift+P"]

# Color selection shortcuts
set_color_red = ["R"]
set_color_green = ["G"]
set_color_blue = ["B"]
set_color_yellow = ["Y"]
set_color_orange = ["O"]
set_color_pink = ["P"]
set_color_white = ["W"]
set_color_black = ["K"]
# Screen eyedropper
pick_screen_color = ["I"]

# Screenshot shortcuts
capture_full_screen = ["Ctrl+Alt+F"]
capture_active_window = ["Ctrl+Shift+O"]
capture_selection = ["Ctrl+Shift+I"]

# Clipboard/File specific captures
capture_clipboard_full = ["Ctrl+C"]
capture_file_full = ["Ctrl+S"]
capture_clipboard_selection = ["Ctrl+Shift+C"]
capture_file_selection = ["Ctrl+Shift+S"]
capture_clipboard_region = ["Ctrl+6"]
capture_file_region = ["Ctrl+Alt+6"]
export_canvas_file = []
export_canvas_clipboard = []
export_canvas_clipboard_and_file = []
export_board_pdf_file = []
export_all_boards_pdf_file = []

# Open the most recent capture folder
open_capture_folder = ["Ctrl+Alt+O"]

# Toggle frozen mode
toggle_frozen_mode = ["Ctrl+Shift+F"]

# Zoom controls
zoom_in = ["Ctrl+Alt++", "Ctrl+Alt+="]
zoom_out = ["Ctrl+Alt+-", "Ctrl+Alt+_"]
reset_zoom = ["Ctrl+Alt+0"]
toggle_zoom_lock = ["Ctrl+Alt+L"]
refresh_zoom_capture = ["Ctrl+Alt+R"]

# Preset slots
apply_preset_1 = ["1"]
apply_preset_2 = ["2"]
apply_preset_3 = ["3"]
apply_preset_4 = ["4"]
apply_preset_5 = ["5"]
save_preset_1 = ["Shift+1"]
save_preset_2 = ["Shift+2"]
save_preset_3 = ["Shift+3"]
save_preset_4 = ["Shift+4"]
save_preset_5 = ["Shift+5"]
clear_preset_1 = []
clear_preset_2 = []
clear_preset_3 = []
clear_preset_4 = []
clear_preset_5 = []

# Help overlay (press F10 while drawing for a full reference)
```

**Keybinding Format:**

Keybindings are specified as strings with modifiers and keys separated by `+`:
- Simple keys: `"E"`, `"T"`, `"Escape"`, `"F10"`
- With modifiers: `"Ctrl+Z"`, `"Shift+T"`, `"Ctrl+Shift+W"`
- Special keys: `"Escape"`, `"Return"`, `"Backspace"`, `"Space"`, `"F10"`, `"F11"`, `"Home"`, `"End"`, `"PageUp"`, `"PageDown"`, `"ArrowUp"`, `"ArrowDown"`, `"ArrowLeft"`, `"ArrowRight"`, `"+"`, `"-"`, `"="`, `"_"`

**Supported Modifiers:**
- `Ctrl` (or `Control`)
- `Shift`
- `Alt`

**Modifier Order:**
Modifiers can appear in any order - `"Ctrl+Shift+W"`, `"Shift+Ctrl+W"`, and `"Shift+W+Ctrl"` are all equivalent.

**Multiple Bindings:**
Each action supports multiple keybindings (e.g., both `+` and `=` for increase thickness).

**Duplicate Detection:**
Duplicate keybindings are detected at startup and resolved one key at a time — the rest of both actions' shortcuts always keep working, and your config file is never rewritten. When two actions claim the same combination, the contested key is removed from one of them for that session:

- A binding you customized always beats one that still equals its built-in default. Most collisions are of this kind: a shortcut you never wrote gets filled in from the shipped defaults and lands on a key you assigned to something else.
- If both sides are customized, the earlier action in the internal keymap order (core, selection, tools, board, ui, colors, capture, zoom, presets) keeps the key.

Every resolution is reported: a warning toast and a desktop notification name the key and both actions at startup, the configurator shows them after loading or saving, and the details are written to the log. Because nothing is written back, edit `config.toml` to decide which action should own the shortcut permanently.

**Case Insensitive:**
Key names are case-insensitive in the config file, but will match the actual key case at runtime.

**Examples:**

Vim-style navigation keys:
```toml
[keybindings]
exit = ["Escape", "Q"]
clear_canvas = ["D"]
undo = ["U"]
```

Emacs-style modifiers:
```toml
[keybindings]
exit = ["Ctrl+G"]
undo = ["Ctrl+/"]
clear_canvas = ["Ctrl+K"]
```

Gaming-friendly (WASD area):
```toml
[keybindings]
exit = ["Q"]
toggle_help = ["H"]
undo = ["Z"]
clear_canvas = ["X"]
```

**Notes:**
- Modifiers (<kbd>Shift</kbd>, <kbd>Ctrl</kbd>, <kbd>Alt</kbd>, <kbd>Tab</kbd>) are always captured for drawing tools
- In text input mode, configured keybindings (like <kbd>Ctrl+Q</kbd> for exit) work before keys are consumed as text
- Color keys only work when not holding <kbd>Ctrl</kbd> (to avoid conflicts with other actions)
- Keybinding strings that cannot be parsed are detected at startup, reported, and dropped one string at a time for the running session; every other shortcut keeps working and the config file keeps the typo for you to fix
- Duplicate keybindings across actions are detected at startup, reported, and resolved per key without touching the config file

**Defaults:**
Defaults match the original hardcoded keybindings where possible. Copy/paste selection uses
<kbd>Ctrl+Alt+C</kbd>/<kbd>Ctrl+Alt+V</kbd>, so the clipboard-selection capture shortcut
defaults to <kbd>Ctrl+Shift+C</kbd> to avoid conflicts. The paste action also accepts PNG/JPEG
image data and local image files copied from a file manager.

## Creating Your Configuration

1. Create the directory:
   ```bash
   mkdir -p ~/.config/wayscriber
   ```

2. Copy the example config:
   ```bash
   cp config.example.toml ~/.config/wayscriber/config.toml
   ```

3. Edit to your preferences:
   ```bash
   nano ~/.config/wayscriber/config.toml
   ```

## Configuration Priority

Settings are loaded in this order:
1. Built-in defaults (hardcoded)
2. Configuration file values (override defaults)
3. Runtime changes made while Wayscriber is running (temporary, not saved)

Nothing in step 3 reaches `config.toml`. Preference actions in the overlay—layout mode, section and
status bar visibility, icon mode, click highlight, the input HUD, preset save/clear, quick color
recoloring, board edits—change the current run and reset on the next start; each one points at the
configurator screen that owns its configured default. The tray's session entry opens the
configurator rather than editing the file, and shortcuts are edited in the configurator only. A
configurator Save is the single write, and it rewrites only the settings you changed, without
reformatting unrelated settings or removing user comments.

Direct overlay manipulation—toolbar drags, pin/minimize, the display-form cycle, pane and section
collapse, individual item visibility and order, and board pins—goes to `runtime-ui.toml` and
survives a restart; see
[Configured defaults and runtime UI preferences](#configured-defaults-and-runtime-ui-preferences)
for the full table.

**Note:** Changes to the config file require restarting wayscriber daemon to take effect.

To reload config changes:
```bash
# Use the reload script
./reload-daemon.sh

# Or manually
pkill wayscriber
wayscriber --daemon &
```

## Environment Variables

These override behavior at runtime. Bool-ish values treat anything except `0`, `false`, or `off` as true.

- `WAYSCRIBER_NO_TRAY=1` disables the tray icon (default: tray enabled)
- `WAYSCRIBER_RESUME_SESSION=1/0` forces session persistence on/off for the current run (default: unset; follows config)
- `WAYSCRIBER_CONFIGURATOR=/path/to/wayscriber-configurator` overrides the configurator executable path
- `WAYSCRIBER_DISABLE_UPDATE_CHECK=1` disables the background update check for this run (overrides `[updates] check`; `--check-update` still works)
- `WAYSCRIBER_FORCE_INLINE_TOOLBARS=1` forces inline toolbars on Wayland (default: off)
- `WAYSCRIBER_TOOLBAR_DRAG_PREVIEW=0` disables inline toolbar drag preview (default: on)
- `WAYSCRIBER_TOOLBAR_POINTER_LOCK=1` enables pointer-lock drag path (experimental; default: on)
- `WAYSCRIBER_TOOLBAR_DRAG_THROTTLE_MS=12` throttles toolbar drag updates (default: 12; set 0 to disable)
- `WAYSCRIBER_DEBUG_TOOLBAR_DRAG=1` enables toolbar drag logging (default: off)
- `WAYSCRIBER_DEBUG_TOOLBAR_COLOR=1` enables toolbar color picker logging (default: off)
- `WAYSCRIBER_DEBUG_DAMAGE=1` enables damage region logging (default: off)
- `WAYSCRIBER_XDG_OUTPUT=...` forces GNOME fallback overlays onto a specific output (overrides `ui.preferred_output`)
- `WAYSCRIBER_XDG_FULLSCREEN=1` requests fullscreen GNOME fallback overlays (overrides `ui.xdg_fullscreen`)
- `WAYSCRIBER_XDG_FULLSCREEN_FORCE=1` bypasses the GNOME opacity safety check
- `RUST_LOG=info` enables Rust logging (default: unset; use `wayscriber=debug` for app-level logs)

## Troubleshooting

### Config File Not Loading

If your config file isn't being read:

1. Check the file path:
   ```bash
   ls -la ~/.config/wayscriber/config.toml
   ```

2. Verify TOML syntax:
   ```bash
   # Install a TOML validator if needed
   toml-validator ~/.config/wayscriber/config.toml
   ```

3. Check logs for errors:
   ```bash
   RUST_LOG=info wayscriber --active
   ```

### Invalid Values

If you specify invalid values:
- **Out of range**: Values will be clamped to valid ranges
- **Invalid color name**: Falls back to default (red)
- **Malformed RGB**: Falls back to default color
- **Parse errors**: Entire config file ignored, defaults used

Check the application logs for warnings about config issues.

## Advanced Usage

### Per-Project Configs

While wayscriber uses a single global config, you can:
1. Create different config files
2. Symlink the active one to `~/.config/wayscriber/config.toml`

Example:
```bash
# Create project-specific configs
cp config.example.toml ~/configs/wayscriber-presentation.toml
cp config.example.toml ~/configs/wayscriber-recording.toml

# Switch configs
ln -sf ~/configs/wayscriber-presentation.toml ~/.config/wayscriber/config.toml
```

### Configuration Examples

**High-contrast presentation mode:**
```toml
[drawing]
default_color = "yellow"
default_thickness = 5.0
default_font_size = 48.0

[ui]
status_bar_position = "top-right"
```

**Screen recording mode (subtle annotations):**
```toml
[drawing]
default_color = "blue"
default_thickness = 2.0
default_font_size = 24.0

[performance]
buffer_count = 4
enable_vsync = false
max_fps_no_vsync = 120
ui_animation_fps = 30

[ui]
show_status_bar = false
```

**Teaching/presentation mode (start in whiteboard):**
```toml
[boards]
default_board = "whiteboard"

[drawing]
default_thickness = 4.0
default_font_size = 42.0

[ui]
status_bar_position = "top-right"
```

**High-refresh display optimization:**
```toml
[performance]
buffer_count = 4
enable_vsync = false
max_fps_no_vsync = 144
ui_animation_fps = 120
```

## See Also

- `SETUP.md` - Installation and system requirements
- `config.example.toml` - Annotated example configuration
- `README.md` - Main documentation with usage guide
