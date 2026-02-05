# wayscriber

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org/)

A ZoomIt-like real-time screen annotation tool for Linux/Wayland, written in Rust.

Docs: https://wayscriber.com/docs/

<details>
<summary>Screenshots</summary>

![Demo Poster](https://wayscriber.com/demo-poster-6.webp)
![Demo Poster](https://wayscriber.com/demo-poster-2.webp)

</details>

<details>
<summary>Demo Video</summary>

[View demo (wayscriber.com)](https://wayscriber.com/demo.webm)


https://github.com/user-attachments/assets/4b5ed159-8d1c-44cb-8fe4-e0f2ea41d818


</details>

---

## Table of Contents

- [Why wayscriber?](#why-wayscriber)
- [Features](#features)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Usage](#usage)
- [Getting Help](#getting-help)
- [Controls Reference](#controls-reference)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [Roadmap](#roadmap)
- [Additional Information](#additional-information)
- [License & Credits](#license--credits)

---

## Why wayscriber?

- **Annotate live** over any app without disrupting your workflow
- **Professional presentation tools** — presenter mode, numbered callouts, click highlights, screen freeze, zoom
- **Persistent sessions** that survive restarts
- **Native Wayland performance** with ZoomIt-like controls
- **Lightweight daemon** with instant toggle via keybind

### Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Wayland (layer-shell) | ✅ Supported | Hyprland, Sway, River, Wayfire, Niri/Cosmic, Plasma/KWin |
| GNOME | ⚠️ Partial | Portal fallback; overlay windowed |
| X11 | ❌ | Not supported |

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
- Freehand pen, highlighter, eraser (circle/rect)
- Shapes: lines, rectangles, ellipses (with fill toggle)
- Arrows with optional auto-numbered labels
- Step markers for walkthroughs
- Multiline text & sticky notes with smoothing
- Selection: Alt-drag, <kbd>V</kbd> tool, properties panel
- Duplicate (<kbd>Ctrl+D</kbd>), delete (<kbd>Delete</kbd>), undo/redo
- Color picker, palettes, size via hotkeys or scroll

### Boards
- Named boards with transparent overlay or custom backgrounds
- Isolated pages per board with auto-contrast pens
- Jump slots: <kbd>Ctrl+Shift+1..9</kbd>
- Toggle whiteboard/blackboard
- Board picker: <kbd>Ctrl+Shift+B</kbd>

### Capture & Screenshots
- Full-screen saves, active-window grabs, region capture
- Copy to clipboard or save to file
- Uses `grim`, `slurp`, `wl-clipboard` (installed automatically by deb/rpm/AUR packages; fallback: xdg-desktop-portal)

### Session Persistence
- Opt-in per board/monitor canvas + tool state restore
- CLI overrides: `--resume-session` / `--no-resume-session`
- Tray checkmark flips config on disk

### Toolbars & UI
- Floating toolbars (pin/unpin: <kbd>F2</kbd>/<kbd>F9</kbd>)
- Preset slots, icon or text modes
- Color picker with extended palettes
- Status bar, board/page controls
- Help overlay (<kbd>F1</kbd>), quick reference (<kbd>Shift+F1</kbd>)
- Command palette (<kbd>Ctrl+K</kbd>)

### Presets
- Save tool + color + size (plus fill/opacity/text) into 3-5 slots
- Apply: <kbd>1</kbd>-<kbd>5</kbd>
- Save: <kbd>Shift+1</kbd>-<kbd>Shift+5</kbd>

### Presenter Helpers
- Click highlights with configurable colors/radius/duration
- Persistent ring while click highlight tool is active
- Presenter mode (<kbd>Ctrl+Shift+M</kbd>): hides UI, forces click highlights
- Screen freeze (<kbd>Ctrl+Shift+F</kbd>): pause display while apps run

### Callouts & Zoom
- **Numbered callouts:** auto-numbered arrow labels, step markers
- Reset shortcuts (<kbd>Ctrl+Shift+R</kbd> for arrow labels): see Controls Reference
- **Zoom:** spotlight details with ZoomIt-style controls
  - Zoom in/out: <kbd>Ctrl+Alt</kbd> + scroll or <kbd>Ctrl+Alt++</kbd>/<kbd>Ctrl+Alt+-</kbd>
  - Reset: <kbd>Ctrl+Alt+0</kbd>
  - Lock view: <kbd>Ctrl+Alt+L</kbd>
  - Pan: middle drag or arrow keys

---

## Quick Start

**1. Install** (Debian/Ubuntu repo, auto-updates):
```bash
sudo install -d /usr/share/keyrings
curl -fsSL https://wayscriber.com/apt/WAYSCRIBER-GPG-KEY.asc | sudo gpg --dearmor -o /usr/share/keyrings/wayscriber.gpg
echo "deb [signed-by=/usr/share/keyrings/wayscriber.gpg] https://wayscriber.com/apt stable main" | sudo tee /etc/apt/sources.list.d/wayscriber.list
sudo apt update
sudo apt install wayscriber
# Optional GUI configurator
sudo apt install wayscriber-configurator
```

**2. Run:**
```bash
wayscriber --active
```

**3. Draw** — use your mouse. Press <kbd>F1</kbd> or <kbd>F10</kbd> for help; <kbd>Shift+F1</kbd> for quick reference; <kbd>Ctrl+K</kbd> for the command palette; <kbd>Escape</kbd> to exit.

For other distros or running as a daemon, see [Installation](#installation) and [Usage](#usage).

---

## Installation

### Debian / Ubuntu (repo – recommended)
```bash
sudo install -d /usr/share/keyrings
curl -fsSL https://wayscriber.com/apt/WAYSCRIBER-GPG-KEY.asc | sudo gpg --dearmor -o /usr/share/keyrings/wayscriber.gpg
echo "deb [signed-by=/usr/share/keyrings/wayscriber.gpg] https://wayscriber.com/apt stable main" | sudo tee /etc/apt/sources.list.d/wayscriber.list
sudo apt update
sudo apt install wayscriber
# Optional GUI configurator
sudo apt install wayscriber-configurator
```

One-off .deb (no auto-updates):
```bash
wget -O wayscriber-amd64.deb https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-amd64.deb
sudo apt install ./wayscriber-amd64.deb
```
Configurator .deb (optional):
```bash
wget -O wayscriber-configurator-amd64.deb https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-configurator-amd64.deb
sudo apt install ./wayscriber-configurator-amd64.deb
```

### Fedora / RHEL (repo – recommended)
```bash
cat <<'EOF' | sudo tee /etc/yum.repos.d/wayscriber.repo
[wayscriber]
name=Wayscriber Repo
baseurl=https://wayscriber.com/rpm
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=https://wayscriber.com/rpm/RPM-GPG-KEY-wayscriber.asc
EOF
sudo dnf clean all
sudo dnf install wayscriber
# Optional GUI configurator
sudo dnf install wayscriber-configurator
```

One-off .rpm (no auto-updates):
```bash
wget -O wayscriber-x86_64.rpm https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-x86_64.rpm
sudo rpm -Uvh wayscriber-x86_64.rpm
```
Configurator .rpm (optional):
```bash
wget -O wayscriber-configurator-x86_64.rpm https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-configurator-x86_64.rpm
sudo rpm -Uvh wayscriber-configurator-x86_64.rpm
```

### Arch Linux (AUR)

```bash
yay -S wayscriber        # from source
yay -S wayscriber-bin    # prebuilt binary
# Optional GUI configurator:
yay -S wayscriber-configurator
```

### NixOS / Nix

**Run without installing:**
```bash
nix run github:devmobasa/wayscriber
```

**Install to profile:**
```bash
nix profile install github:devmobasa/wayscriber
# Optional GUI configurator:
nix profile install github:devmobasa/wayscriber#wayscriber-configurator
```

**Add to NixOS configuration (flake-based):**
```nix
# flake.nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    wayscriber.url = "github:devmobasa/wayscriber";
  };

  outputs = { nixpkgs, wayscriber, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [{
        environment.systemPackages = [
          wayscriber.packages.x86_64-linux.default
        ];
      }];
    };
  };
}
```

**Development shell:**
```bash
nix develop github:devmobasa/wayscriber
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

### Screenshot Tools

`wl-clipboard`, `grim`, and `slurp` are installed automatically by deb/rpm/AUR packages.
If you build from source or use the tarball, install them manually:
```bash
sudo apt-get install wl-clipboard grim slurp   # Debian/Ubuntu
sudo dnf install wl-clipboard grim slurp       # Fedora
```

See https://wayscriber.com/docs/ for the latest documentation.

---

## Usage

### One-Shot Mode

Launch wayscriber when you need it, exit when done:

```bash
wayscriber --active
wayscriber --active --mode whiteboard
wayscriber --active --mode blueprint
wayscriber --freeze   # start with screen frozen
```

Bind to a key (Hyprland example):
```conf
bind = SUPER, D, exec, wayscriber --active
```

Press <kbd>F1</kbd>/<kbd>F10</kbd> for help or <kbd>Shift+F1</kbd> for quick reference. <kbd>Ctrl+K</kbd> opens the command palette; <kbd>F11</kbd> opens the configurator; <kbd>Escape</kbd> or <kbd>Ctrl+Q</kbd> exits.

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

Use `--no-tray` or `WAYSCRIBER_NO_TRAY=1` if you don't have a system tray; otherwise right-click the tray icon for options:
- Toggle overlay visibility
- Freeze/unfreeze the current overlay
- Capture full screen / active window / region
- Toggle the help overlay
- Flip session resume on/off (writes to config)
- Clear saved session data
- Open the log folder
- Open configurator / open config file / quit

Troubleshooting tray icons:
- If the tray icon is blank or the menu shows square placeholders (notably Noctalia/Quickshell), start the daemon with `WAYSCRIBER_TRAY_FORCE_PIXMAP=1`.

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
| <kbd>Ctrl+Shift+P</kbd> | Capture full screen (respects `capture.copy_to_clipboard`) |
| <kbd>Ctrl+Shift+O</kbd> | Capture active window (respects `capture.copy_to_clipboard`) |
| <kbd>Ctrl+Shift+I</kbd> | Capture selection (respects `capture.copy_to_clipboard`) |
| <kbd>Ctrl+C</kbd> | Copy full screen to clipboard |
| <kbd>Ctrl+S</kbd> | Save full screen as PNG |
| <kbd>Ctrl+Shift+C</kbd> | Select region → clipboard |
| <kbd>Ctrl+Shift+S</kbd> | Select region → save PNG |
| <kbd>Ctrl+6</kbd> | Region → clipboard (explicit) |
| <kbd>Ctrl+Alt+6</kbd> | Region → save PNG (explicit) |
| <kbd>Ctrl+Alt+O</kbd> | Open last capture folder |

Requires `wl-clipboard`, `grim`, `slurp` (installed automatically by deb/rpm/AUR packages). Falls back to xdg-desktop-portal if missing.
Use `--exit-after-capture` / `--no-exit-after-capture` to override whether the overlay closes after a capture. `--about` opens the About window.

---

## Getting Help

- **Help overlay:** <kbd>F1</kbd> or <kbd>F10</kbd>
- **Quick reference:** <kbd>Shift+F1</kbd>
- **Command palette:** <kbd>Ctrl+K</kbd>
- **Full docs:** https://wayscriber.com/docs/

---

## Controls Reference

Press <kbd>F1</kbd> for the complete in-app cheat sheet.

### Essential Shortcuts

| Action | Key |
|--------|-----|
| Freehand pen | Drag |
| Arrow | <kbd>Ctrl+Shift</kbd> + drag |
| Rectangle | <kbd>Ctrl</kbd> + drag |
| Text mode | <kbd>T</kbd> |
| Select/move | <kbd>Alt</kbd> + drag or <kbd>V</kbd> |
| Undo / Redo | <kbd>Ctrl+Z</kbd> / <kbd>Ctrl+Y</kbd> |
| Color keys | <kbd>R</kbd> <kbd>G</kbd> <kbd>B</kbd> <kbd>Y</kbd> <kbd>O</kbd> <kbd>P</kbd> <kbd>W</kbd> <kbd>K</kbd> |
| Size | <kbd>+</kbd> / <kbd>-</kbd> or scroll |
| Help | <kbd>F1</kbd> |
| Exit | <kbd>Escape</kbd> |

<details>
<summary>All Drawing Tools</summary>

| Action | Key/Mouse |
|--------|-----------|
| Freehand pen | Drag with left mouse button |
| Straight line | <kbd>Shift</kbd> + drag |
| Rectangle | <kbd>Ctrl</kbd> + drag |
| Ellipse/Circle | <kbd>Tab</kbd> + drag |
| Arrow | <kbd>Ctrl+Shift</kbd> + drag |
| Step marker tool | Toolbar (bindable) |
| Highlight brush | <kbd>Ctrl+Alt+H</kbd> |
| Text mode | <kbd>T</kbd>, <kbd>Click</kbd> to position, type, <kbd>Enter</kbd> to finish |
| Sticky note | <kbd>N</kbd>, <kbd>Click</kbd> to place, type, <kbd>Enter</kbd> to finish |

</details>

<details>
<summary>Boards</summary>

| Action | Key |
|--------|-----|
| Toggle Whiteboard | <kbd>Ctrl+W</kbd> |
| Toggle Blackboard | <kbd>Ctrl+B</kbd> |
| Return to Transparent | <kbd>Ctrl+Shift+T</kbd> |
| Switch board slot | <kbd>Ctrl+Shift+1..9</kbd> |
| Previous/next board | <kbd>Ctrl+Shift</kbd> + <kbd>←</kbd>/<kbd>→</kbd> |
| New board | <kbd>Ctrl+Shift+N</kbd> |
| Delete board | <kbd>Ctrl+Shift+Delete</kbd> |
| Board picker | <kbd>Ctrl+Shift+B</kbd> |

</details>

<details>
<summary>Colors & Sizes</summary>

| Color | Key |
|-------|-----|
| Red | <kbd>R</kbd> |
| Green | <kbd>G</kbd> |
| Blue | <kbd>B</kbd> |
| Yellow | <kbd>Y</kbd> |
| Orange | <kbd>O</kbd> |
| Pink | <kbd>P</kbd> |
| White | <kbd>W</kbd> |
| Black | <kbd>K</kbd> |

| Action | Key |
|--------|-----|
| Increase thickness | <kbd>+</kbd> / <kbd>=</kbd> / scroll down |
| Decrease thickness | <kbd>-</kbd> / <kbd>_</kbd> / scroll up |
| Increase font size | <kbd>Ctrl+Shift++</kbd> / <kbd>Shift</kbd> + scroll down |
| Decrease font size | <kbd>Ctrl+Shift+-</kbd> / <kbd>Shift</kbd> + scroll up |
| Increase marker opacity | <kbd>Ctrl+Alt</kbd> + <kbd>↑</kbd> |
| Decrease marker opacity | <kbd>Ctrl+Alt</kbd> + <kbd>↓</kbd> |

</details>

<details>
<summary>Selection & Arrange</summary>

| Action | Key |
|--------|-----|
| Duplicate selection | <kbd>Ctrl+D</kbd> |
| Copy selection | <kbd>Ctrl+Alt+C</kbd> |
| Paste selection | <kbd>Ctrl+Alt+V</kbd> |
| Delete selection | <kbd>Delete</kbd> |
| Bring to front/back | <kbd>]</kbd> / <kbd>[</kbd> |
| Nudge selection | Arrow keys (large: <kbd>PageUp</kbd>/<kbd>PageDown</kbd>) |
| Move to edges | <kbd>Home</kbd>/<kbd>End</kbd> / <kbd>Ctrl+Home</kbd>/<kbd>Ctrl+End</kbd> |
| Select/move shapes | Hold <kbd>Alt</kbd> + drag |
| Select tool | <kbd>V</kbd> |
| Add to selection | <kbd>Shift</kbd> + click |
| Selection properties | <kbd>Ctrl+Alt+P</kbd> |

</details>

<details>
<summary>Pages</summary>

| Action | Key |
|--------|-----|
| Previous/next page | <kbd>Ctrl+Alt</kbd> + <kbd>←</kbd>/<kbd>→</kbd> or <kbd>Ctrl+Alt</kbd> + <kbd>PageUp</kbd>/<kbd>PageDown</kbd> |
| New page | <kbd>Ctrl+Alt+N</kbd> |
| Duplicate page | <kbd>Ctrl+Alt+D</kbd> |
| Delete page | <kbd>Ctrl+Alt+Delete</kbd> |

</details>

<details>
<summary>Editing & UI</summary>

| Action | Key |
|--------|-----|
| Undo | <kbd>Ctrl+Z</kbd> |
| Redo | <kbd>Ctrl+Shift+Z</kbd> / <kbd>Ctrl+Y</kbd> |
| Select all | <kbd>Ctrl+A</kbd> |
| Eraser | <kbd>D</kbd> |
| Toggle eraser mode | <kbd>Ctrl+Shift+E</kbd> |
| Clear all | <kbd>E</kbd> |
| Cancel action | <kbd>Right-click</kbd> (while drawing) / <kbd>Escape</kbd> |
| Context menu | <kbd>Right-click</kbd> (idle) / <kbd>Shift+F10</kbd> / <kbd>Menu</kbd>, <kbd>Arrow keys</kbd> + <kbd>Enter</kbd>/<kbd>Space</kbd> |
| Edit selected text/note | <kbd>Enter</kbd> (single selection) |
| Toggle toolbars | <kbd>F2</kbd> / <kbd>F9</kbd> |
| Help overlay | <kbd>F1</kbd> / <kbd>F10</kbd> |
| Quick reference | <kbd>Shift+F1</kbd> |
| Configurator | <kbd>F11</kbd> |
| Command palette | <kbd>Ctrl+K</kbd> |
| Status bar | <kbd>F4</kbd> / <kbd>F12</kbd> |
| Apply preset slot | <kbd>1</kbd> - <kbd>5</kbd> |
| Save preset slot | <kbd>Shift+1</kbd> - <kbd>Shift+5</kbd> |
| Toggle click highlight | <kbd>Ctrl+Shift+H</kbd> |
| Reset arrow labels | <kbd>Ctrl+Shift+R</kbd> |
| Toggle freeze | <kbd>Ctrl+Shift+F</kbd> |
| Zoom in/out | <kbd>Ctrl+Alt</kbd> + scroll / <kbd>Ctrl+Alt++</kbd> / <kbd>Ctrl+Alt+-</kbd> |
| Reset zoom | <kbd>Ctrl+Alt+0</kbd> |
| Toggle zoom lock | <kbd>Ctrl+Alt+L</kbd> |
| Pan zoom view | <kbd>Middle drag</kbd> / <kbd>Arrow keys</kbd> |
| Exit | <kbd>Escape</kbd> / <kbd>Ctrl+Q</kbd> |

</details>

Arrow labels can auto-number when enabled in the arrow toolbar; reset with <kbd>Ctrl+Shift+R</kbd>. Step markers auto-increment and reset from the toolbar (or bind `reset_step_markers` in `config.toml`). Preset slots can be saved/cleared from the toolbar; edit names and advanced fields in `config.toml`.

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

[presets]
# slot_count: 3-5
slot_count = 5

[presets.slot_1]
name = "Red pen"
tool = "pen"
color = "red"
size = 3.0

[performance]
buffer_count = 3
enable_vsync = true
max_fps_no_vsync = 60
ui_animation_fps = 30

[ui]
# status bar visibility and position

[boards]
# named boards + backgrounds
```

### Session Persistence

Enable via configurator (<kbd>F11</kbd> → Session tab), CLI flags, or the tray checkmark (writes to config).

```bash
wayscriber --resume-session      # force resume (persist/restore all boards + history/tool state)
wayscriber --no-resume-session   # disable resume for this run
wayscriber --session-info        # inspect saved sessions
wayscriber --clear-session       # remove stored boards
```

Notes:
- When `restore_tool_state` is enabled (default), the last-used tool settings (including arrow head placement) override config defaults on startup. Disable it in the Session tab or clear the session to force config values.

### Tablet/Stylus Support

Tablet support (`zwp_tablet_v2`) ships in default builds but is disabled at runtime by default:

```toml
[tablet]
enabled = true
pressure_enabled = true
min_thickness = 1.0
max_thickness = 8.0
```

Enable it in `config.toml` and restart wayscriber. To build without tablet support: `cargo build --release --no-default-features` (or remove the `tablet-input` feature).

See https://wayscriber.com/docs/ for the full reference.

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

### Environment variables

Common toggles:
- `WAYSCRIBER_TOOLBAR_DRAG_PREVIEW=0` disables inline toolbar drag preview (default: on)
- `WAYSCRIBER_TOOLBAR_POINTER_LOCK=1` enables pointer-lock drag path (default: on)
- `WAYSCRIBER_TOOLBAR_DRAG_THROTTLE_MS=12` throttles toolbar drag updates (default: 12; set 0 to disable)
- `WAYSCRIBER_DEBUG_TOOLBAR_DRAG=1` enables toolbar drag logging (default: off)
- `WAYSCRIBER_DEBUG_TOOLBAR_COLOR=1` enables toolbar color picker logging (default: off)
- `WAYSCRIBER_FORCE_INLINE_TOOLBARS=1` forces inline toolbars on Wayland (default: off)
- `WAYSCRIBER_NO_TRAY=1` disables the tray icon (default: tray enabled)

See `docs/CONFIG.md` for the full list.

### Performance tuning

```toml
[performance]
buffer_count = 2
enable_vsync = true
ui_animation_fps = 30
```

---

## Contributing

Contribution principles: wayscriber is shared as a gift exchange, not a contract. Requests
are welcome, but there is no guaranteed timeline or support obligation. For the full
principles, see https://wayscriber.com/docs/ethos/gift-exchange.html

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, project structure, and workflow notes.

### Roadmap

- [x] Native Wayland layer-shell
- [x] Daemon mode with system tray
- [x] Multiple customizable boards/backgrounds
- [x] Session persistence (with CLI override + tray config toggle)
- [x] Highlighter & eraser tools
- [x] Additional shapes (filled shapes)
- [x] Color picker
- [x] Zoom (ZoomIt-style controls)
- [x] Presets (tool/color/size slots)
- [x] Sticky notes
- [x] Board pages (multi-page boards)
- [x] Presenter mode
- [x] Command palette
- [x] Numbered callouts (incrementing arrow labels)
- [ ] Multi-monitor support
- [ ] Save annotations to image
- [ ] Color picker integration with captures/export

See [Satty](https://github.com/gabm/Satty) for capture → annotate → save workflows. wayscriber is designed as an always-available drawing layer.

---

## Additional Information

<details>
<summary>Comparison with ZoomIt</summary>

| Feature | ZoomIt (Windows) | wayscriber |
|---------|------------------|------------|
| Drawing tools | ✅ | ✅ |
| Boards/Backgrounds | ✅ | ✅ |
| Multi-line text | ❌ | ✅ |
| Custom fonts | ❌ | ✅ |
| Config file | ❌ | ✅ |
| Help overlay | ❌ | ✅ |
| Zoom | ✅ | ✅ |
| Break timer | ✅ | ❌ |

</details>

---

## License & Credits

**MIT License** — see [LICENSE](LICENSE)

### Acknowledgments

- Inspired by [ZoomIt](https://learn.microsoft.com/en-us/sysinternals/downloads/zoomit) by Mark Russinovich
- Built for Linux (distros that use Wayland)
- Similar ideas from [Gromit-MPX](https://github.com/bk138/gromit-mpx)
- Uses [Cairo](https://www.cairographics.org/) and [smithay-client-toolkit](https://github.com/Smithay/client-toolkit)

Developed with AI assistance (ChatGPT, Codex, Claude Code).
