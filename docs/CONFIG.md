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

# Default pen thickness in pixels (1.0 - 40.0)
default_thickness = 3.0

# Default eraser size in pixels (1.0 - 40.0)
default_eraser_size = 12.0

# Default eraser mode ("brush" or "stroke")
default_eraser_mode = "brush"

# Default marker opacity multiplier (0.05 - 0.90). Multiplies the current color alpha.
marker_opacity = 0.32

# Default font size for text mode (8.0 - 72.0)
# Can be adjusted at runtime with <kbd>Ctrl+Shift++</kbd>/<kbd>Ctrl+Shift+-</kbd> or <kbd>Shift</kbd> + scroll
default_font_size = 32.0
```

**Color Options:**
- **Named colors**: `"red"`, `"green"`, `"blue"`, `"yellow"`, `"orange"`, `"pink"`, `"white"`, `"black"`
- **RGB arrays**: `[255, 0, 0]` for red, `[0, 255, 0]` for green, etc.

**Runtime Adjustments:**
- **Pen thickness**: Use <kbd>+</kbd>/<kbd>-</kbd> keys or scroll wheel (range: 1-40px)
- **Eraser size**: Use <kbd>+</kbd>/<kbd>-</kbd> keys or scroll wheel when eraser tool is active (range: 1-40px)
- **Eraser mode**: Use <kbd>Ctrl+Shift+E</kbd> to toggle brush vs stroke erasing
- **Font size**: Use <kbd>Ctrl+Shift++</kbd>/<kbd>Ctrl+Shift+-</kbd> or <kbd>Shift</kbd> + scroll (range: 8-72px)

**Defaults:**
- Color: Red
- Thickness: 3.0px
- Eraser size: 12.0px
- Eraser mode: Brush
- Font size: 32.0px

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
- **false**: Uncapped rendering, may cause tearing but lower latency

**UI Animation FPS:**
- **30** (default): Smooth enough for most effects
- **0**: Unlimited (renders every frame while animations are active)
- Higher values improve smoothness at the cost of extra redraws

**Defaults:**
- Buffer count: 3 (triple buffering)
- VSync: true
- UI animation FPS: 30

### `[ui]` - User Interface

Controls visual indicators, overlays, and UI styling.

```toml
[ui]
# Show status bar with current color/thickness/tool
show_status_bar = true

# Show a small "FROZEN" badge when frozen mode is active
show_frozen_badge = true

# Filter help overlay sections based on enabled features
help_overlay_context_filter = true

# Status bar position
# Options: "top-left", "top-right", "bottom-left", "bottom-right"
status_bar_position = "bottom-left"

# Status bar styling
[ui.status_bar_style]
font_size = 14.0
padding = 10.0
bg_color = [0.0, 0.0, 0.0, 0.7]      # Semi-transparent black [R, G, B, A]
text_color = [1.0, 1.0, 1.0, 1.0]    # White
dot_radius = 4.0

# Help overlay styling
[ui.help_overlay_style]
font_size = 14.0
font_family = "Noto Sans, DejaVu Sans, Liberation Sans, Sans"
line_height = 22.0
padding = 20.0
bg_color = [0.0, 0.0, 0.0, 0.85]     # Darker background
border_color = [0.3, 0.6, 1.0, 0.9]  # Light blue
border_width = 2.0
text_color = [1.0, 1.0, 1.0, 1.0]    # White

# Click highlight styling (visual feedback for mouse clicks)
[ui.click_highlight]
enabled = false
radius = 24.0
outline_thickness = 4.0
duration_ms = 750
fill_color = [1.0, 0.8, 0.0, 0.35]
outline_color = [1.0, 0.6, 0.0, 0.9]
use_pen_color = true  # Existing highlights update immediately when you change pen color
```

**Status Bar:**
- Shows current color, pen thickness, and active tool
- Press <kbd>F10</kbd> to toggle help overlay
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

**Defaults:**
- Show status bar: true
- Position: bottom-left
- Status bar font: 14px
- Help overlay font: 16px
- Semi-transparent dark backgrounds
- Light blue help overlay border

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
# show_step_section = false
# show_text_controls = true
# show_settings_section = false
#
# [ui.toolbar.mode_overrides.regular] # Full mode overrides
# show_presets = true
# show_actions_section = true
# show_actions_advanced = false
# show_step_section = false
# show_text_controls = true
# show_settings_section = true
#
# [ui.toolbar.mode_overrides.advanced] # Legacy mode overrides
# show_presets = true
# show_actions_section = true
# show_actions_advanced = true
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
```

**Behavior:**
- **Icon/text mode**: `use_icons` switches between compact icons and labeled buttons.
- **Colors**: `show_more_colors` toggles the extended palette row.
- **Layout**: `layout_mode` picks a preset complexity level; `mode_overrides` lets you customize each mode.
- **Actions**: `show_actions_section` shows the basic actions row; `show_actions_advanced` reveals the extended actions.
- **Presets**: `show_presets` hides/shows the preset slots section.
- **Text controls**: `show_text_controls` keeps font size/family visible even when text isn’t active.
- **Step controls**: `show_step_section` hides/shows the Step Undo/Redo section.
- **Settings**: `show_settings_section` hides/shows the settings footer (config buttons and toggles).
- **Delays**: `show_delay_sliders` shows the timed undo/redo-all sliders in the side panel.
- **Marker opacity**: the marker opacity slider appears when the marker tool is active; `show_marker_opacity_section` keeps it visible even when using other tools.
- **Preset toasts**: `show_preset_toasts` enables toast confirmations for preset apply/save/clear.
- **Pinned**: `top_pinned`/`side_pinned` control whether each toolbar opens on startup.

**Defaults:** all set as above.

### `[board]` - Board Modes (Whiteboard/Blackboard)

Controls whiteboard and blackboard mode settings.

```toml
[board]
# Enable board mode features
enabled = true

# Default mode on startup
# Options: "transparent" (default overlay), "whiteboard" (light), "blackboard" (dark)
default_mode = "transparent"

# Whiteboard background color [R, G, B] (0.0-1.0 range)
# Default: off-white (253, 253, 253) for softer appearance
whiteboard_color = [0.992, 0.992, 0.992]

# Blackboard background color [R, G, B] (0.0-1.0 range)
# Default: near-black (17, 17, 17) for softer appearance
blackboard_color = [0.067, 0.067, 0.067]

# Default pen color for whiteboard mode [R, G, B] (0.0-1.0 range)
# Default: black for contrast on light background
whiteboard_pen_color = [0.0, 0.0, 0.0]

# Default pen color for blackboard mode [R, G, B] (0.0-1.0 range)
# Default: white for contrast on dark background
blackboard_pen_color = [1.0, 1.0, 1.0]

# Automatically adjust pen color when entering board modes
# Set to false if you want to keep your current color when switching modes
auto_adjust_pen = true
```

**Board Modes:**
- **Transparent**: Default overlay mode showing the screen underneath
- **Whiteboard**: Light background for drawing (like a physical whiteboard)
- **Blackboard**: Dark background for drawing (like a chalkboard)

**Keybindings:**
- <kbd>Ctrl+W</kbd>: Toggle whiteboard mode (press again to exit)
- <kbd>Ctrl+B</kbd>: Toggle blackboard mode (press again to exit)
- <kbd>Ctrl+Shift+T</kbd>: Return to transparent mode

**Frame Isolation:**
- Each mode maintains independent drawings
- Switching modes preserves all work
- Undo/clear operations affect only the current mode

**Color Themes:**

High Contrast (pure white/black):
```toml
[board]
whiteboard_color = [1.0, 1.0, 1.0]
blackboard_color = [0.0, 0.0, 0.0]
```

Chalkboard Theme (green board):
```toml
[board]
blackboard_color = [0.11, 0.18, 0.13]
blackboard_pen_color = [0.95, 0.95, 0.8]
```

Sepia Theme (vintage):
```toml
[board]
whiteboard_color = [0.96, 0.93, 0.86]
whiteboard_pen_color = [0.29, 0.23, 0.18]
```

**CLI Override:**
You can override the default mode from the command line:
```bash
wayscriber --active --mode whiteboard
wayscriber --active --mode blackboard
wayscriber --daemon --mode whiteboard
```

**Defaults:**
- Enabled: true
- Default mode: transparent
- Whiteboard: off-white background, black pen
- Blackboard: near-black background, white pen
- Auto-adjust pen: true

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

- `persist_*` — choose which board modes (transparent/whiteboard/blackboard) survive restarts
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

# Adjust font size
increase_font_size = ["Ctrl+Shift++", "Ctrl+Shift+="]
decrease_font_size = ["Ctrl+Shift+-", "Ctrl+Shift+_"]

# Board mode toggles
toggle_whiteboard = ["Ctrl+W"]
toggle_blackboard = ["Ctrl+B"]
return_to_transparent = ["Ctrl+Shift+T"]

# Page navigation
page_prev = ["Ctrl+Alt+ArrowLeft", "Ctrl+Alt+PageUp"]
page_next = ["Ctrl+Alt+ArrowRight", "Ctrl+Alt+PageDown"]
page_new = ["Ctrl+Alt+N"]
page_duplicate = ["Ctrl+Alt+D"]
page_delete = ["Ctrl+Alt+Delete"]

# Toggle help overlay
toggle_help = ["F10"]

# Toggle status bar visibility
toggle_status_bar = ["F12"]

# Toggle presenter mode
toggle_presenter_mode = ["Ctrl+Shift+K"]

# Toggle click highlight (visual mouse halo)
toggle_click_highlight = ["Ctrl+Shift+H"]

# Toggle highlight-only drawing tool
toggle_highlight_tool = ["Ctrl+Alt+H"]

# Toggle selection properties panel
toggle_selection_properties = ["Ctrl+Alt+P"]

# Toggle eraser behavior (brush vs stroke)
toggle_eraser_mode = ["Ctrl+Shift+E"]

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
capture_file_region = ["Ctrl+Shift+6"]

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
- `WAYSCRIBER_FORCE_INLINE_TOOLBARS=1` forces inline toolbars on Wayland (default: off)
- `WAYSCRIBER_TOOLBAR_DRAG_PREVIEW=0` disables inline toolbar drag preview (default: on)
- `WAYSCRIBER_TOOLBAR_POINTER_LOCK=1` enables pointer-lock drag path (experimental; default: off)
- `WAYSCRIBER_DEBUG_TOOLBAR_DRAG=1` enables toolbar drag logging (default: off)
- `WAYSCRIBER_DEBUG_DAMAGE=1` enables damage region logging (default: off)
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
[board]
default_mode = "whiteboard"
auto_adjust_pen = true

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
