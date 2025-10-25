#!/bin/bash
# Installation script for wayscriber

set -e

# Get the directory where the script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Get the project root (parent of tools/)
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

INSTALL_DIR="$HOME/.local/bin"
BINARY_NAME="wayscriber"
CONFIGURATOR_BINARY_NAME="wayscriber-configurator"
LEGACY_BINARY_NAME="hyprmarker"
LEGACY_CONFIGURATOR_BINARY_NAME="hyprmarker-configurator"
CONFIG_DIR="$HOME/.config/wayscriber"
HYPR_CONFIG="$HOME/.config/hypr/hyprland.conf"

echo "================================"
echo "   Wayscriber Installation"
echo "================================"
echo ""

# Ensure required binaries are built
echo "Building Wayscriber binaries (release)..."
(cd "$PROJECT_ROOT" && cargo build --release --bins)

echo "Building Wayscriber configurator binaries (release)..."
(cd "$PROJECT_ROOT" && cargo build --release --bins --manifest-path configurator/Cargo.toml --target-dir target)

if [ ! -f "$PROJECT_ROOT/target/release/$CONFIGURATOR_BINARY_NAME" ] \
   && [ -f "$PROJECT_ROOT/configurator/target/release/$CONFIGURATOR_BINARY_NAME" ]; then
    mkdir -p "$PROJECT_ROOT/target/release"
    cp "$PROJECT_ROOT/configurator/target/release/$CONFIGURATOR_BINARY_NAME" \
       "$PROJECT_ROOT/target/release/$CONFIGURATOR_BINARY_NAME"
fi

# Create install directory if needed
echo "Creating installation directory: $INSTALL_DIR"
mkdir -p "$INSTALL_DIR"

# Copy binary
echo "Installing binary to $INSTALL_DIR/$BINARY_NAME"
cp "$PROJECT_ROOT/target/release/$BINARY_NAME" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$BINARY_NAME"

echo "Installing legacy compatibility alias to $INSTALL_DIR/$LEGACY_BINARY_NAME"
cp "$PROJECT_ROOT/target/release/$LEGACY_BINARY_NAME" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$LEGACY_BINARY_NAME"

echo "Installing configurator to $INSTALL_DIR/$CONFIGURATOR_BINARY_NAME"
cp "$PROJECT_ROOT/target/release/$CONFIGURATOR_BINARY_NAME" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$CONFIGURATOR_BINARY_NAME"

echo "Installing configurator alias to $INSTALL_DIR/$LEGACY_CONFIGURATOR_BINARY_NAME"
cp "$PROJECT_ROOT/target/release/$LEGACY_CONFIGURATOR_BINARY_NAME" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$LEGACY_CONFIGURATOR_BINARY_NAME"

# Check if install directory is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "⚠️  Warning: $INSTALL_DIR is not in your PATH"
    echo "   Add this line to your ~/.bashrc or ~/.zshrc:"
    echo "   export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo ""
fi

# Create config directory
echo "Creating config directory: $CONFIG_DIR"
mkdir -p "$CONFIG_DIR"

# Copy example config if config doesn't exist
if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    if [ -f "$PROJECT_ROOT/config.example.toml" ]; then
        echo "Installing example config to $CONFIG_DIR/config.toml"
        cp "$PROJECT_ROOT/config.example.toml" "$CONFIG_DIR/config.toml"
    fi
fi

echo ""
echo "✅ Installation complete!"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Setup Instructions"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "1. Test the installation:"
echo "   $BINARY_NAME --help"
echo ""
echo "2. Run in daemon mode (recommended):"
echo "   $BINARY_NAME --daemon &"
echo ""
echo "3. For Hyprland integration, add to $HYPR_CONFIG:"
echo ""
echo "   # Autostart wayscriber daemon"
echo "   exec-once = $INSTALL_DIR/$BINARY_NAME --daemon"
echo ""
echo "   # Toggle overlay with Super+D"
echo "   bind = SUPER, D, exec, pkill -SIGUSR1 $BINARY_NAME"
echo ""

# Setup autostart options
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Autostart Setup"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Choose autostart method:"
echo "  1) Systemd user service (recommended - runs on login)"
echo "  2) Hyprland exec-once (Hyprland only)"
echo "  3) Skip autostart setup"
echo ""
read -p "Enter choice [1-3]: " -n 1 -r
echo ""
echo ""

case $REPLY in
    1)
        # Systemd user service
        SYSTEMD_DIR="$HOME/.config/systemd/user"
        SERVICE_FILE="$SYSTEMD_DIR/wayscriber.service"

        echo "Setting up systemd user service..."
        mkdir -p "$SYSTEMD_DIR"

        if [ -f "$PROJECT_ROOT/packaging/wayscriber.service" ]; then
            cp "$PROJECT_ROOT/packaging/wayscriber.service" "$SERVICE_FILE"
            sed -i "s|ExecStart=/usr/bin/wayscriber --daemon|ExecStart=$INSTALL_DIR/$BINARY_NAME --daemon|" "$SERVICE_FILE"
            sed -i "s|Environment=\"PATH=/usr/local/bin:/usr/bin:/bin\"|Environment=\"PATH=$INSTALL_DIR:/usr/local/bin:/usr/bin:/bin\"|" "$SERVICE_FILE"
            echo "✅ Service file installed to $SERVICE_FILE"

            # Enable and start the service
            systemctl --user daemon-reload
            systemctl --user enable wayscriber.service
            systemctl --user start wayscriber.service

            echo "✅ Service enabled and started"
            echo ""
            echo "Service status:"
            systemctl --user status wayscriber.service --no-pager -l
            echo ""
            echo "Commands:"
            echo "  Start:   systemctl --user start wayscriber"
            echo "  Stop:    systemctl --user stop wayscriber"
            echo "  Status:  systemctl --user status wayscriber"
            echo "  Logs:    journalctl --user -u wayscriber -f"
        else
            echo "⚠️  Service file not found. Please run installer from repository root."
        fi

        # Still add Hyprland keybind if config exists
        if [ -f "$HYPR_CONFIG" ]; then
            echo ""
            read -p "Add Super+D keybind to Hyprland config? (y/n) " -n 1 -r
            echo ""
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                if grep -q "pkill -SIGUSR1 $BINARY_NAME" "$HYPR_CONFIG"; then
                    echo "⚠️  Keybind already configured"
                else
                    echo "" >> "$HYPR_CONFIG"
                    echo "# wayscriber toggle keybind" >> "$HYPR_CONFIG"
                    echo "bind = SUPER, D, exec, pkill -SIGUSR1 $BINARY_NAME" >> "$HYPR_CONFIG"
                    echo "✅ Keybind added to Hyprland config"
                    echo ""
                    echo "Reload Hyprland: hyprctl reload"
                fi
            fi
        fi
        ;;

    2)
        # Hyprland exec-once
        if [ -f "$HYPR_CONFIG" ]; then
            echo "Adding to Hyprland config..."
            if grep -q "wayscriber --daemon" "$HYPR_CONFIG"; then
                echo "⚠️  wayscriber already configured in Hyprland config"
            else
                echo "" >> "$HYPR_CONFIG"
                echo "# wayscriber - Screen annotation tool" >> "$HYPR_CONFIG"
                echo "exec-once = $INSTALL_DIR/$BINARY_NAME --daemon" >> "$HYPR_CONFIG"
                echo "bind = SUPER, D, exec, pkill -SIGUSR1 $BINARY_NAME" >> "$HYPR_CONFIG"
                echo "✅ Added to Hyprland config"
            fi
            echo ""
            echo "Reload Hyprland to activate:"
            echo "  hyprctl reload"
        else
            echo "⚠️  Hyprland config not found at $HYPR_CONFIG"
            echo "Add these lines manually to your Hyprland config:"
            echo "  exec-once = $INSTALL_DIR/$BINARY_NAME --daemon"
            echo "  bind = SUPER, D, exec, pkill -SIGUSR1 $BINARY_NAME"
        fi
        ;;

    3)
        echo "Skipping autostart setup."
        echo "To start manually: $BINARY_NAME --daemon &"
        ;;

    *)
        echo "Invalid choice. Skipping autostart setup."
        ;;
esac

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Usage"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Daemon mode (background, toggle with Super+D):"
echo "  $BINARY_NAME --daemon"
echo ""
echo "One-shot mode (overlay shows immediately):"
echo "  $BINARY_NAME --active"
echo ""
echo "Controls:"
echo "  - Freehand: Drag mouse"
echo "  - Line: Shift + drag"
echo "  - Rectangle: Ctrl + drag"
echo "  - Ellipse: Tab + drag"
echo "  - Arrow: Ctrl+Shift + drag"
echo "  - Text: Press T"
echo "  - Colors: R/G/B/Y/O/P/W/K"
echo "  - Thickness: +/- or scroll wheel"
echo "  - Help: F10"
echo "  - Launch configurator: F11"
echo "  - Undo: Ctrl+Z"
echo "  - Clear: E"
echo "  - Exit: Escape"
echo ""
echo "Configuration:"
echo "  Edit: $CONFIG_DIR/config.toml"
echo ""
echo "Documentation:"
echo "  docs/SETUP.md"
echo "  docs/CONFIG.md"
echo "  docs/QUICKSTART.md"
echo ""
echo "Happy annotating! 🎨"
echo ""
