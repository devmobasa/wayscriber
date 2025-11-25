# wayscriber

> TL;DR: wayscriber is a ZoomIt-like screen real-time annotation tool for Linux, written in Rust.
> Building from source requires Rust 1.85+.
> Quick start: [set it up in four steps](#quick-start).

## Gallery

![Demo Poster](https://wayscriber.com/demo-poster-4.png)

![Demo Poster](https://wayscriber.com/demo-poster-2.png)

## Demo

<video controls width="720" poster="https://wayscriber.com/demo-poster-4.png">
  <source src="https://wayscriber.com/demo.mp4" type="video/mp4">
  Your browser does not support the video tag. Watch here: https://wayscriber.com/demo.mp4
</video>

![Demo GIF](https://wayscriber.com/demo.gif)

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)

- [Why wayscriber?](#why-wayscriber)
- [Quick Start](#quick-start)
- [Features at a Glance](#features-at-a-glance)
- [Demo](#demo)
- [Installation](#installation)
- [Running wayscriber](#running-wayscriber)
- [Controls Reference](#controls-reference)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)
- [Additional Information](#additional-information)
- [Project History](#project-history)
- [Contributing & Credits](#contributing--credits)

## Why wayscriber?
- Annotate live over any app/window on any monitor; the overlay floats above without rearranging your workspace.
- Use shapes, arrows, and text to explain steps, give demos, or build quick guides.
- Redact/hide parts of the screen and grab screenshots (region, active window, fullscreen) in one keypress.
- Toggle instantly from a lightweight background daemon when you need it.
- Persist per-monitor/per-board canvases and tool state so your setup comes back after restarts.
- Presenter helpers: click highlights and “freeze screen” to pause what viewers see while apps keep running.
- Works on wlr-layer-shell compositors (Plasma KDE/KWin, Hyprland, Sway, Wayfire, River, …) with an xdg fallback for GNOME.
- Real-hardware tests where it works:
  - Ubuntu 25.10 GNOME (xdg fallback)
  - Fedora 43 KDE (Plasma, layer-shell)
  - Fedora 43 GNOME (xdg fallback)
  - Debian 13.2 KDE (Plasma, layer-shell)
  - Debian 13.2 GNOME (xdg fallback)
  - CachyOS 2025-August KDE (Plasma, layer-shell)
  - Hyprland on Arch (layer-shell)
  - Niri on Arch (layer-shell)
- Complements tools like [Satty](https://github.com/gabm/Satty): Satty excels at capture → annotate → save workflows, while wayscriber stays resident as an always-available drawing layer with instant mode switching.

## Quick Start

**1. Install wayscriber**
1. Debian/Ubuntu (latest release):
   - Download & install:  
     ```bash
     wget -O wayscriber-amd64.deb https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-amd64.deb
     sudo apt install ./wayscriber-amd64.deb
     ```
     _Note: apt may print “Download is performed unsandboxed as root …” when installing a local file; this is expected._
2. Fedora/RHEL (latest release):
   - Download & install:  
     ```bash
     wget -O wayscriber-x86_64.rpm https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-x86_64.rpm
     sudo rpm -Uvh wayscriber-x86_64.rpm
     ```
3. Arch Linux (AUR):
	- `yay -S wayscriber` (source) or `paru -S wayscriber`
	- `yay -S wayscriber-bin` (prebuilt) or `paru -S wayscriber-bin`
4. Other distros: see [Installation](#installation), then install `wl-clipboard`, `grim`, and `slurp` for the fastest screenshot workflow.

**2. Choose how to run it:**

### Option 1: One-Shot Mode (Simple)
Launch wayscriber when you need it, exit when done:

```bash
wayscriber --active
```

Or bind to a key in `~/.config/hypr/hyprland.conf`:
```conf
bind = SUPER, D, exec, wayscriber --active
```

Press `F10` for help, `F11` for configurator, `Escape`/`Ctrl+Q` to exit, and `F12` to toggle the status bar. Use `Ctrl+Shift+F` to freeze/unfreeze the screen while keeping your drawings, or start already frozen with `wayscriber --freeze`.

### Option 2: Daemon Mode (Background Service)
Run wayscriber in the background and toggle it with a keybind:

**Enable the service:**
```bash
systemctl --user enable --now wayscriber.service
```

**Add keybinding** to `~/.config/hypr/hyprland.conf`:
```conf
bind = SUPER, D, exec, pkill -SIGUSR1 wayscriber
```

**Reload Hyprland:**
```bash
hyprctl reload
```

**Note:** If the daemon doesn't start after a reboot, see [Troubleshooting](#daemon-not-starting-after-reboot).

**Alternative:** Use Hyprland's exec-once instead of systemd:
```conf
exec-once = wayscriber --daemon
bind = SUPER, D, exec, pkill -SIGUSR1 wayscriber
```

## Features at a Glance

- **Drawing & editing**: Freehand pen, translucent highlighter marker, straight lines, rectangles, ellipses, arrows, and multiline text with smoothing; undo & redo; quick line-width and color changes via hotkeys or scroll.
- **Board modes**: Whiteboard, blackboard, and transparent overlays, each with isolated frames and auto pen contrast; snap back to transparent with `Ctrl+Shift+T`.
- **Floating toolbars**: Pin/unpin top and side toolbars (`F2`/`F9`) with icon or text display modes, extended color palettes, undo/redo actions, and optional delay sliders; hover for tooltips even when frozen.
- **Capture shortcuts**: Full-screen saves, active-window grabs, and region capture to file or clipboard using `grim`, `slurp`, and `wl-clipboard` when available.
- **Session persistence**: Opt-in per board/monitor storage that restores your canvas plus pen color & thickness; inspect with `wayscriber --session-info` or clear with `wayscriber --clear-session`.
- **Freeze / pause view**: Toggle freeze to pause what viewers see while your apps keep running; hotkey `Ctrl+Shift+F` or start frozen with `wayscriber --freeze`.
- **Workflow helpers**: Background daemon with SIGUSR1 toggle, tray icon, one-shot mode, live status bar, and in-app help overlay (`F10`).
- **Context menus & selection**: Right-click or press `Shift+F10` for per-shape actions (delete, duplicate, layer order, lock/unlock, properties) with keyboard navigation and selection halos for clarity.
- **Click highlights**: Presenter-style halo on mouse clicks with configurable colors, radius, and duration; follows your pen color by default, toggle the effect with `Ctrl+Shift+H` or swap to highlight-only mode with `Ctrl+Alt+H`.
- **Configurator & CLI**: Launch `wayscriber-configurator` (or press `F11`) to tweak colors, bindings, persistence, compression, and more; power users can edit the TOML or use CLI switches.
- **Performance & reliability**: Dirty-region rendering keeps redraws fast, while session files use atomic writes, size limits, compression, and backups for safety.

### Session Persistence

Wayscriber can remember your boards between runs (per monitor and per board color) along with pen color/thickness. Persistence is opt-in. Toggle it from the configurator (`F11 → Session` tab) or launch the GUI directly:

```bash
wayscriber-configurator
```

Prefer text? Edit `~/.config/wayscriber/config.toml`. Helpful commands:

```bash
wayscriber --session-info     # Inspect saved sessions
wayscriber --clear-session    # Remove stored boards
```

Grab a walk-through in `docs/CONFIG.md` if you want to edit the TOML by hand.
See the `[ui.toolbar]` section there to pin toolbars, switch icon/text mode, and show/hide colors/actions/delay sliders.

## Demo

https://github.com/user-attachments/assets/7c4b36ec-0f6a-4aad-93fb-f9c966d43873

## Installation

See **[docs/SETUP.md](docs/SETUP.md)** for detailed walkthroughs.

### GitHub release packages (Debian/Ubuntu & Fedora/RHEL)

Use the stable filenames from the latest release:

```bash
# Debian/Ubuntu
wget -O wayscriber-amd64.deb https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-amd64.deb
sudo apt install ./wayscriber-amd64.deb
# Note: apt may print “Download is performed unsandboxed as root …” when installing a local file; this is expected.

# Fedora/RHEL
wget -O wayscriber-x86_64.rpm https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-x86_64.rpm
sudo rpm -Uvh wayscriber-x86_64.rpm
```

### Arch Linux (AUR)

```bash
# yay – build from source
yay -S wayscriber

# yay – prebuilt binaries
yay -S wayscriber-bin

# paru – build from source
paru -S wayscriber

# paru – prebuilt binaries
paru -S wayscriber-bin
```

The package installs the user service at `/usr/lib/systemd/user/wayscriber.service`.


### Other Distros

**Install dependencies:**

Ubuntu / Debian:
```bash
sudo apt-get install libcairo2-dev libwayland-dev libpango1.0-dev
```

Fedora:
```bash
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config cairo-devel wayland-devel pango-devel libxkbcommon-devel cairo-gobject-devel
```

Optional but recommended for screenshots:
```bash
sudo apt-get install wl-clipboard grim slurp   # Debian/Ubuntu
sudo dnf install wl-clipboard grim slurp       # Fedora
```

**Build from source:**

```bash
git clone https://github.com/devmobasa/wayscriber.git
cd wayscriber
cargo build --release
```

The binary will be at `target/release/wayscriber`.

### Manual Install Script

```bash
cargo build --release
./tools/install.sh
```

The installer places the binary at `~/.local/bin/wayscriber`, creates `~/.config/wayscriber/`, and offers to configure Hyprland.

## Running wayscriber

### Daemon Mode

Run wayscriber in the background and toggle with a keybind.

**Enable the service:**
```bash
systemctl --user enable --now wayscriber.service
```

**Add keybinding** to `~/.config/hypr/hyprland.conf`:
```conf
bind = SUPER, D, exec, pkill -SIGUSR1 wayscriber
```

**Reload Hyprland:**
```bash
hyprctl reload
```

The daemon shows a system tray icon (may be in Waybar drawer). Press `Super+D` to toggle overlay, right-click tray icon for options.

**Service commands:**
```bash
systemctl --user status wayscriber.service
systemctl --user restart wayscriber.service
journalctl --user -u wayscriber.service -f
```

**Note:** If the daemon doesn't start after reboot, see [Troubleshooting](#daemon-not-starting-after-reboot).

**Alternative:** Use Hyprland's exec-once instead of systemd:
```conf
exec-once = wayscriber --daemon
bind = SUPER, D, exec, pkill -SIGUSR1 wayscriber
```

### One-Shot Mode

Launch directly into an active overlay without the daemon:

```bash
wayscriber --active
wayscriber --active --mode whiteboard
wayscriber --active --mode blackboard
```

Bind it to keys if you prefer:

```conf
bind = $mainMod, D, exec, wayscriber --active
bind = $mainMod SHIFT, D, exec, wayscriber --active --mode whiteboard
```

Exit the overlay with `Escape` or `Ctrl+Q`.

### Screenshot Shortcuts

wayscriber ships with keyboard shortcuts for quick captures:

- `Ctrl+C` – copy the entire screen to the clipboard.
- `Ctrl+S` – save the entire screen as a PNG (uses your capture directory).
- `Ctrl+Shift+C` – select a region and copy it to the clipboard.
- `Ctrl+Shift+S` – select a region and save it as a PNG.
- `Ctrl+Shift+O` – capture the active window (Hyprland fast path, portal fallback).
- `Ctrl+6` / `Ctrl+Shift+6` – reserved for remembered-region clipboard/file captures (coming soon).

**Requirements:** install `wl-clipboard`, `grim`, and `slurp` for the fastest Hyprland workflow. If they are missing, wayscriber falls back to `xdg-desktop-portal`'s interactive picker.

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
| Highlight brush (highlight-only) | `Ctrl+Alt+H` |
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
| Redo last undo | `Ctrl+Shift+Z` or `Ctrl+Y` |
| Clear all | `E` |
| Cancel action | Right-click or `Escape` |
| **Toolbars & UI** |
| Toggle toolbars | `F2` or `F9` |
| Toggle help overlay | `F10` |
| Launch configurator | `F11` |
| Toggle status bar | `F12` |
| Toggle click highlight | `Ctrl+Shift+H` |
| Toggle freeze | `Ctrl+Shift+F` |
| Highlight brush (highlight-only) | `Ctrl+Alt+H` |
| **Exit** |
| Exit overlay | `Escape` or `Ctrl+Q` |

## Configuration

- Config file location: `~/.config/wayscriber/config.toml`.
- Copy defaults to get started:

  ```bash
  mkdir -p ~/.config/wayscriber
  cp config.example.toml ~/.config/wayscriber/config.toml
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
exec-once = wayscriber --daemon
```

### Service won't start

- Check status: `systemctl --user status wayscriber.service`
- Tail logs: `journalctl --user -u wayscriber.service -f`
- Restart: `systemctl --user restart wayscriber.service`

### Overlay not appearing

1. Verify Wayland session: `echo $WAYLAND_DISPLAY`
2. Ensure your compositor supports `wlr-layer-shell` (Hyprland, Sway, river, etc.)
3. Run with logs for clues: `RUST_LOG=info wayscriber --active`

### Config issues

- Confirm the file exists: `ls -la ~/.config/wayscriber/config.toml`
- Watch for TOML errors in logs: `RUST_LOG=info wayscriber --active`

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
wayscriber/
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

| Feature | ZoomIt (Windows) | wayscriber (Linux) |
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
- [ ] Additional shapes (filled shapes)
- [x] Highlighter/Highlight brush
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
