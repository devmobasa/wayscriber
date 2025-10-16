# hyprmarker

> TL;DR: hyprmarker is a ZoomIt-like screen annotation tool for Wayland compositors, written in Rust.
> Works on compositors with the wlr-layer-shell protocol (Hyprland, Sway, river, …); building from source requires Rust 1.70+.
> Quick start: [set it up in four steps](#quick-start).

![Demo](demo.gif)

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)

- [Quick Start](#quick-start)
- [Features at a Glance](#features-at-a-glance)
- [Demo](#demo)
- [Installation](#installation)
- [Running hyprmarker](#running-hyprmarker)
- [Controls Reference](#controls-reference)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)
- [Additional Information](#additional-information)
- [Contributing & Credits](#contributing--credits)

## Quick Start

1. Install hyprmarker.  
   - Arch Linux: `yay -S hyprmarker` or `paru -S hyprmarker` (AUR). Screenshot tools (`wl-clipboard`, `grim`, `slurp`) are installed automatically.  
   - Other distros: see [Installation](#installation) for dependencies and source builds, then install `wl-clipboard`, `grim`, and `slurp` for full screenshot support.
2. Start the background service so the overlay is available instantly:
   ```bash
   systemctl --user enable --now hyprmarker.service
   ```
3. Add a keybinding to toggle the overlay in `~/.config/hypr/hyprland.conf`:
   ```conf
   bind = SUPER, D, exec, pkill -SIGUSR1 hyprmarker
   ```
4. Reload Hyprland (`hyprctl reload`), then press `Super+D` to draw. `F10` shows the help overlay; `Escape` hides it.

Need an alternative launch method? Jump to [Hyprland exec-once](#hyprland-exec-once) or the rest of [Running hyprmarker](#running-hyprmarker).

## Features at a Glance

- Freehand drawing plus straight lines, rectangles, ellipses, and arrows.
- Text annotations with multi-line support, custom fonts, and adjustable size.
- Whiteboard/blackboard modes with auto pen contrast and isolated frames.
- Quick color palette and line thickness adjustments via hotkeys or scroll wheel.
- Status bar with live tool feedback and an in-app help overlay (`F10`).
- Background daemon with tray icon and customizable TOML configuration.

## Demo

![Demo](demo.gif)

## Installation

See **[docs/SETUP.md](docs/SETUP.md)** for detailed walkthroughs.

### Arch Linux (AUR)

```bash
# Using yay
yay -S hyprmarker

# Or using paru
paru -S hyprmarker
```

The package installs the user service at `/usr/lib/systemd/user/hyprmarker.service`.

### Other Distros

**Install dependencies:**

```bash
# Ubuntu / Debian
sudo apt-get install libcairo2-dev libwayland-dev libpango1.0-dev

# Fedora
sudo dnf install cairo-devel wayland-devel pango-devel
```

Optional but recommended for screenshots:
```bash
sudo apt-get install wl-clipboard grim slurp   # Debian/Ubuntu
sudo dnf install wl-clipboard grim slurp       # Fedora
```

**Build from source:**

```bash
git clone https://github.com/devmobasa/hyprmarker.git
cd hyprmarker
cargo build --release
```

The binary will be at `target/release/hyprmarker`.

### Manual Install Script

```bash
cargo build --release
./tools/install.sh
```

The installer places the binary at `~/.local/bin/hyprmarker`, creates `~/.config/hyprmarker/`, and offers to configure Hyprland.

## Running hyprmarker

### Background Daemon (systemd)

Run hyprmarker as a persistent background service that listens for your keybinding.

1. Ensure the unit file is available:  
   - AUR package: `/usr/lib/systemd/user/hyprmarker.service` is already installed.  
   - Manual build: copy `packaging/hyprmarker.service` to `~/.config/systemd/user/` or run `./tools/install.sh`.
2. Enable and start the service:
   ```bash
   systemctl --user enable --now hyprmarker.service
   ```
3. Add a Hyprland keybinding:
   ```conf
   bind = SUPER, D, exec, pkill -SIGUSR1 hyprmarker
   ```
4. Reload Hyprland: `hyprctl reload`.

While the daemon runs, a system tray icon appears (it may live in your Waybar drawer). Press `Super+D` to summon the overlay, draw with the mouse, then `Ctrl+Q` or `Escape` to hide it. Right-click the tray icon for toggle/quit actions.

```bash
# Handy service commands
systemctl --user restart hyprmarker.service
systemctl --user stop hyprmarker.service
journalctl --user -u hyprmarker.service -f
```

### Hyprland exec-once

Prefer to manage the daemon yourself? Add this to `~/.config/hypr/hyprland.conf`:

```conf
exec-once = hyprmarker --daemon
bind = SUPER, D, exec, pkill -SIGUSR1 hyprmarker
```

Reload with `hyprctl reload`.

### One-Shot Mode

Launch directly into an active overlay without the daemon:

```bash
hyprmarker --active
hyprmarker --active --mode whiteboard
hyprmarker --active --mode blackboard
```

Bind it to keys if you prefer:

```conf
bind = $mainMod, D, exec, hyprmarker --active
bind = $mainMod SHIFT, D, exec, hyprmarker --active --mode whiteboard
```

Exit the overlay with `Escape` or `Ctrl+Q`.

### Screenshot Shortcuts

hyprmarker ships with keyboard shortcuts for quick captures:

- `Ctrl+C` – copy the entire screen to the clipboard.
- `Ctrl+S` – save the entire screen as a PNG (uses your capture directory).
- `Ctrl+Shift+C` – select a region and copy it to the clipboard.
- `Ctrl+Shift+S` – select a region and save it as a PNG.
- `Ctrl+Shift+O` – capture the active window (Hyprland fast path, portal fallback).
- `Ctrl+6` / `Ctrl+Shift+6` – reserved for remembered-region clipboard/file captures (coming soon).

**Requirements:** install `wl-clipboard`, `grim`, and `slurp` for the fastest Hyprland workflow. If they are missing, hyprmarker falls back to `xdg-desktop-portal`'s interactive picker.

## Controls Reference

Press `F10` at any time for the in-app keyboard and mouse cheat sheet.

| Action | Key/Mouse |
|--------|-----------|
| **Drawing Tools** |
| Freehand pen | Default (drag with left mouse button) |
| Straight line | Hold `Shift` + drag |
| Rectangle | Hold `Ctrl` + drag |
| Ellipse/Circle | Hold `Tab` + drag |
| Arrow | Hold `Ctrl+Shift` + drag |
| Text mode | Press `T`, click to position, type, `Shift+Enter` for new line, `Enter` to finish |
| **Board Modes** |
| Toggle Whiteboard | `Ctrl+W` (press again to exit) |
| Toggle Blackboard | `Ctrl+B` (press again to exit) |
| Return to Transparent | `Ctrl+Shift+T` |
| **Colors** |
| Red | `R` |
| Green | `G` |
| Blue | `B` |
| Yellow | `Y` |
| Orange | `O` |
| Pink | `P` |
| White | `W` |
| Black | `K` |
| **Line Thickness** |
| Increase | `+`, `=`, or scroll down |
| Decrease | `-`, `_`, or scroll up |
| **Font Size** |
| Increase | `Ctrl+Shift++` or `Shift` + scroll down |
| Decrease | `Ctrl+Shift+-` or `Shift` + scroll up |
| **Editing** |
| Undo last shape | `Ctrl+Z` |
| Clear all | `E` |
| Cancel action | Right-click or `Escape` |
| **Help & Exit** |
| Toggle help overlay | `F10` |
| Exit overlay | `Escape` or `Ctrl+Q` |

## Configuration

- Config file location: `~/.config/hyprmarker/config.toml`.
- Copy defaults to get started:

  ```bash
  mkdir -p ~/.config/hyprmarker
  cp config.example.toml ~/.config/hyprmarker/config.toml
  ```

- Key sections to tweak:
  - `[drawing]` – default color, thickness, and font settings.
  - `[performance]` – buffer count and VSync.
  - `[ui]` – status bar visibility and position.
  - `[board]` – whiteboard/blackboard presets and auto-adjust options.

Example snippet:

```toml
[drawing]
default_color = "red"
default_thickness = 3.0

[performance]
buffer_count = 3
enable_vsync = true
```

See **[docs/CONFIG.md](docs/CONFIG.md)** for the full configuration reference.

## Troubleshooting

### Service won't start

- Check status: `systemctl --user status hyprmarker.service`
- Tail logs: `journalctl --user -u hyprmarker.service -f`
- Restart: `systemctl --user restart hyprmarker.service`

### Overlay not appearing

1. Verify Wayland session: `echo $WAYLAND_DISPLAY`
2. Ensure your compositor supports `wlr-layer-shell` (Hyprland, Sway, river, etc.)
3. Run with logs for clues: `RUST_LOG=info hyprmarker --active`

### Config issues

- Confirm the file exists: `ls -la ~/.config/hyprmarker/config.toml`
- Watch for TOML errors in logs: `RUST_LOG=info hyprmarker --active`

### Performance

Tune `[performance]` in `config.toml` if memory or latency is a concern:

```toml
[performance]
buffer_count = 2
enable_vsync = true
```

## Additional Information

### Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Wayland (Hyprland, Sway, etc.) | ✅ **SUPPORTED** | Requires wlr-layer-shell protocol |

### Performance Characteristics

- Triple-buffered rendering prevents flicker during fast drawing.
- Frame-synchronized updates (VSync) keep strokes smooth.
- Dirty-region updates minimize CPU/GPU overhead.
- Tested to sustain 60 FPS on 1080p–4K displays.

### Architecture Overview

```
hyprmarker/
├── src/
│   ├── main.rs           # Entry point, CLI parsing
│   ├── daemon.rs         # Daemon mode with signal handling
│   ├── ui.rs             # Status bar and help overlay rendering
│   ├── util.rs           # Utility functions
│   ├── backend/
│   │   ├── mod.rs        # Backend module
│   │   └── wayland.rs    # Wayland wlr-layer-shell implementation
│   ├── config/
│   │   ├── mod.rs        # Configuration loader and validator
│   │   ├── types.rs      # Config structure definitions
│   │   └── enums.rs      # Color specs and enums
│   ├── draw/
│   │   ├── mod.rs        # Drawing module
│   │   ├── color.rs      # Color definitions and constants
│   │   ├── font.rs       # Font descriptor for Pango
│   │   ├── frame.rs      # Frame container for shapes
│   │   ├── shape.rs      # Shape definitions (lines, text, etc.)
│   │   └── render.rs     # Cairo/Pango rendering functions
│   └── input/
│       ├── mod.rs        # Input handling module
│       ├── state.rs      # Drawing state machine
│       ├── events.rs     # Keyboard/mouse event types
│       ├── modifiers.rs  # Modifier key tracking
│       └── tool.rs       # Drawing tool enum
├── tools/                # Helper scripts (install, run, reload)
├── packaging/            # Distribution files (service, PKGBUILD)
├── docs/                 # Documentation
└── config.example.toml   # Example configuration
```

### Documentation

- **[docs/SETUP.md](docs/SETUP.md)** – system setup and installation details
- **[docs/CONFIG.md](docs/CONFIG.md)** – configuration reference

### Comparison with ZoomIt

| Feature | ZoomIt (Windows) | hyprmarker (Linux) |
|---------|------------------|--------------------|
| Freehand drawing | ✅ | ✅ |
| Straight lines | ✅ | ✅ |
| Rectangles | ✅ | ✅ |
| Ellipses | ✅ | ✅ |
| Arrows | ✅ | ✅ |
| Text annotations | ✅ | ✅ |
| **Whiteboard mode** | ✅ (W key) | ✅ (`Ctrl+W`) |
| **Blackboard mode** | ✅ (K key) | ✅ (`Ctrl+B`) |
| Multi-line text | ❌ | ✅ (`Shift+Enter`) |
| Custom fonts | ❌ | ✅ (Pango) |
| Color selection | ✅ | ✅ (8 colors) |
| Undo | ✅ | ✅ |
| Clear all | ✅ | ✅ |
| Help overlay | ❌ | ✅ |
| Status bar | ❌ | ✅ |
| Configuration file | ❌ | ✅ |
| Scroll wheel thickness | ❌ | ✅ |
| Zoom functionality | ✅ | ❌ (not planned) |
| Break timer | ✅ | ❌ (not planned) |
| Screen recording | ✅ | ❌ (not planned) |

### Roadmap

- [x] Native Wayland wlr-layer-shell implementation
- [x] Configuration file support
- [x] Status bar and help overlay
- [x] Scroll wheel thickness adjustment
- [x] Daemon mode with global hotkey toggle (Super+D)
- [x] System tray integration
- [x] Autostart with systemd user service
- [x] Multi-line text support (Shift+Enter)
- [x] Custom fonts with Pango rendering
- [x] Whiteboard/blackboard modes with isolated frames
- [x] Board mode configuration (colors, auto-adjust)
- [x] CLI `--mode` flag for initial board selection
- [ ] Multi-monitor support with per-monitor surfaces
- [ ] Additional shapes (filled shapes, highlighter)
- [ ] Save annotations to image file
- [ ] Eraser tool
- [ ] Color picker

### License

MIT License — see [LICENSE](LICENSE) for details.

## Contributing & Credits

- Pull requests and bug reports are welcome. Priority areas include compositor compatibility testing, multi-monitor support, and new drawing tools.
- Development basics:

  ```bash
  cargo build
  cargo run -- --active
  cargo test
  cargo clippy
  cargo fmt
  ```

- Acknowledgments:
  - Inspired by [ZoomIt](https://learn.microsoft.com/en-us/sysinternals/downloads/zoomit) by [Mark Russinovich](https://github.com/markrussinovich)
  - Built for [Hyprland](https://hyprland.org/) by [vaxry](https://github.com/vaxerski)
  - Similar ideas from [Gromit-MPX](https://github.com/bk138/gromit-mpx)
  - Development approach inspired by [DHH](https://dhh.dk/)'s [Omarchy](https://omarchy.org)
  - Uses [Cairo](https://www.cairographics.org/) and [smithay-client-toolkit](https://github.com/Smithay/client-toolkit)
- This tool was developed with AI assistance:
  - Initial concept & planning: ChatGPT
  - Architecture review & design: Codex
  - Implementation: Claude Code (Anthropic)

Created as a native Wayland implementation of ZoomIt-style annotation features for Linux desktops.
