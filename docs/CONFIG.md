# Configuration Guide

## Overview

wayscriber supports customization through a TOML configuration file located at:
```
~/.config/wayscriber/config.toml
```

All settings are optional. If the configuration file doesn't exist or settings are missing, sensible defaults will be used.

## Configuration File Location

The configuration file should be placed at:
- Linux: `~/.config/wayscriber/config.toml`
- The directory will be created automatically when you first create the config file

## Example Configuration

See `config.example.toml` in the repository root for a complete example with documentation.

## Configuration Sections

### `[drawing]` - Drawing Defaults

Controls the default appearance of annotations.

```toml
[drawing]
# Default pen color
# Options: "red", "green", "blue", "yellow", "orange", "pink", "white", "black"
# Or RGB array: [255, 0, 0]
default_color = "red"

# Default pen thickness in pixels (1.0 - 50.0)
default_thickness = 3.0

# Default eraser size in pixels (1.0 - 50.0)
default_eraser_size = 12.0

# Default eraser mode ("brush" or "stroke")
default_eraser_mode = "brush"

# Default marker opacity multiplier (0.05 - 0.90). Multiplies the current color alpha.
marker_opacity = 0.32

# Default fill state for rectangle/ellipse tools
default_fill_enabled = false

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
```

**Color Options:**
- **Named colors**: `"red"`, `"green"`, `"blue"`, `"yellow"`, `"orange"`, `"pink"`, `"white"`, `"black"`
- **RGB arrays**: `[255, 0, 0]` for red, `[0, 255, 0]` for green, etc.

**Runtime Adjustments:**
- **Pen thickness**: Use <kbd>+</kbd>/<kbd>-</kbd> keys or scroll wheel (range: 1-50px)
- **Eraser size**: Use <kbd>+</kbd>/<kbd>-</kbd> keys or scroll wheel when eraser tool is active (range: 1-50px)
- **Eraser mode**: Use <kbd>Ctrl+Shift+E</kbd> to toggle brush vs stroke erasing
- **Marker opacity**: Use <kbd>Ctrl+Alt</kbd> + <kbd>↑</kbd>/<kbd>↓</kbd>
- **Font size**: Use <kbd>Ctrl+Shift++</kbd>/<kbd>Ctrl+Shift+-</kbd> or <kbd>Shift</kbd> + scroll (range: 8-72px)

**Defaults:**
- Color: Red
- Thickness: 3.0px
- Eraser size: 12.0px
- Eraser mode: Brush
- Marker opacity: 0.32
- Fill enabled: false
- Font size: 32.0px
- Font family/weight/style: Sans / bold / normal
- Text background: false
- Hit-test tolerance: 6.0px (linear threshold: 400)
- Undo stack limit: 100

### `[arrow]` - Arrow Geometry

Controls the appearance of arrow annotations.

```toml
[arrow]
# Arrowhead length in pixels
length = 20.0

# Arrowhead angle in degrees (15-60)
# 30 degrees gives a nice balanced arrow
angle_degrees = 30.0

# Place the arrowhead at the end of the line instead of the start
head_at_end = false
```

**Defaults:**
- Length: 20.0px
- Angle: 30.0°
- Head at end: false (head at the start)

### `[presets]` - Quick Tool Slots

Configure 3-5 tool presets that you can apply or update via hotkeys or the toolbar strip.

```toml
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
arrow_head_at_end = false
show_status_bar = true
```

**Required fields:** `tool`, `color`, `size`  
**Optional fields:** `eraser_kind`, `eraser_mode`, `marker_opacity`, `fill_enabled`, `font_size`, `text_background_enabled`, `arrow_length`, `arrow_angle`, `arrow_head_at_end`, `show_status_bar`

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
- `custom_section_enabled` reveals the Step controls in the side toolbar; those controls use the custom delays and step counts above.

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
# Prevents tearing and limits rendering to display refresh rate
enable_vsync = true

# Max FPS when VSync is disabled (0 = unlimited)
# Prevents CPU spinning at very high FPS; set to match your monitor
max_fps_no_vsync = 60

# UI animation frame rate (0 = unlimited)
# Higher values smooth UI effects at the cost of more redraws
ui_animation_fps = 30
```

**Buffer Count:**
- **2**: Double buffering - minimal memory usage, may flicker on fast drawing
- **3**: Triple buffering - recommended default, smooth drawing
- **4**: Quad buffering - for high-refresh displays (144Hz+), ultra-smooth

**VSync:**
- **true** (default): Synchronizes with display refresh rate, no tearing
- **false**: Capped by `max_fps_no_vsync` (set to 0 for uncapped); may cause tearing but lower latency

**Max FPS (VSync off):**
- **60** (default): Suitable for most displays
- **0**: Unlimited (uncapped; higher CPU usage)
- Set to your monitor refresh (60/120/144/240) for best balance

**UI Animation FPS:**
- **30** (default): Smooth enough for most effects
- **0**: Unlimited (renders every frame while animations are active)
- Higher values improve smoothness at the cost of extra redraws

**Defaults:**
- Buffer count: 3 (triple buffering)
- VSync: true
- Max FPS (VSync off): 60
- UI animation FPS: 30

### `[ui]` - User Interface

Controls visual indicators, overlays, and UI styling.

```toml
[ui]
# Show status bar with current color/thickness/tool
show_status_bar = true

# Show board label in the status bar
show_status_board_badge = true

# Show page counter in the status bar
show_status_page_badge = true

# Show the board/page badge even when the status bar is visible
show_page_badge_with_status_bar = false

# Show a small "FROZEN" badge when frozen mode is active
show_frozen_badge = false

# Filter help overlay sections based on enabled features
help_overlay_context_filter = true

# Status bar position
# Options: "top-left", "top-right", "bottom-left", "bottom-right"
status_bar_position = "bottom-left"

# Preferred output name for GNOME fallback (xdg-shell) overlays
#preferred_output = "eDP-1"

# Request fullscreen for the GNOME fallback overlay (disable if opaque)
#xdg_fullscreen = false

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

**UI Styling:**
- **Font sizes**: Customize text size for status bar and help overlay
- **Colors**: All RGBA values (0.0-1.0 range) with transparency control
- **Layout**: Padding, line height, dot size, border width all configurable
- **Click highlight**: Enable presenter-style click halos with adjustable radius, colors, and duration; by default the halo follows your current pen color (set `use_pen_color = false` to keep a fixed color)
- **Highlight tool ring**: `show_on_highlight_tool = true` keeps a persistent halo visible while the highlight tool is active
- **Context menu**: `ui.context_menu.enabled` toggles right-click / keyboard menus
- **GNOME fallback**: `preferred_output` pins the xdg-shell overlay to a specific monitor; `xdg_fullscreen` requests fullscreen instead of maximized

**Defaults:**
- Show status bar: true
- Show frozen badge: false
- Position: bottom-left
- Status bar font: 21px
- Help overlay font: 14px
- Semi-transparent dark backgrounds with muted borders

### `[presenter_mode]` - Presenter Mode

Control which UI elements presenter mode hides and how tools behave when it is active.

```toml
[presenter_mode]
hide_status_bar = true
hide_toolbars = true
hide_tool_preview = true
close_help_overlay = true
enable_click_highlight = true
tool_behavior = "force-highlight"
show_toast = true
```

**Tool behavior options:**
- `"keep"`: Leave the active tool unchanged
- `"force-highlight"`: Switch to highlight on entry, allow tool changes
- `"force-highlight-locked"`: Switch to highlight and lock tools while presenting

### `[ui.toolbar]` - Floating Toolbars

Controls the top and side toolbars (toggle with <kbd>F2</kbd>/<kbd>F9</kbd>).

```toml
[ui.toolbar]
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
# show_settings_section = false
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
# show_settings_section = true
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
# show_settings_section = true

# Show top toolbar on startup (pinned)
top_pinned = true

# Show side toolbar on startup (pinned)
side_pinned = true

# Use icons instead of text labels in toolbars
use_icons = true

# Show extended color palette in the top toolbar
show_more_colors = false

# Show basic actions (undo/redo/clear) in the side toolbar
show_actions_section = true

# Show advanced actions (undo all, delay, freeze, etc.)
show_actions_advanced = false

# Show zoom actions (zoom in/out/reset/lock)
show_zoom_actions = true

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

# Show Settings section (config shortcuts + advanced toggles)
show_settings_section = true

# Show delayed undo/redo sliders in the side toolbar
show_delay_sliders = false

# Show the marker opacity slider at the bottom of the side toolbar even when the marker tool isn't selected
show_marker_opacity_section = false

# Show preset action toast notifications on apply/save/clear
show_preset_toasts = true

# Show cursor tool preview bubble
show_tool_preview = false

# Initial toolbar offsets (layer-shell/inline)
top_offset = 0.0
top_offset_y = 0.0
side_offset = 0.0
side_offset_x = 0.0

# Force inline toolbars even when layer-shell is available
force_inline = false
```

**Behavior:**
- **Icon/text mode**: `use_icons` switches between compact icons and labeled buttons.
- **Colors**: `show_more_colors` toggles the extended palette row.
- **Layout**: `layout_mode` picks a preset complexity level; `mode_overrides` lets you customize each mode.
- **Actions**: `show_actions_section` shows the basic actions row; `show_actions_advanced` reveals the extended actions.
- **Zoom actions**: `show_zoom_actions` toggles the zoom controls in the Canvas drawer.
- **Pages**: `show_pages_section` toggles the page navigation block.
- **Boards**: `show_boards_section` toggles the board navigation block.
- **Presets**: `show_presets` hides/shows the preset slots section.
- **Text controls**: `show_text_controls` keeps font size/family visible even when text isn’t active.
- **Step controls**: `show_step_section` hides/shows the Step Undo/Redo section.
- **Settings**: `show_settings_section` hides/shows the settings footer (config buttons and toggles).
- **Delays**: `show_delay_sliders` shows the timed undo/redo-all sliders in the side panel.
- **Marker opacity**: the marker opacity slider appears when the marker tool is active; `show_marker_opacity_section` keeps it visible even when using other tools.
- **Preset toasts**: `show_preset_toasts` enables toast confirmations for preset apply/save/clear.
- **Tool preview**: `show_tool_preview` toggles the cursor bubble.
- **Offsets**: `top_offset`, `top_offset_y`, `side_offset`, `side_offset_x` store toolbar positions.
- **Force inline**: `force_inline` (or `WAYSCRIBER_FORCE_INLINE_TOOLBARS`) skips layer-shell toolbars.
- **Pinned**: `top_pinned`/`side_pinned` control whether each toolbar opens on startup.

**Defaults:** all set as above.

### `[boards]` - Boards (Backgrounds + Names)

Configure multiple boards (each with its own pages) plus the special transparent overlay.

```toml
[boards]
max_count = 9
auto_create = true
show_board_badge = true
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
default_pen_color = { rgb = [0.0, 0.0, 0.0] }
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
- `persist_customizations` — runtime edits (rename/background) are written back to config.
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
- Inline edits persist immediately when `persist_customizations = true`.

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
- Install `wl-clipboard`, `grim`, and `slurp` for the best Wayland experience; otherwise wayscriber falls back to `xdg-desktop-portal`.
- Use `--exit-after-capture` / `--no-exit-after-capture` to override exit behavior per run.

### `[tablet]` - Tablet/Stylus Input

Runtime toggles for tablet/stylus input (Wayland `zwp_tablet_v2`).

```toml
[tablet]
enabled = false
pressure_enabled = true
min_thickness = 1.0
max_thickness = 8.0
```

**Notes:**
- Requires the `tablet-input` feature at build time (enabled in default release builds).
- Set `enabled = true` to activate tablet input at runtime.

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
max_file_size_mb = 10
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
- `max_file_size_mb` — skips loading and writing session files beyond this size cap
- `compress` — `auto` (gzip files above the threshold), `on`, or `off`
- `auto_compress_threshold_kb` — size threshold for `compress = "auto"`
- `backup_retention` — how many rotated `.bak` files to keep (set to 0 to disable backups)
- `max_persisted_undo_depth` — optional cap for serialized history; default follows the runtime undo limit (set `persist_history = false` to skip history entirely)

> **Privacy note:** Session files are stored unencrypted. Clear the session directory or disable persistence when working with sensitive material.

Use the CLI helpers for quick maintenance:

- `wayscriber --session-info` prints the active storage path, file details, and shape counts.
- `wayscriber --clear-session` removes the session file, backup, and lock.

Session overrides and recovery:

- CLI flags: `--resume-session` forces persistence on, `--no-resume-session` forces it off for the current run. The environment variable `WAYSCRIBER_RESUME_SESSION=1/0` does the same.
- Recovery: if a session file is corrupt or cannot be parsed/decompressed, wayscriber logs a warning, writes a `.bak` copy of the bad file, removes the corrupt file, and continues with defaults. Overrides above still apply after recovery.

### `[keybindings]` - Custom Keybindings

Customize keyboard shortcuts for all actions. Each action can have multiple keybindings.

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
select_line_tool = []
select_rect_tool = []
select_ellipse_tool = []
select_arrow_tool = []
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

# Toggle status bar visibility
toggle_status_bar = ["F12", "F4"]

# Toggle toolbars
toggle_toolbar = ["F2", "F9"]

# Toggle presenter mode
toggle_presenter_mode = ["Ctrl+Shift+M"]

# Toggle click highlight (visual mouse halo)
toggle_click_highlight = ["Ctrl+Shift+H"]

# Toggle fill for rectangle/ellipse
toggle_fill = []

# Toggle selection properties panel
toggle_selection_properties = ["Ctrl+Alt+P"]

# Toggle context menu (keyboard alternative to right-click)
open_context_menu = ["Shift+F10", "Menu"]

# Launch the desktop configurator (requires wayscriber-configurator)
open_configurator = ["F11"]

# Color selection shortcuts
set_color_red = ["R"]
set_color_green = ["G"]
set_color_blue = ["B"]
set_color_yellow = ["Y"]
set_color_orange = ["O"]
set_color_pink = ["P"]
set_color_white = ["W"]
set_color_black = ["K"]

# Screenshot shortcuts
capture_full_screen = ["Ctrl+Shift+P"]
capture_active_window = ["Ctrl+Shift+O"]
capture_selection = ["Ctrl+Shift+I"]

# Clipboard/File specific captures
capture_clipboard_full = ["Ctrl+C"]
capture_file_full = ["Ctrl+S"]
capture_clipboard_selection = ["Ctrl+Shift+C"]
capture_file_selection = ["Ctrl+Shift+S"]
capture_clipboard_region = ["Ctrl+6"]
capture_file_region = ["Ctrl+Alt+6"]

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
The system will detect and report duplicate keybindings at startup. If two actions share the same key combination, the application will log an error and use default keybindings.

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
- Invalid keybinding strings will be logged and fall back to defaults
- Duplicate keybindings across actions will be detected and reported at startup

**Defaults:**
Defaults match the original hardcoded keybindings where possible. Copy/paste selection uses
<kbd>Ctrl+Alt+C</kbd>/<kbd>Ctrl+Alt+V</kbd>, so the clipboard-selection capture shortcut
defaults to <kbd>Ctrl+Shift+C</kbd> to avoid conflicts.

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
3. Runtime changes via keybindings (temporary, not saved)

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
- `WAYSCRIBER_FORCE_INLINE_TOOLBARS=1` forces inline toolbars on Wayland (default: off)
- `WAYSCRIBER_TOOLBAR_DRAG_PREVIEW=0` disables inline toolbar drag preview (default: on)
- `WAYSCRIBER_TOOLBAR_POINTER_LOCK=1` enables pointer-lock drag path (experimental; default: off)
- `WAYSCRIBER_DEBUG_TOOLBAR_DRAG=1` enables toolbar drag logging (default: off)
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
enable_vsync = true
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
enable_vsync = true
ui_animation_fps = 120
```

## See Also

- `SETUP.md` - Installation and system requirements
- `config.example.toml` - Annotated example configuration
- `README.md` - Main documentation with usage guide
