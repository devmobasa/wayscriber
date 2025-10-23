# hyprmarker

> TL;DR: hyprmarker is a ZoomIt-like screen annotation tool for Wayland compositors, written in Rust.
> Works on compositors with the wlr-layer-shell protocol (Hyprland, Sway, river, …); building from source requires Rust 1.70+.
> Quick start: [set it up in four steps](#quick-start).

<details>
<summary>📹 Demo Video (Click to expand)</summary>

https://github.com/user-attachments/assets/7c4b36ec-0f6a-4aad-93fb-f9c966d43873

</details>

<details>
<summary>🖼️ Demo GIF (Click to expand)</summary>

![Demo GIF](https://github.com/user-attachments/assets/e99eb161-c603-4133-926b-79de7a8fb567)

</details>

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)

- [Why hyprmarker?](#why-hyprmarker)
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

## Why hyprmarker?

- Works across Wayland compositors (Sway, Wayfire, River, Hyprland, …) via wlr-layer-shell. Tested extensively on Hyprland. Reports from other compositors welcome.
- Built for live presentations, classroom sessions, and screenshares - toggle with a key and annotate your screen instantly without breaking flow.
- Complements tools like [Satty](https://github.com/gabm/Satty): Satty excels at capture → annotate → save workflows, while hyprmarker stays resident as an always-available drawing layer with instant mode switching.

## Quick Start

**1. Install hyprmarker**
   - Arch Linux: `yay -S hyprmarker` or `paru -S hyprmarker` (AUR). The binary lands in `/usr/bin` and required tools (`wl-clipboard`, `grim`, `slurp`) are pulled in automatically.
   - Other distros: see [Installation](#installation), then install `wl-clipboard`, `grim`, and `slurp` for the fastest screenshot workflow.

**2. Choose how to run it:**

### Option 1: One-Shot Mode (Simple)
Launch hyprmarker when you need it, exit when done:

```bash
hyprmarker --active
```

Or bind to a key in `~/.config/hypr/hyprland.conf`:
```conf
bind = SUPER, D, exec, hyprmarker --active
```

Press `F10` for help, `F11` for configurator, `Escape` to exit.

### Option 2: Daemon Mode (Background Service)
Run hyprmarker in the background and toggle it with a keybind:

**Enable the service:**
```bash
systemctl --user enable --now hyprmarker.service
```

**Add keybinding** to `~/.config/hypr/hyprland.conf`:
```conf
bind = SUPER, D, exec, pkill -SIGUSR1 hyprmarker
```

**Reload Hyprland:**
```bash
hyprctl reload
```

**Note:** If the daemon doesn't start after a reboot, see [Troubleshooting](#daemon-not-starting-after-reboot).

**Alternative:** Use Hyprland's exec-once instead of systemd:
```conf
exec-once = hyprmarker --daemon
bind = SUPER, D, exec, pkill -SIGUSR1 hyprmarker
```

## Features at a Glance

- Freehand drawing plus straight lines, rectangles, ellipses, and arrows.
- Text annotations with multi-line support, custom fonts, and adjustable size.
- Whiteboard/blackboard modes with auto pen contrast and isolated frames.
- Quick color palette and line thickness adjustments via hotkeys or scroll wheel.
- Status bar with live tool feedback and an in-app help overlay (`F10`).
- Launch the native configurator from the overlay (`F11`) to edit settings without leaving your session.
- Background daemon with tray icon and customizable TOML configuration.

## Demo

https://github.com/user-attachments/assets/7c4b36ec-0f6a-4aad-93fb-f9c966d43873

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

### Daemon Mode

Run hyprmarker in the background and toggle with a keybind.

**Enable the service:**
```bash
systemctl --user enable --now hyprmarker.service
```

**Add keybinding** to `~/.config/hypr/hyprland.conf`:
```conf
bind = SUPER, D, exec, pkill -SIGUSR1 hyprmarker
```

**Reload Hyprland:**
```bash
hyprctl reload
```

The daemon shows a system tray icon (may be in Waybar drawer). Press `Super+D` to toggle overlay, right-click tray icon for options.

**Service commands:**
```bash
systemctl --user status hyprmarker.service
systemctl --user restart hyprmarker.service
journalctl --user -u hyprmarker.service -f
```

**Note:** If the daemon doesn't start after reboot, see [Troubleshooting](#daemon-not-starting-after-reboot).

**Alternative:** Use Hyprland's exec-once instead of systemd:
```conf
exec-once = hyprmarker --daemon
bind = SUPER, D, exec, pkill -SIGUSR1 hyprmarker
```

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
| Launch configurator | `F11` |
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

### Daemon not starting after reboot

**If using systemd:** User services don't start at boot by default. Enable lingering:
```bash
loginctl enable-linger $USER
```

**Simpler alternative:** Use Hyprland's `exec-once` instead:
```conf
exec-once = hyprmarker --daemon
```

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
  - Use `./tools/fetch-all-deps.sh` to prefetch crates for the main binary and configurator before running frozen/offline builds.

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
