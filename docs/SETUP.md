# Complete Setup Guide

## Installation

### Package installs (recommended when available)

If you installed wayscriber via a package (deb/rpm/aur), enable the user service:

```bash
systemctl --user enable --now wayscriber.service
```

The service keeps the daemon running in the background; you only need a keybind to toggle the overlay.

### Quick Install

Run the install script:
```bash
./tools/install.sh
```

This will:
1. Build the release binary
2. Copy it to `~/.local/bin/wayscriber`
3. Tell you how to add Hyprland keybind

### Manual Install

If you prefer manual installation:

```bash
# Build
cargo build --release

# Copy to user bin
mkdir -p ~/.local/bin
cp target/release/wayscriber ~/.local/bin/
chmod +x ~/.local/bin/wayscriber

# Make sure ~/.local/bin is in your PATH
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

## Hyprland Keybind Setup

### Method 1: Systemd user service + toggle (preferred when available)

```bash
systemctl --user enable --now wayscriber.service
```

Add the toggle keybinding to `~/.config/hypr/hyprland.conf`:

```conf
# wayscriber - Screen annotation daemon (Super+D to toggle)
bind = SUPER, D, exec, pkill -SIGUSR1 wayscriber
```

### Method 2: Daemon autostart via compositor (no systemd)

Add to `~/.config/hypr/hyprland.conf`:

```conf
# wayscriber - Screen annotation daemon (Super+D to toggle)
exec-once = wayscriber --daemon
bind = SUPER, D, exec, pkill -SIGUSR1 wayscriber
```

Then reload:
```bash
hyprctl reload
```

Now press <kbd>Super+D</kbd> to toggle the overlay on/off!

### Method 3: One-Shot Mode (Alternative)

For quick one-time annotations without daemon:

```bash
# Run directly (not recommended - daemon mode is better)
wayscriber --active
```

This starts a fresh overlay each time. Exit with <kbd>Escape</kbd>.

**Note:** We recommend using daemon mode with <kbd>Super+D</kbd> instead as it preserves your drawings.

## Usage Flow

### Daemon Mode Workflow (Recommended)

1. **Daemon starts automatically** → Runs in background with system tray icon (systemd user service or compositor autostart)
2. **Press <kbd>Super+D</kbd>** → Drawing overlay appears
3. **Draw your annotations** → All tools available
4. **Press <kbd>Escape</kbd> or <kbd>Ctrl+Q</kbd>** → Overlay hides (daemon keeps running)
5. **Press <kbd>Super+D</kbd> again** → Overlay reappears with previous drawings intact

No system tray/StatusNotifier watcher? Start the daemon with `wayscriber --daemon --no-tray` (or set `WAYSCRIBER_NO_TRAY=1`) to skip the tray icon; the <kbd>Super+D</kbd> toggle still works.

### One-Shot Mode Workflow (Alternative)

1. **Run command** → Fresh drawing overlay appears
2. **Draw your annotations** → All tools available
3. **Press <kbd>Escape</kbd>** → Drawing overlay closes completely
4. **Run command again** → New fresh overlay (previous drawings lost)

**Note:** Daemon mode with <kbd>Super+D</kbd> is recommended as it preserves your drawings when you toggle the overlay.

## Verification

Test the setup:

```bash
# Test binary is accessible
which wayscriber

# Test daemon mode
systemctl --user status wayscriber.service || wayscriber --daemon &

# Test keybind
Press <kbd>Super+D</kbd> (should show overlay)
Press <kbd>Escape</kbd> (should hide overlay)
```

## Autostart

- If you enabled `wayscriber.service`, systemd handles autostart.
- If you used compositor autostart, the `exec-once` line starts wayscriber on login.

## Troubleshooting

**Keybind not working?**
- Check `hyprctl reload` was run
- Check for conflicts: `hyprctl binds | grep "SUPER, D"`
- Try a different key combo

**Binary not found?**
- Check PATH: `echo $PATH | grep .local/bin`
- Add to PATH if missing (see Manual Install)
- Restart terminal after PATH change

**Want different key?**
- Edit hyprland.conf
- Examples:
  - `SUPER, D` → <kbd>Super+D</kbd>
  - `ALT, D` → <kbd>Alt+D</kbd>
  - `CTRL SHIFT, 2` → <kbd>Ctrl+Shift+2</kbd>

## Uninstall

```bash
rm ~/.local/bin/wayscriber
# Remove keybind from hyprland.conf
```

## Recommended Setup

**Best setup (daemon mode):**

1. Install: `./tools/install.sh`
2. Add to hyprland.conf:
   ```conf
   exec-once = wayscriber --daemon
   bind = SUPER, D, exec, pkill -SIGUSR1 wayscriber
   ```
3. Reload: `hyprctl reload`
4. Use: Press <kbd>Super+D</kbd> to toggle overlay

Done! Drawings persist, tray icon available. ✨
