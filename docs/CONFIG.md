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

# Default pen thickness in pixels (1.0 - 20.0)
default_thickness = 3.0

# Default font size for text mode (8.0 - 72.0)
# Can be adjusted at runtime with Ctrl+Shift+/- or Shift+Scroll
default_font_size = 32.0
```

**Color Options:**
- **Named colors**: `"red"`, `"green"`, `"blue"`, `"yellow"`, `"orange"`, `"pink"`, `"white"`, `"black"`
- **RGB arrays**: `[255, 0, 0]` for red, `[0, 255, 0]` for green, etc.

**Runtime Adjustments:**
- **Pen thickness**: Use `+`/`-` keys or scroll wheel (range: 1-20px)
- **Font size**: Use `Ctrl+Shift+`/`-` or `Shift+Scroll` (range: 8-72px)

**Defaults:**
- Color: Red
- Thickness: 3.0px
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
```

**Defaults:**
- Length: 20.0px
- Angle: 30.0°

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
```

**Buffer Count:**
- **2**: Double buffering - minimal memory usage, may flicker on fast drawing
- **3**: Triple buffering - recommended default, smooth drawing
- **4**: Quad buffering - for high-refresh displays (144Hz+), ultra-smooth

**VSync:**
- **true** (default): Synchronizes with display refresh rate, no tearing
- **false**: Uncapped rendering, may cause tearing but lower latency

**Defaults:**
- Buffer count: 3 (triple buffering)
- VSync: true

### `[ui]` - User Interface

Controls visual indicators, overlays, and UI styling.

```toml
[ui]
# Show status bar with current color/thickness/tool
show_status_bar = true

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
font_size = 16.0
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
```

**Status Bar:**
- Shows current color, pen thickness, and active tool
- Press `F10` to toggle help overlay
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
- **Click highlight**: Enable presenter-style click halos with adjustable radius, colors, and duration; combine with the highlight-only tool toggle for presentation mode

**Defaults:**
- Show status bar: true
- Position: bottom-left
- Status bar font: 14px
- Help overlay font: 16px
- Semi-transparent dark backgrounds
- Light blue help overlay border

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
- `Ctrl+W`: Toggle whiteboard mode (press again to exit)
- `Ctrl+B`: Toggle blackboard mode (press again to exit)
- `Ctrl+Shift+T`: Return to transparent mode

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

# Filename template (chrono format specifiers allowed)
filename_template = "screenshot_%Y-%m-%d_%H%M%S"

# Image format (currently "png")
format = "png"

# Copy captures to clipboard in addition to saving files
copy_to_clipboard = true
```

**Tips:**
- Set `copy_to_clipboard = false` if you prefer file-only captures.
- Clipboard-only shortcuts ignore the save directory automatically.
- Install `wl-clipboard`, `grim`, and `slurp` for the best Wayland experience; otherwise wayscriber falls back to `xdg-desktop-portal`.

### `[session]` - Session Persistence

Optional on-disk persistence for your drawings. Disabled by default so each session starts fresh.

```toml
[session]
persist_transparent = false
persist_whiteboard = false
persist_blackboard = false
restore_tool_state = true
storage = "auto"
# custom_directory = "/absolute/path"
per_output = true
max_shapes_per_frame = 10000
max_file_size_mb = 10
compress = "auto"
auto_compress_threshold_kb = 100
backup_retention = 1
```

- `persist_*` — choose which board modes (transparent/whiteboard/blackboard) survive restarts
- `restore_tool_state` — save pen colour, thickness, font size, arrow settings, and status bar visibility
- `storage` — `auto` (XDG data dir, e.g. `~/.local/share/wayscriber`), `config` (same directory as `config.toml`), or `custom`
- `custom_directory` — absolute path used when `storage = "custom"`; supports `~`
- `per_output` — when `true` (default) keep a separate session file for each monitor; set to `false` to share one file per Wayland display as in earlier releases
- `max_shapes_per_frame` — trims older shapes if a frame grows beyond this count when loading/saving
- `max_file_size_mb` — skips loading and writing session files beyond this size cap
- `compress` — `auto` (gzip files above the threshold), `on`, or `off`
- `auto_compress_threshold_kb` — size threshold for `compress = "auto"`
- `backup_retention` — how many rotated `.bak` files to keep (set to 0 to disable backups)

> **Privacy note:** Session files are stored unencrypted. Clear the session directory or disable persistence when working with sensitive material.

Use the CLI helpers for quick maintenance:

- `wayscriber --session-info` prints the active storage path, file details, and shape counts.
- `wayscriber --clear-session` removes the session file, backup, and lock.

### `[keybindings]` - Custom Keybindings

Customize keyboard shortcuts for all actions. Each action can have multiple keybindings.

```toml
[keybindings]
# Exit overlay (or cancel current action)
exit = ["Escape", "Ctrl+Q"]

# Enter text mode
enter_text_mode = ["T"]

# Clear all annotations on current canvas
clear_canvas = ["E"]

# Undo last annotation
undo = ["Ctrl+Z"]

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

# Toggle help overlay
toggle_help = ["F10"]

# Toggle status bar visibility
toggle_status_bar = ["F12"]

# Toggle click highlight (visual mouse halo)
toggle_click_highlight = ["Ctrl+Shift+H"]

# Toggle highlight-only drawing tool
toggle_highlight_tool = ["Ctrl+Alt+H"]

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

# Help overlay (press F10 while drawing for a full reference)
```

**Keybinding Format:**

Keybindings are specified as strings with modifiers and keys separated by `+`:
- Simple keys: `"E"`, `"T"`, `"Escape"`, `"F10"`
- With modifiers: `"Ctrl+Z"`, `"Shift+T"`, `"Ctrl+Shift+W"`
- Special keys: `"Escape"`, `"Return"`, `"Backspace"`, `"Space"`, `"F10"`, `"F11"`, `"+", `-`, `=`, `_`

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
- Modifiers (`Shift`, `Ctrl`, `Alt`, `Tab`) are always captured for drawing tools
- In text input mode, configured keybindings (like `Ctrl+Q` for exit) work before keys are consumed as text
- Color keys only work when not holding `Ctrl` (to avoid conflicts with other actions)
- Invalid keybinding strings will be logged and fall back to defaults
- Duplicate keybindings across actions will be detected and reported at startup

**Defaults:**
All defaults match the original hardcoded keybindings to maintain compatibility.

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
```

## See Also

- `SETUP.md` - Installation and system requirements
- `config.example.toml` - Annotated example configuration
- `README.md` - Main documentation with usage guide
