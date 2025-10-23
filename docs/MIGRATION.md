# Migration Guide: hyprmarker → Wayscriber

This document tracks the rename of the project from **hyprmarker** to **Wayscriber** (introduced in v0.5.0). The goal is to make the transition smooth for existing users while communicating the new scope of the project: a compositor-agnostic annotation layer for Wayland.

## What's New

- Binary name is now `wayscriber`.
- Configurator binary is now `wayscriber-configurator`.
- Configuration lives under `~/.config/wayscriber/`.
- The systemd user unit installs as `wayscriber.service`.
- Project website and documentation live at [https://wayscriber.com](https://wayscriber.com).

## Migration Checklist

1. **Preview the config copy**
   ```bash
   wayscriber --migrate-config --dry-run
   ```
   Review the summary to confirm the source/destination paths.

2. **Copy the configuration**
   ```bash
   wayscriber --migrate-config
   ```
   This copies everything from `~/.config/hyprmarker/` to `~/.config/wayscriber/` and, if needed,
   backs up an existing Wayscriber config to a timestamped `wayscriber.backup.*` directory.

3. **Disable the legacy systemd service**
   ```bash
   systemctl --user disable --now hyprmarker.service 2>/dev/null || true
   ```
   This stops the old daemon and prevents it from starting on login.

   If you installed via a package manager, remove the legacy package before installing Wayscriber (e.g. `paru -R hyprmarker`).

4. **Update Hyprland keybindings (optional)**
   ```bash
   sed -i.bak 's/hyprmarker/wayscriber/g' ~/.config/hypr/hyprland.conf
   ```
   Inspect the `.bak` backup if you want to review the changes first.

After completing those steps, press `Super+D` (or your custom binding) to confirm the overlay opens. If you run into issues, check `journalctl --user -u wayscriber`.

## Compatibility Notes

- The package now ships a `hyprmarker` compatibility binary that forwards to `wayscriber` and prints a warning. Suppress the notice in scripts by setting `HYPRMARKER_SILENCE_RENAME=1`.
- Environment variable overrides for the configurator accept both `WAYSCRIBER_CONFIGURATOR` and the legacy `HYPRMARKER_CONFIGURATOR` names.
- Legacy configuration files remain untouched after the migration so you can roll back if needed.
- On Arch Linux, the `wayscriber` AUR package replaces `hyprmarker`. The legacy `hyprmarker` AUR entry now depends on `wayscriber` and will be retired after the grace period.

## Need Help?

Open an issue with the `rename` label on the repository or drop a message in the community channels. Include details about your setup (distro, compositor, install method) so we can reproduce problems quickly.
