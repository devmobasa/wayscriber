# wayscriber

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://www.rust-lang.org/)

A ZoomIt-like real-time screen annotation tool for Linux/Wayland, written in Rust.

Draw over any app, present with callouts and zoom, keep your boards between sessions — all from a lightweight daemon you toggle with one keybind.

**Docs:** https://wayscriber.com/docs/

![wayscriber annotating source code with arrows, highlights, shapes, step markers and text](https://wayscriber.com/img/annotate-over-code.webp)

<details>
<summary>More screenshots</summary>

| | |
| :---: | :---: |
| ![Radial tool menu](https://wayscriber.com/img/radial-menu.webp)<br>**Radial tool menu** | ![Boards and pages](https://wayscriber.com/img/boards-and-pages.webp)<br>**Boards & pages** |
| ![Blackboard mode](https://wayscriber.com/img/blackboard.webp)<br>**Blackboard** | ![Whiteboard mode](https://wayscriber.com/img/whiteboard.webp)<br>**Whiteboard** |
| ![Advanced toolbar](https://wayscriber.com/img/toolbar-advanced.webp)<br>**Advanced toolbar** | ![Command palette](https://wayscriber.com/img/command-palette.webp)<br>**Command palette** |
| ![Built-in controls](https://wayscriber.com/img/controls-cheatsheet.webp)<br>**Built-in controls (F1)** | ![Simple toolbar](https://wayscriber.com/img/toolbar-simple.webp)<br>**Simple toolbar** |

</details>

<details>
<summary>Demo video</summary>

[View demo (wayscriber.com)](https://wayscriber.com/demo.webm)

https://github.com/user-attachments/assets/4b5ed159-8d1c-44cb-8fe4-e0f2ea41d818

</details>

---

## Table of contents

- [Why wayscriber?](#why-wayscriber)
- [Features](#features)
- [Installation](#installation)
  - [Debian and Ubuntu](#debian-and-ubuntu)
  - [Fedora and RHEL](#fedora-and-rhel)
  - [Arch Linux (AUR)](#arch-linux-aur)
  - [NixOS and Nix](#nixos-and-nix)
  - [GitHub Releases (one-off)](#github-releases-one-off)
  - [From source](#from-source)
  - [Screenshot tools](#screenshot-tools)
- [First launch](#first-launch)
- [Usage](#usage)
  - [Daemon mode (recommended)](#daemon-mode-recommended)
  - [One-shot mode (alternative)](#one-shot-mode-alternative)
  - [Light passthrough mode](#light-passthrough-mode)
  - [Screenshots and export](#screenshots-and-export)
- [Getting help](#getting-help)
- [Controls reference](#controls-reference)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [Roadmap](#roadmap)
- [License and credits](#license-and-credits)

---

## Why wayscriber?

- **Annotate live** over any app without disrupting your workflow
- **Professional presentation tools** — presenter mode, numbered callouts, click highlights, screen freeze, zoom
- **Persistent sessions** that survive restarts
- **Native Wayland performance** with ZoomIt-like controls
- **Lightweight daemon** with instant toggle via keybind

### Platform support

| Platform | Status | Notes |
|----------|--------|-------|
| Wayland (layer-shell) | ✅ Supported | Hyprland, Sway, River, Wayfire, Niri/Cosmic, Plasma/KWin |
| GNOME | ⚠️ Partial | Normal overlay and Freeze via portal when available; [light passthrough](#light-passthrough-mode) unavailable |
| X11 | ❌ | Not supported |

The prebuilt `.deb` packages have a minimum-release requirement — see the note in [Debian and Ubuntu](#debian-and-ubuntu). RPMs, the AUR packages, and Nix are unaffected.

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

## Features

### Drawing and editing
- Freehand pen, highlighter, eraser (circle/rect)
- Shapes: lines, rectangles, ellipses, polygons (with fill toggle)
- Arrows with optional auto-numbered labels; step markers for walkthroughs
- Multiline text and sticky notes with smoothing
- Selection: <kbd>Alt</kbd>-drag, <kbd>V</kbd> tool, properties panel
- Duplicate (<kbd>Ctrl+D</kbd>), delete (<kbd>Delete</kbd>), undo/redo
- Color picker, screen eyedropper with a magnified pixel loupe, palettes, size via hotkeys or scroll
- Render color profiles for print/projector/light-theme preview
- Radial menu at cursor (<kbd>Middle-click</kbd>): quick tool/color selection plus scroll size adjust

### Boards
- Named boards with transparent overlay or custom backgrounds
- Isolated pages per board with auto-contrast pens
- Pan solid boards with <kbd>Space</kbd> + left-drag; reset from the context menu
- Jump slots: <kbd>Ctrl+Shift+1..9</kbd>
- Toggle whiteboard/blackboard
- Board picker: <kbd>Ctrl+Shift+B</kbd>

### Capture and screenshots
- Full-screen saves, active-window grabs, region capture
- Copy to clipboard or save to file
- Uses `grim`, `slurp`, `wl-clipboard` (installed automatically by deb/rpm/AUR packages; fallback: xdg-desktop-portal)

### Sessions and persistence
- Session persistence is enabled by default for boards, undo/redo history, and tool state
- Per-output default sessions, plus named session files with `--session-file`
- Overlay Session panel, configurator Session tab, tray toggle, and CLI overrides
- See [Session manager and persistence](#session-manager-and-persistence) for the full workflow

### Toolbars and UI
- Floating toolbars (pin/unpin: <kbd>F2</kbd>/<kbd>F9</kbd>)
- Two toolbar frontends: GTK4-rendered bars on layer-shell compositors (Hyprland, KWin, Wayfire, River, ...), with automatic fallback to the built-in Cairo bars everywhere else (GNOME xdg fallback, forced-inline mode, builds without the `toolbar-gtk` feature)
- Pick a frontend explicitly with `ui.toolbar.backend = "auto" | "gtk" | "builtin"` or `WAYSCRIBER_TOOLBAR_BACKEND`
- Preset slots, icon or text modes
- Color picker with extended palettes and a screen eyedropper (toolbar, popup, or command palette)
- Status bar, board/page controls
- Help overlay (<kbd>F1</kbd>), quick reference (<kbd>Shift+F1</kbd>)
- Command palette (<kbd>Ctrl+K</kbd> or <kbd>Ctrl+Shift+P</kbd>)
- Search, run, edit, unbind, or reset action shortcuts from the command palette; hold <kbd>Ctrl</kbd>+<kbd>Shift</kbd> while clicking a bindable toolbar control for direct shortcut capture (the modifier chord is configurable)

### Multi-monitor
- Move overlay focus between monitors: <kbd>Ctrl+Alt+Shift+←</kbd>/<kbd>Ctrl+Alt+Shift+→</kbd>
- Toolbars and status bar follow the active output when output focus changes
- Optional active output badge in status bar (`ui.active_output_badge`)
- Output-scoped session restore when `session.per_output = true`
- GNOME fallback output pinning via `ui.preferred_output` or `WAYSCRIBER_XDG_OUTPUT`

### Presets
- Save tool + color + size (plus fill/opacity/text) into 3–5 slots
- Apply: <kbd>1</kbd>–<kbd>5</kbd>; save: <kbd>Shift+1</kbd>–<kbd>Shift+5</kbd>

### Presenter tools
- Click highlights with configurable colors/radius/duration
- Persistent ring while the click highlight tool is active
- Presenter mode (<kbd>Ctrl+Shift+M</kbd>): hides UI, forces click highlights
- Light passthrough (layer-shell): draw while input passes through to the app underneath — see [Light passthrough mode](#light-passthrough-mode)
- Screen freeze (<kbd>Ctrl+Shift+F</kbd>): pause the display while apps keep running. On GNOME, this uses the screenshot portal when available

### Callouts and zoom
- **Numbered callouts:** auto-numbered arrow labels and step markers; reset arrow labels with <kbd>Ctrl+Shift+R</kbd>
- **Zoom:** spotlight details with ZoomIt-style controls
  - Zoom in/out: <kbd>Ctrl+Alt</kbd> + scroll or <kbd>Ctrl+Alt</kbd> + <kbd>+</kbd>/<kbd>-</kbd>
  - Reset: <kbd>Ctrl+Alt+0</kbd>; lock view: <kbd>Ctrl+Alt+L</kbd>
  - Pan: middle drag or arrow keys
  - Right-click menu: **Zoom** → Zoom In / Zoom Out / Reset Zoom

---

## Installation

Pick the path that matches your setup:

| You want | Use |
|----------|-----|
| Fast CLI install with auto-updates on Debian/Ubuntu/Mint/Pop!_OS | [Debian and Ubuntu](#debian-and-ubuntu) |
| Fast CLI install with auto-updates on Fedora/RHEL/Rocky/Alma/Nobara | [Fedora and RHEL](#fedora-and-rhel) |
| Arch, Manjaro, CachyOS, or another Arch-based distro | [AUR](#arch-linux-aur), preferably `wayscriber-bin` for the prebuilt package |
| Nix profile, `nix run`, or NixOS flake setup | [NixOS and Nix](#nixos-and-nix) |
| One-off package (browser or terminal) without adding a repo | [GitHub Releases](#github-releases-one-off) |
| Hacking on wayscriber or building a local binary | [From source](#from-source) |

Repo, AUR, and Nix installs are the best default for CLI users because they use the normal system update flow. GitHub `.deb`/`.rpm` downloads are one-off installs (no auto-updates); use them only when you do not want to add a repo.

Install the main `wayscriber` package first. `wayscriber-configurator` is an optional GUI settings app and does not include the `wayscriber` binary.

### Debian and Ubuntu

Also use this path for Linux Mint, Pop!_OS, and other Debian-based distros.

> **Requires Ubuntu 25.04+ or Debian 13 (trixie) or newer.** The packages depend on `libgtk4-layer-shell0` for the toolbars, which older releases — including Ubuntu 24.04 LTS, Mint 22, and Pop!_OS 22.04 — do not ship. On those, [build from source](#from-source); Pop!_OS 22.04 requires the GTK-less build option.

```bash
sudo apt update
sudo apt install curl gpg
sudo install -d -m 0755 /usr/share/keyrings
curl -fsSL https://wayscriber.com/apt/WAYSCRIBER-GPG-KEY.asc | gpg --dearmor | sudo tee /usr/share/keyrings/wayscriber.gpg > /dev/null
echo "deb [signed-by=/usr/share/keyrings/wayscriber.gpg] https://wayscriber.com/apt stable main" | sudo tee /etc/apt/sources.list.d/wayscriber.list
sudo apt update
sudo apt install wayscriber
# Optional GUI configurator:
sudo apt install wayscriber-configurator
```

For a one-off `.deb` without adding the repo, see [GitHub Releases](#github-releases-one-off).

### Fedora and RHEL

Also use this path for Rocky Linux, AlmaLinux, Nobara, and other RPM-based distros.

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
# Optional GUI configurator:
sudo dnf install wayscriber-configurator
```

For a one-off `.rpm` without adding the repo, see [GitHub Releases](#github-releases-one-off).

### Arch Linux (AUR)

Also use this path for Manjaro, CachyOS, and other Arch-based distros.

```bash
yay -S wayscriber-bin    # prebuilt binary
# or:
yay -S wayscriber        # build from source
# Optional GUI configurator:
yay -S wayscriber-configurator
```

Use your preferred AUR helper if you do not use `yay`.

### NixOS and Nix

**Run without installing:**
```bash
nix run github:devmobasa/wayscriber -- --active
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

Unpinned GitHub flake URLs follow the default branch. Pin a release tag when you want reproducible release installs:
```bash
nix run 'github:devmobasa/wayscriber?ref=v0.9.19' -- --active
nix profile install 'github:devmobasa/wayscriber?ref=v0.9.19'
nix profile install 'github:devmobasa/wayscriber?ref=v0.9.19#wayscriber-configurator'
```

### GitHub Releases (one-off)

Install the release package directly if you prefer not to add a repo — these are one-off installs with no auto-updates.

In a browser:

1. Open the [latest release](https://github.com/devmobasa/wayscriber/releases/latest).
2. Install the main app package that matches your distro:
   - [wayscriber-amd64.deb](https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-amd64.deb) — Ubuntu 25.04+, Debian 13+, and other newer Debian-based distros (see the [release requirement](#debian-and-ubuntu))
   - [wayscriber-x86_64.rpm](https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-x86_64.rpm) — Fedora, RHEL, Rocky Linux, AlmaLinux, Nobara, and other RPM-based distros
3. Optional: install the configurator package after wayscriber:
   - [wayscriber-configurator-amd64.deb](https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-configurator-amd64.deb)
   - [wayscriber-configurator-x86_64.rpm](https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-configurator-x86_64.rpm)
   - Note: `wayscriber-configurator` is a separate app and does not install `wayscriber`.
4. Launch **Wayscriber** from your application menu/launcher.
5. Optional later: switch to daemon mode (recommended) using the steps in [Daemon mode](#daemon-mode-recommended).

<details>
<summary>Same one-off install from the terminal (.deb / .rpm)</summary>

Debian/Ubuntu:
```bash
wget -O wayscriber-amd64.deb https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-amd64.deb
sudo apt install ./wayscriber-amd64.deb
```

Fedora/RHEL:
```bash
wget -O wayscriber-x86_64.rpm https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-x86_64.rpm
sudo dnf install ./wayscriber-x86_64.rpm
```

Configurator .deb (optional):
```bash
wget -O wayscriber-configurator-amd64.deb https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-configurator-amd64.deb
sudo apt install ./wayscriber-configurator-amd64.deb
```

Configurator .rpm (optional):
```bash
wget -O wayscriber-configurator-x86_64.rpm https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-configurator-x86_64.rpm
sudo dnf install ./wayscriber-configurator-x86_64.rpm
```

</details>

### From source

Rust 1.95 or newer is required. If `rustup` is not already installed, install it (this also installs `cargo`), then load Cargo into your current shell:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Then clone the repository — the dependency and build steps below run from inside it:

```bash
git clone https://github.com/devmobasa/wayscriber.git
cd wayscriber
rustup toolchain install 1.95.0
rustup override set 1.95.0
cargo --version
```

The override selects Rust 1.95 only inside this checkout and leaves your global default unchanged. You can instead keep any existing default that is already Rust 1.95 or newer.

**Dependencies:**

```bash
# Debian 13+ / Ubuntu 25.04+ (including Ubuntu 26.04 LTS)
sudo apt-get install build-essential pkg-config libcairo2-dev libwayland-dev libpango1.0-dev libgtk-4-dev libgtk4-layer-shell-dev

# Fedora
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config cairo-devel wayland-devel pango-devel libxkbcommon-devel cairo-gobject-devel gtk4-devel gtk4-layer-shell-devel
```

On **Ubuntu 24.04 LTS and Mint 22** there is no `libgtk4-layer-shell-dev` package, but GTK itself is new enough. Either build the small layer-shell library from the pinned source (needs `meson ninja-build wayland-protocols`, installs into `/usr`):

```bash
sudo apt-get install meson ninja-build wayland-protocols
bash tools/install-gtk4-layer-shell.sh   # builds + installs gtk4-layer-shell 1.3.0
```

…or skip it and use the GTK-less build option below. That keeps the built-in Cairo toolbars and every other default feature.

**Pop!_OS 22.04 ships GTK 4.6, below the GTK 4.12 API required by the default build.** Building only gtk4-layer-shell is therefore not enough there: use the GTK-less build below, unless you separately install GTK 4.12 or newer.

**Build:**

With the GTK toolbar frontend (the default):

```bash
cargo build --release
# Binary: target/release/wayscriber
```

Without the GTK toolbar frontend:

```bash
cargo build --release --no-default-features --features tablet-input,portal,tray
# Binary: target/release/wayscriber
```

**Optional install script:**
```bash
./tools/install.sh
```

### Screenshot tools

`wl-clipboard`, `grim`, and `slurp` are installed automatically by deb/rpm/AUR packages.
If you build from source or use the tarball, install them manually:

```bash
sudo apt-get install wl-clipboard grim slurp   # Debian/Ubuntu
sudo dnf install wl-clipboard grim slurp       # Fedora
```

---

## First launch

After installing:

```bash
wayscriber --version
wayscriber --active
```

(If you used `nix run` instead of installing, `wayscriber` is not on your `PATH` — keep using the `nix run` command from [Installation](#nixos-and-nix).)

Once the overlay is up:

- <kbd>F1</kbd> or <kbd>F10</kbd> — help overlay
- <kbd>Shift+F1</kbd> — quick reference
- <kbd>Ctrl+K</kbd> / <kbd>Ctrl+Shift+P</kbd> — command palette
- <kbd>F11</kbd> — configurator
- <kbd>Escape</kbd> — hide or exit

For daily use, set up [daemon mode](#daemon-mode-recommended): toggles are faster and session state survives between activations.

---

## Usage

### Daemon mode (recommended)

Run wayscriber in the background and toggle it with a keybind. Recommended for daily use: faster toggle, better workflow, and session persistence.

**Enable the service:**

```bash
systemctl --user enable --now wayscriber.service
wayscriber --daemon-toggle
```

Prefer a GUI? Open `wayscriber-configurator`, go to the **Daemon** tab, and click **Install/Update Service**, then **Enable + Start**.

**Bind a toggle key:**

Bind `wayscriber --daemon-toggle` to a global shortcut in your compositor or desktop environment. The configurator's **Daemon** tab can also do this: set a shortcut and click **Apply Shortcut** (GNOME: writes a GNOME custom shortcut; KDE/Plasma: writes the systemd drop-in env `WAYSCRIBER_PORTAL_SHORTCUT` for portal global shortcuts).

Hyprland:
```conf
bind = SUPER, D, exec, wayscriber --daemon-toggle
```
Then reload your config:
```bash
hyprctl reload
```

GNOME (Ubuntu/Debian/Fedora):
1. Open `Settings -> Keyboard -> Keyboard Shortcuts`.
2. Scroll down to `Custom Shortcuts`, click `+`.
3. Name: `Wayscriber Toggle`.
4. Command: `wayscriber --daemon-toggle`.
5. Set a shortcut key (recommended on Ubuntu GNOME: <kbd>Super+G</kbd>; <kbd>Super+D</kbd> is often already in use).

KDE Plasma:
1. Open `Settings -> Keyboard -> Shortcuts`.
2. Add new `Command or Script`.
3. Name: `Wayscriber Toggle`.
4. Command: `wayscriber --daemon-toggle`.
5. Assign a key (for example <kbd>Meta+Shift+D</kbd>).

Other desktops/window managers: bind `wayscriber --daemon-toggle` to any global shortcut key you prefer.

> Use only one toggle binding. Duplicate entries (for example two `SUPER+D` binds, or a leftover one-shot bind on the same key) can fire twice and immediately undo the toggle.
> If your compositor shortcut environment does not resolve `wayscriber` from `PATH`, use the absolute path from `command -v wayscriber` instead.

**System tray:**

Right-click the tray icon for options. Less-frequent actions are grouped into
Drawing Modes, Capture, and Settings & Data submenus so the menu remains usable
on shorter or scaled displays:

- Toggle overlay visibility
- Freeze/unfreeze the current overlay
- Capture full screen / active window / region
- Toggle the help overlay
- Flip session resume on/off (writes to config)
- Clear saved session data
- Open the log folder
- Open configurator / open config file / quit

Supported desktops use a theme-adaptive symbolic tray icon. Hosts that do not reliably resolve named icons (including Noctalia/Quickshell/COSMIC) automatically receive scale-aware colored pixmaps, including a 48px HiDPI rendition. Set `[tray].icon_style` to `"auto"` (default), `"symbolic"`, or `"colored"` to choose the main tray icon style; restart the daemon after changing it. Use `--no-tray` or `WAYSCRIBER_NO_TRAY=1` if you don't have a system tray. If the tray icon is still blank or the menu shows square placeholders, start the daemon with `WAYSCRIBER_TRAY_FORCE_PIXMAP=1`; this environment override takes precedence over the TOML setting.

**Alternative — compositor autostart instead of systemd:**
```conf
exec-once = wayscriber --daemon
bind = SUPER, D, exec, wayscriber --daemon-toggle
```

**Service commands:**
```bash
systemctl --user status wayscriber.service
systemctl --user restart wayscriber.service
journalctl --user -u wayscriber.service -f
```

### One-shot mode (alternative)

Launch wayscriber when you need it, then exit:

```bash
wayscriber --active
wayscriber --active --mode whiteboard
wayscriber --active --mode blueprint
wayscriber --freeze   # start with screen frozen
```

`whiteboard` and `blackboard` are built in; `blueprint` is an example of a custom board defined in `config.toml` (`[[boards.items]]` — see [Configuration](#configuration)).

Bind to a key (Hyprland example):
```conf
bind = SUPER, D, exec, wayscriber --active
```

The same in-overlay shortcuts apply as in [First launch](#first-launch); <kbd>Escape</kbd> or <kbd>Ctrl+Q</kbd> exits.

### Light passthrough mode

Light passthrough (layer-shell compositors only) lets normal keyboard and pointer input reach the app underneath while wayscriber stays visible for drawing.

- <kbd>F6</kbd> enters passthrough from the focused overlay. It is a wayscriber in-overlay shortcut, not an OS/global shortcut — once passthrough is active, wayscriber may no longer receive that keypress.
- For reliable control (including getting back out of passthrough), bind compositor/global shortcuts to `wayscriber --light-toggle` and `wayscriber --light-draw-toggle`.
- Use `wayscriber --light-draw-on` on press and `wayscriber --light-draw-off` on release for draw-while-held shortcuts.
- Hyprland and KDE binding examples are in [docs/SETUP.md](docs/SETUP.md#light-passthrough-controls-on-hyprland); the KDE section follows the Hyprland binding example.
- **Stock GNOME Wayland does not support this mode** — regular app windows cannot provide the required click-through shell overlay. Freeze may still work for still-image capture, but it is not a live passthrough replacement. A GNOME Shell extension approach would be needed for true shell-level passthrough.

### Screenshots and export

| Shortcut | Action |
|----------|--------|
| <kbd>Ctrl+Alt+F</kbd> | Capture full screen (respects `capture.copy_to_clipboard`) |
| <kbd>Ctrl+Shift+O</kbd> | Capture active window (respects `capture.copy_to_clipboard`) |
| <kbd>Ctrl+Shift+I</kbd> | Capture selection (respects `capture.copy_to_clipboard`) |
| <kbd>Ctrl+C</kbd> | Copy full screen to clipboard |
| <kbd>Ctrl+S</kbd> | Save full screen as PNG |
| <kbd>Ctrl+Shift+C</kbd> | Select region → clipboard |
| <kbd>Ctrl+Shift+S</kbd> | Select region → save PNG |
| <kbd>Ctrl+6</kbd> | Region → clipboard (explicit) |
| <kbd>Ctrl+Alt+6</kbd> | Region → save PNG (explicit) |
| <kbd>Ctrl+Alt+O</kbd> | Open last capture folder |

Shortcuts marked "respects `capture.copy_to_clipboard`" send the capture to the clipboard or a file according to that `config.toml` setting; the other shortcuts always use the destination shown. Captures need the [screenshot tools](#screenshot-tools) and fall back to xdg-desktop-portal if they are missing.

Use `--exit-after-capture` / `--no-exit-after-capture` to override whether the overlay closes after a capture.

<details>
<summary>PDF export</summary>

Canvas export commands are available in the command palette and keybindings. `export_board_pdf_file` saves the active board as a multi-page PDF, `export_all_boards_pdf_file` saves every board in board order, and both PDF actions are unbound by default. PDF exports keep transparent pages blank unless `[export.pdf] transparent_background = "desktop"` is set, which captures the live desktop behind the overlay for transparent pages only.

</details>

---

## Getting help

- **Help overlay:** <kbd>F1</kbd> or <kbd>F10</kbd>
- **Quick reference:** <kbd>Shift+F1</kbd>
- **Command palette:** <kbd>Ctrl+K</kbd> or <kbd>Ctrl+Shift+P</kbd> (search `monitor` or `display` for output actions)
- **About window:** `wayscriber --about`
- **Full docs:** https://wayscriber.com/docs/

---

## Controls reference

Press <kbd>F1</kbd> for the complete in-app cheat sheet.

### Essential shortcuts

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
<summary>All drawing tools</summary>

| Action | Key/Mouse |
|--------|-----------|
| Freehand pen | Drag with left mouse button |
| Straight line | <kbd>Shift</kbd> + drag |
| Rectangle | <kbd>Ctrl</kbd> + drag |
| Ellipse/Circle | <kbd>Tab</kbd> + drag |
| Arrow | <kbd>Ctrl+Shift</kbd> + drag |
| Triangle / parallelogram / rhombus / regular polygon | Toolbar Polygons picker (bindable) |
| Freeform polygon | Toolbar Polygons picker, then click vertices; <kbd>Enter</kbd> or double-click to finish |
| Step marker tool | Toolbar (bindable) |
| Highlight brush | <kbd>Ctrl+Alt+H</kbd> |
| Text mode | <kbd>T</kbd>, <kbd>Click</kbd> to position, type, <kbd>Enter</kbd> to finish |
| Sticky note | <kbd>N</kbd>, <kbd>Click</kbd> to place, type, <kbd>Enter</kbd> to finish |

The polygon tools are available from the toolbar picker; their default keybindings are intentionally empty. Drag and mouse-button mappings are configurable — see [Drag-tool mappings](#drag-tool-mappings).

</details>

<details>
<summary>Boards</summary>

| Action | Key |
|--------|-----|
| Toggle Whiteboard | <kbd>Ctrl+W</kbd> |
| Toggle Blackboard | <kbd>Ctrl+B</kbd> |
| Return to Transparent | <kbd>Ctrl+Shift+T</kbd> |
| Switch board slot | <kbd>Ctrl+Shift+1..9</kbd> |
| Previous/next output | <kbd>Ctrl+Alt+Shift</kbd> + <kbd>←</kbd>/<kbd>→</kbd> |
| Previous/next board | <kbd>Ctrl+Shift</kbd> + <kbd>←</kbd>/<kbd>→</kbd> |
| New board | <kbd>Ctrl+Shift+N</kbd> |
| Delete board | <kbd>Ctrl+Shift+Delete</kbd> |
| Board picker | <kbd>Ctrl+Shift+B</kbd> |
| Pan solid boards | Hold <kbd>Space</kbd> + left-drag |
| Reset solid-board pan | <kbd>Right-click</kbd> → Reset Canvas Position |

</details>

<details>
<summary>Colors and sizes</summary>

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

The first eight quick colors map to the shortcuts above and are customizable — see [Quick colors](#quick-colors).

Pick a color directly from the displayed desktop with the screen eyedropper: press <kbd>I</kbd>, use the eyedropper button in the toolbar or color picker, or search for **Pick screen color** in the command palette (<kbd>Ctrl+K</kbd>). Rebind it if you prefer another shortcut:

```toml
[keybindings.colors]
pick_screen_color = ["I"]
```

| Action | Key |
|--------|-----|
| Increase thickness | <kbd>+</kbd> / <kbd>=</kbd> / scroll down |
| Decrease thickness | <kbd>-</kbd> / <kbd>_</kbd> / scroll up |
| Increase font size | <kbd>Ctrl+Shift</kbd> + <kbd>+</kbd> / <kbd>Shift</kbd> + scroll down |
| Decrease font size | <kbd>Ctrl+Shift</kbd> + <kbd>-</kbd> / <kbd>Shift</kbd> + scroll up |
| Increase marker opacity | <kbd>Ctrl+Alt</kbd> + <kbd>↑</kbd> |
| Decrease marker opacity | <kbd>Ctrl+Alt</kbd> + <kbd>↓</kbd> |

</details>

<details>
<summary>Selection and arrange</summary>

| Action | Key |
|--------|-----|
| Duplicate selection | <kbd>Ctrl+D</kbd> |
| Copy selection | <kbd>Ctrl+Alt+C</kbd> |
| Paste selection or copied PNG/JPEG image | <kbd>Ctrl+Alt+V</kbd> |
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
<summary>Editing and UI</summary>

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
| Command palette | <kbd>Ctrl+K</kbd> / <kbd>Ctrl+Shift+P</kbd> |
| Screen eyedropper | <kbd>I</kbd>, toolbar/color picker, or <kbd>Ctrl+K</kbd> → **Pick screen color** |
| Radial menu | <kbd>Middle-click</kbd> (idle) open/close; <kbd>Left-click</kbd> select; <kbd>Right-click</kbd>/<kbd>Escape</kbd> dismiss; scroll adjusts active tool size |
| Status bar | <kbd>F4</kbd> / <kbd>F12</kbd> |
| Apply preset slot | <kbd>1</kbd> - <kbd>5</kbd> |
| Save preset slot | <kbd>Shift+1</kbd> - <kbd>Shift+5</kbd> |
| Toggle click highlight | <kbd>Ctrl+Shift+H</kbd> |
| Toggle light passthrough (in-overlay) | <kbd>F6</kbd> (see [Light passthrough mode](#light-passthrough-mode)) |
| Reset arrow labels | <kbd>Ctrl+Shift+R</kbd> |
| Toggle freeze | <kbd>Ctrl+Shift+F</kbd> |
| Zoom in/out | <kbd>Ctrl+Alt</kbd> + scroll / <kbd>Ctrl+Alt</kbd> + <kbd>+</kbd> / <kbd>Ctrl+Alt</kbd> + <kbd>-</kbd> |
| Reset zoom | <kbd>Ctrl+Alt+0</kbd> |
| Toggle zoom lock | <kbd>Ctrl+Alt+L</kbd> |
| Pan zoom view | <kbd>Middle drag</kbd> / <kbd>Arrow keys</kbd> |
| Exit | <kbd>Escape</kbd> / <kbd>Ctrl+Q</kbd> |

</details>

Notes:

- Arrow labels can auto-number when enabled in the arrow toolbar; reset with <kbd>Ctrl+Shift+R</kbd>.
- Step markers auto-increment and reset from the toolbar (or bind `reset_step_markers` in `config.toml`).
- Preset slots can be saved/cleared from the toolbar; edit names and advanced fields in `config.toml`.
- The blur tool has no default keyboard shortcut; bind `select_blur_tool` in `config.toml` if you want direct keyboard access.

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

See `docs/CONFIG.md` and https://wayscriber.com/docs/ for the full reference.

### Key sections

```toml
[drawing]
default_color = "red"
default_thickness = 3.0
polygon_sides = 5

[[drawing.quick_colors]]
label = "Blush"
color = "#FFB3BA"

[drawing.drag_tools.right]
drag_tool = "pen"
drag_color = "blue"

[presets]
# slot_count: 3-5
slot_count = 5

[presets.slot_1]
name = "Red pen"
tool = "pen"
color = "red"
size = 3.0

[performance]
# vsync and fps caps — see Performance tuning under Troubleshooting

[ui]
# status bar visibility and position

[boards]
# named boards + backgrounds
```

### Drag-tool mappings

Drag modifier mappings are configurable via `[drawing]` (`drag_tool`, `shift_drag_tool`, `ctrl_drag_tool`, `ctrl_shift_drag_tool`, `tab_drag_tool`) or in the configurator Drawing tab. For per-button workflows, use `[drawing.drag_tools.left]`, `[drawing.drag_tools.right]`, and `[drawing.drag_tools.middle]`; each binding can set a tool and optional color. The polygon tools have intentionally empty default keybindings — select them from the toolbar picker or bind them yourself. Freeform polygon is selectable but not drag-bindable.

### Quick colors

The quick color palette is configurable with ordered `[[drawing.quick_colors]]` entries. The first eight entries map to the <kbd>R</kbd>/<kbd>G</kbd>/<kbd>B</kbd>/<kbd>Y</kbd>/<kbd>O</kbd>/<kbd>P</kbd>/<kbd>W</kbd>/<kbd>K</kbd> shortcuts; if fewer are configured by hand, missing shortcut positions use the built-in defaults. The implicit default toolbar palette also preserves Cyan, Purple, and Gray as expanded toolbar colors while the radial menu keeps its original first-eight color ring. Explicit entries beyond the first eight have no shortcut action binding and opt those extra colors into dense palette UIs, capped to the first 24 colors.

### Session manager and persistence

Session persistence is enabled by default. Manage it via the configurator (<kbd>F11</kbd> → Session tab), CLI flags, or the tray checkmark (writes to config).

```bash
wayscriber --resume-session      # force resume (persist/restore all boards + history/tool state)
wayscriber --no-resume-session   # disable resume for this run
wayscriber --session-info        # inspect saved sessions
wayscriber --clear-session       # remove stored boards
wayscriber --clear-tool-state    # reset saved tool defaults, keep boards/history

# Named session files — --session-file combines with any of the flags above
# and with --active / --freeze / --daemon:
wayscriber --active --session-file ~/Documents/lecture-04.wayscriber-session
wayscriber --daemon-toggle --session-file ~/Documents/meeting.wayscriber-session
```

See [Session manager examples](examples/session-manager.md) for complete CLI, overlay, and configurator workflows.

<details>
<summary>Behavior notes</summary>

- Config values seed startup defaults. When `restore_tool_state` is enabled (default), the last-used tool settings saved in the session (including arrow head placement) override those config defaults on startup. Run `wayscriber --clear-tool-state` to remove only that saved tool layer so config defaults apply next startup while saved boards/history remain. In a running overlay, use Command Palette → Reset Tool Defaults to clear the saved layer and immediately apply config defaults to the active tools.
- `--session-file` uses exactly the selected file, implies persistence for that overlay run, rejects directories/symlinks/special files, and does not create missing parent directories. A running daemon can launch a hidden overlay with a named target; if the overlay is already visible, hide it before switching to a different named session.
- The overlay Session panel lives in the side toolbar's Settings drawer. It can open an existing named session, save the current overlay as another named session, show session info, clear the active session, reopen recent named sessions, and jump to the configurator. The Open/Save As dialogs use `zenity` or `kdialog`; Save As appends `.wayscriber-session` when no extension is supplied and asks before replacing existing session artifacts.
- The configurator Session tab manages recent named sessions recorded when named-session targets are opened or saved from the CLI, daemon, or overlay. It can rename catalog labels, reveal files, and forget metadata without touching files. Clear Tool State removes only the saved tool layer; Clear Saved Data removes session files. Duplicate, Move, Clear Tool State, and Clear are disabled while an overlay, manually started daemon, or background service is active.

</details>

### Tablet and stylus support

Tablet support (`zwp_tablet_v2`) ships in default builds and is enabled at runtime by default:

```toml
[tablet]
enabled = true
pressure_enabled = true
min_thickness = 1.0
max_thickness = 8.0
```

It works out of the box in default builds. Set `[tablet].enabled = false` in `config.toml` to opt out. To build without tablet support, drop only that feature (bare `--no-default-features` would also strip portal capture, tray, and the GTK toolbars): `cargo build --release --no-default-features --features portal,tray,toolbar-gtk`.

---

## Troubleshooting

### Daemon not starting after reboot

Enabled user services start when you log in, not at boot — if the daemon is missing after a reboot and login, see [Service won't start](#service-wont-start). Enable lingering only if you want the daemon started before login or kept running after logout:
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

### Overlay is blurry/non-transparent on KDE Plasma

**Cause:** The "Better Blur DX" effect (or similar blur effects) may blur wayscriber's transparent overlay.

**Solution (Option 1 — configure Better Blur DX):**
1. Open **System Settings** → **Window Management** → **Desktop Effects**
2. Click the configure button next to "Better Blur DX"
3. Go to the **Force Blur** tab
4. Add `wayscriber` to the window class list
5. Make sure `Blur all except matching` is selected
6. Click **Apply**

**Solution (Option 2 — use standard blur):**
1. Disable "Better Blur DX" in **Desktop Effects**
2. Enable the standard "Blur" effect instead

To find wayscriber's window class, run this and launch wayscriber before the 2-second sleep ends:
```bash
sleep 2; qdbus org.kde.KWin /KWin queryWindowInfo | grep resourceClass
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
- `WAYSCRIBER_TOOLBAR_BACKEND=auto|gtk|builtin` overrides the toolbar frontend (default: auto)
- `WAYSCRIBER_NO_TRAY=1` disables the tray icon (default: tray enabled)
- `WAYSCRIBER_TRAY_FORCE_PIXMAP=1` forces colored tray pixmaps and overrides `[tray].icon_style`

See `docs/CONFIG.md` for the full list.

### Performance tuning

Default behavior prioritizes lower drawing latency by disabling vsync and capping no-vsync rendering:

```toml
[performance]
buffer_count = 3
enable_vsync = false
max_fps_no_vsync = 120
ui_animation_fps = 30
```

Use `120` as a strong low-latency cap for common systems. Try `144`, `165`, `240`, or higher if it matches your display and the machine handles the extra rendering work. Use `max_fps_no_vsync = 0` only for profiling because uncapped rendering can spin CPU/GPU hard.

Set `enable_vsync = true` when tear-free presentation, lower power use, or quieter behavior matters more than input latency. Vsync usually adds a frame-cadence floor, especially on 60 Hz displays; disabling it improves input latency but may allow tearing and higher CPU/GPU usage.

---

## Contributing

Contribution principles: wayscriber is shared as a gift exchange, not a contract. Requests are welcome, but there is no guaranteed timeline or support obligation. For the full principles, see https://wayscriber.com/docs/ethos/gift-exchange.html

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, project structure, and workflow notes.

## Roadmap

- [x] Native Wayland layer-shell
- [x] Daemon mode with system tray
- [x] Multiple customizable boards/backgrounds
- [x] Session manager and persistence (named sessions, overlay actions, configurator catalog, CLI override, tray config toggle)
- [x] Highlighter & eraser tools
- [x] Additional shapes (filled shapes)
- [x] Selection tools & properties panel
- [x] Blur tool
- [x] Tablet/stylus support with pressure
- [x] Color picker
- [x] Screen eyedropper: pick a draw color from the screen
- [x] Render color profiles
- [x] Zoom (ZoomIt-style controls)
- [x] Presets (tool/color/size slots)
- [x] Sticky notes
- [x] Board pages (multi-page boards)
- [x] Presenter mode
- [x] Click highlights
- [x] Screen freeze
- [x] Light passthrough mode
- [x] Command palette
- [x] Radial menu
- [x] Numbered callouts (incrementing arrow labels)
- [x] Multi-monitor support
- [x] Save annotations to image
- [x] Multi-page board PDF export

Future plans are tracked in [GitHub issues](https://github.com/devmobasa/wayscriber/issues).

---

## License and credits

**MIT License** — see [LICENSE](LICENSE)

### Acknowledgments

- Inspired by [ZoomIt](https://learn.microsoft.com/en-us/sysinternals/downloads/zoomit) by Mark Russinovich
- Built for Linux (distros that use Wayland)
- Similar ideas from [Gromit-MPX](https://github.com/bk138/gromit-mpx)
- Uses [Cairo](https://www.cairographics.org/) and [smithay-client-toolkit](https://github.com/Smithay/client-toolkit)

Developed with AI assistance (ChatGPT, Codex, Claude Code).
