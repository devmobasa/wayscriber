# Tools

Helper scripts for development and installation.

## Scripts

- **install.sh** - Installation script for wayscriber
  - Builds and installs binary to `~/.local/bin`
  - Sets up config directory
  - Optionally configures systemd or Hyprland autostart
  - Usage: `./tools/install.sh`

- **run.sh** - Quick run script for development
  - Runs wayscriber in daemon mode with debug logging
  - Usage: `./tools/run.sh`

- **reload-daemon.sh** - Reload running daemon
  - Kills and restarts the daemon (picks up config changes)
  - Usage: `./tools/reload-daemon.sh`

All scripts work from any location in the project.
