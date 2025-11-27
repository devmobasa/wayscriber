# wayscriber

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)

A ZoomIt-like real-time screen annotation tool for Linux/Wayland, written in Rust.

<details>
<summary>Screenshots</summary>

![Demo Poster](https://wayscriber.com/demo-poster-4.webp)
![Demo Poster](https://wayscriber.com/demo-poster-2.webp)

</details>

<details>
<summary>Demo Video</summary>

[View demo (wayscriber.com)](https://wayscriber.com/demo.mp4)

https://github.com/user-attachments/assets/75fe3e9b-b156-47e5-8434-318d7f25151d

</details>

---

## Table of Contents

- [Why wayscriber?](#why-wayscriber)
- [Features](#features)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Usage](#usage)
- [Controls Reference](#controls-reference)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [Additional Information](#additional-information)

---

## Why wayscriber?

- **Annotate live** over any app/window on any monitor without rearranging your workspace
- **Draw shapes, arrows, and text** to explain steps, give demos, or build quick guides
- **Redact screen regions** and capture screenshots with one keypress
- **Toggle instantly** from a lightweight background daemon
- **Persist your work** — canvases and tool state restore after restarts
- **Presenter helpers** — click highlights and screen freeze while apps keep running

### Supported Compositors

Works on layer-shell compositors (wlroots, Smithay-based like Niri/Cosmic, Plasma KDE/KWin, Hyprland, Sway, Wayfire, River) with an xdg fallback for GNOME.

<details>
<summary>Tested environments</summary>

- Ubuntu 25.10 GNOME (xdg fallback)
- Fedora 43 KDE (Plasma, layer-shell)
- Fedora 43 GNOME (xdg fallback)
- Debian 13.2 KDE (Plasma, layer-shell)
- Debian 13.2 GNOME (xdg fallback)
- CachyOS 2025-August KDE (Plasma, layer-shell)
- Hyprland on Arch (layer-shell)
- Niri on Arch (layer-shell)

</details>

---

## Features

### Drawing & Editing
Freehand pen, highlighter, eraser (circle/rect), straight lines, rectangles, ellipses, arrows, multiline text with smoothing. Undo/redo, quick size/color changes via hotkeys or scroll.

### Board Modes
Whiteboard, blackboard, and transparent overlays with isolated frames and auto pen contrast. Snap back to transparent with `Ctrl+Shift+T`.

### Capture & Screenshots
Full-screen saves, active-window grabs, and region capture to file or clipboard using `grim`, `slurp`, and `wl-clipboard`.

### Session Persistence
Opt-in per board/monitor storage that restores your canvas plus pen color & thickness.

### Toolbars & UI
Floating toolbars (pin/unpin with `F2`/`F9`), icon or text modes, color palettes, status bar, and in-app help overlay (`F1`/`F10`).

### Presenter Helpers
Click highlights with configurable colors/radius/duration. Screen freeze (`Ctrl+Shift+F`) to pause what viewers see while apps run.

---

## Quick Start

**1. Install** (Debian/Ubuntu):
```bash
wget -O wayscriber-amd64.deb https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-amd64.deb
sudo apt install ./wayscriber-amd64.deb
```

**2. Run**:
```bash
wayscriber --active
```

**3. Draw** — use your mouse. Press `F1` or `F10` for help, `Escape` to exit.

For other distros or running as a daemon, see [Installation](#installation) and [Usage](#usage).

---

## Installation

### Debian / Ubuntu

```bash
wget -O wayscriber-amd64.deb https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-amd64.deb
sudo apt install ./wayscriber-amd64.deb
```

### Fedora / RHEL

```bash
wget -O wayscriber-x86_64.rpm https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-x86_64.rpm
sudo rpm -Uvh wayscriber-x86_64.rpm
```

### Arch Linux (AUR)

```bash
yay -S wayscriber        # from source
yay -S wayscriber-bin    # prebuilt binary
```

### From Source

**Dependencies:**

```bash
# Debian/Ubuntu
sudo apt-get install libcairo2-dev libwayland-dev libpango1.0-dev

# Fedora
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config cairo-devel wayland-devel pango-devel libxkbcommon-devel cairo-gobject-devel
```

**Build:**

```bash
git clone https://github.com/devmobasa/wayscriber.git
cd wayscriber
cargo build --release
# Binary: target/release/wayscriber
```

**Optional install script:**
```bash
./tools/install.sh
```

### Optional Dependencies

For the best screenshot workflow, install:
```bash
sudo apt-get install wl-clipboard grim slurp   # Debian/Ubuntu
sudo dnf install wl-clipboard grim slurp       # Fedora
```

See **[docs/SETUP.md](docs/SETUP.md)** for detailed walkthroughs.

---

## Usage

### One-Shot Mode

Launch wayscriber when you need it, exit when done:

```bash
wayscriber --active
wayscriber --active --mode whiteboard
wayscriber --active --mode blackboard
wayscriber --freeze   # start with screen frozen
```

Bind to a key (Hyprland example):
```conf
bind = SUPER, D, exec, wayscriber --active
```

Press `F1`/`F10` for help, `F11` for configurator, `Escape` or `Ctrl+Q` to exit.

### Daemon Mode

Run wayscriber in the background and toggle with a keybind:

```bash
systemctl --user enable --now wayscriber.service
```

Add keybinding (Hyprland):
```conf
bind = SUPER, D, exec, pkill -SIGUSR1 wayscriber
```

Reload your config:
```bash
hyprctl reload
```

> [!CAUTION]
> Daemon mode requires a system tray. Without one, the daemon will fail to start.

**Alternative** — use compositor autostart instead of systemd:
```conf
exec-once = wayscriber --daemon
bind = SUPER, D, exec, pkill -SIGUSR1 wayscriber
```

**Service commands:**
```bash
systemctl --user status wayscriber.service
systemctl --user restart wayscriber.service
journalctl --user -u wayscriber.service -f
```

### Screenshot Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+C` | Copy entire screen to clipboard |
| `Ctrl+S` | Save entire screen as PNG |
| `Ctrl+Shift+C` | Select region → clipboard |
| `Ctrl+Shift+S` | Select region → save PNG |
| `Ctrl+Shift+O` | Capture active window |

Requires `wl-clipboard`, `grim`, `slurp`. Falls back to xdg-desktop-portal if missing.

---

## Controls Reference

Press `F1` or `F10` at any time for the in-app cheat sheet.

### Drawing Tools

| Action | Key/Mouse |
|--------|-----------|
| Freehand pen | Drag with left mouse button |
| Straight line | `Shift` + drag |
| Rectangle | `Ctrl` + drag |
| Ellipse/Circle | `Tab` + drag |
| Arrow | `Ctrl+Shift` + drag |
| Highlight brush | `Ctrl+Alt+H` |
| Text mode | `T`, click to position, type, `Enter` to finish |

### Board Modes

| Action | Key |
|--------|-----|
| Toggle Whiteboard | `Ctrl+W` |
| Toggle Blackboard | `Ctrl+B` |
| Return to Transparent | `Ctrl+Shift+T` |

### Colors

| Color | Key |
|-------|-----|
| Red | `R` |
| Green | `G` |
| Blue | `B` |
| Yellow | `Y` |
| Orange | `O` |
| Pink | `P` |
| White | `W` |
| Black | `K` |

### Size Adjustments

| Action | Key |
|--------|-----|
| Increase thickness | `+` / `=` / scroll down |
| Decrease thickness | `-` / `_` / scroll up |
| Increase font size | `Ctrl+Shift++` / `Shift` + scroll down |
| Decrease font size | `Ctrl+Shift+-` / `Shift` + scroll up |

### Editing & UI

| Action | Key |
|--------|-----|
| Undo | `Ctrl+Z` |
| Redo | `Ctrl+Shift+Z` / `Ctrl+Y` |
| Eraser | `D` |
| Clear all | `E` |
| Cancel action | Right-click / `Escape` |
| Toggle toolbars | `F2` / `F9` |
| Help overlay | `F1` / `F10` |
| Configurator | `F11` |
| Status bar | `F4` / `F12` |
| Toggle click highlight | `Ctrl+Shift+H` |
| Toggle freeze | `Ctrl+Shift+F` |
| Exit | `Escape` / `Ctrl+Q` |

---

## Configuration

Config file: `~/.config/wayscriber/config.toml`

**Create from example:**
```bash
mkdir -p ~/.config/wayscriber
cp config.example.toml ~/.config/wayscriber/config.toml
```

**Or use the GUI configurator:**
```bash
wayscriber-configurator   # or press F11
```

### Key Sections

```toml
[drawing]
default_color = "red"
default_thickness = 3.0

[performance]
buffer_count = 3
enable_vsync = true

[ui]
# status bar visibility and position

[board]
# whiteboard/blackboard presets
```

### Session Persistence

Enable via configurator (`F11 → Session` tab) or edit config directly.

```bash
wayscriber --session-info     # inspect saved sessions
wayscriber --clear-session    # remove stored boards
```

### Tablet/Stylus Support

Tablet support (`zwp_tablet_v2`) is enabled by default:

```toml
[tablet]
enabled = true
pressure_enabled = true
min_thickness = 1.0
max_thickness = 8.0
```

To build without tablet support: `cargo build --release --no-default-features`

See **[docs/CONFIG.md](docs/CONFIG.md)** for the full reference.

---

## Troubleshooting

### Daemon not starting after reboot

User services don't start at boot by default. Enable lingering:
```bash
loginctl enable-linger $USER
```

Or use compositor autostart instead:
```conf
exec-once = wayscriber --daemon
```

### Service won't start

```bash
systemctl --user status wayscriber.service
journalctl --user -u wayscriber.service -f
systemctl --user restart wayscriber.service
```

### Overlay not appearing

1. Verify Wayland session: `echo $WAYLAND_DISPLAY`
2. Ensure compositor supports `wlr-layer-shell`
3. Run with logs: `RUST_LOG=info wayscriber --active`

### Config issues

```bash
ls -la ~/.config/wayscriber/config.toml
RUST_LOG=info wayscriber --active   # watch for TOML errors
```

### Performance tuning

```toml
[performance]
buffer_count = 2
enable_vsync = true
```

---

## Contributing

Pull requests and bug reports welcome. Priority areas:
- Compositor compatibility testing
- Multi-monitor support
- New drawing tools

### Development

```bash
cargo fmt
cargo clippy
cargo test
cargo run -- --active
```

Use `./tools/fetch-all-deps.sh` to prefetch crates before offline builds.

### Architecture

<details>
<summary>Project structure</summary>

```
wayscriber/
├── src/
│   ├── main.rs           # Entry point, CLI parsing
│   ├── daemon.rs         # Daemon mode with signal handling
│   ├── ui.rs             # Status bar and help overlay
│   ├── backend/
│   │   └── wayland.rs    # Wayland wlr-layer-shell implementation
│   ├── config/           # Configuration loader and types
│   ├── draw/             # Drawing, shapes, rendering (Cairo/Pango)
│   └── input/            # Input handling, state machine
├── tools/                # Helper scripts
├── packaging/            # Distribution files
├── docs/                 # Documentation
└── config.example.toml
```

</details>

---

## Additional Information

### Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Wayland (layer-shell) | ✅ Supported | Hyprland, Sway, River, Wayfire, Niri/Cosmic, Plasma/KWin |
| GNOME | ⚠️ Partial | Portal fallback; overlay windowed |
| X11 | ❌ | Not supported |

### Comparison with ZoomIt

| Feature | ZoomIt (Windows) | wayscriber |
|---------|------------------|------------|
| Drawing tools | ✅ | ✅ |
| Whiteboard/Blackboard | ✅ | ✅ |
| Multi-line text | ❌ | ✅ |
| Custom fonts | ❌ | ✅ |
| Config file | ❌ | ✅ |
| Help overlay | ❌ | ✅ |
| Zoom | ✅ | ❌ |
| Break timer | ✅ | ❌ |

### Roadmap

- [x] Native Wayland layer-shell
- [x] Daemon mode with system tray
- [x] Whiteboard/blackboard modes
- [x] Session persistence
- [x] Highlighter & eraser tools
- [ ] Multi-monitor support
- [ ] Save annotations to image
- [ ] Color picker

See [Satty](https://github.com/gabm/Satty) for capture → annotate → save workflows. wayscriber is designed as an always-available drawing layer.

---

## License & Credits

**MIT License** — see [LICENSE](LICENSE)

### Acknowledgments

- Inspired by [ZoomIt](https://learn.microsoft.com/en-us/sysinternals/downloads/zoomit) by Mark Russinovich
- Built for [Hyprland](https://hyprland.org/) by vaxry
- Similar ideas from [Gromit-MPX](https://github.com/bk138/gromit-mpx)
- Uses [Cairo](https://www.cairographics.org/) and [smithay-client-toolkit](https://github.com/Smithay/client-toolkit)

Developed with AI assistance (ChatGPT, Codex, Claude Code).
