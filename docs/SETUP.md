# Complete Setup Guide

## Installation

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

### Method 1: Daemon Mode with Toggle (Recommended)

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

Now press **Super+D** to toggle the overlay on/off!

### Method 2: One-Shot Mode (Alternative)

For quick one-time annotations without daemon:

```bash
# Run directly (not recommended - daemon mode is better)
wayscriber --active
```

This starts a fresh overlay each time. Exit with Escape.

**Note:** We recommend using daemon mode with Super+D instead as it preserves your drawings.

## Usage Flow

### Daemon Mode Workflow (Recommended)

1. **Daemon starts automatically** → Runs in background with system tray icon
2. **Press Super+D** → Drawing overlay appears
3. **Draw your annotations** → All tools available
4. **Press Escape or Ctrl+Q** → Overlay hides (daemon keeps running)
5. **Press Super+D again** → Overlay reappears with previous drawings intact

No system tray/StatusNotifier watcher? Start the daemon with `wayscriber --daemon --no-tray` (or set `WAYSCRIBER_NO_TRAY=1`) to skip the tray icon; the Super+D toggle still works.

### One-Shot Mode Workflow (Alternative)

1. **Run command** → Fresh drawing overlay appears
2. **Draw your annotations** → All tools available
3. **Press Escape** → Drawing overlay closes completely
4. **Run command again** → New fresh overlay (previous drawings lost)

**Note:** Daemon mode with Super+D is recommended as it preserves your drawings when you toggle the overlay.

## Verification

Test the setup:

```bash
# Test binary is accessible
which wayscriber

# Test daemon mode
wayscriber --daemon &

# Test keybind
Press Super+D (should show overlay)
Press Escape (should hide overlay)
```

## Autostart

Daemon mode is already included in Method 1! The `exec-once` line will start wayscriber automatically on login.

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
  - `SUPER, D` → Super+D
  - `ALT, D` → Alt+D
  - `CTRL SHIFT, 2` → Ctrl+Shift+2

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
4. Use: Press Super+D to toggle overlay

Done! Drawings persist, tray icon available. ✨
