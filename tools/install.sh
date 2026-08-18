#!/bin/bash
# Installation script for wayscriber

set -e

# Get the directory where the script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Get the project root (parent of tools/)
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

INSTALL_DIR="${WAYSCRIBER_INSTALL_DIR:-/usr/bin}"
while [ "$INSTALL_DIR" != "/" ] && [ "${INSTALL_DIR%/}" != "$INSTALL_DIR" ]; do
    INSTALL_DIR="${INSTALL_DIR%/}"
done
case "$INSTALL_DIR" in
    /usr) INSTALL_DIR="/usr/bin" ;;
    /usr/local) INSTALL_DIR="/usr/local/bin" ;;
esac
BINARY_NAME="wayscriber"
INSTALLED_BINARY="$INSTALL_DIR/$BINARY_NAME"
BIND_COMMAND="$INSTALL_DIR/$BINARY_NAME --daemon-toggle"
CONFIG_DIR="$HOME/.config/wayscriber"
HYPR_CONFIG="$HOME/.config/hypr/hyprland.conf"
REPLACE_OTHER=0

die() {
    echo "❌ $*" >&2
    exit 1
}

usage() {
    echo "Usage: $0 [--replace-other]"
    echo "Build a source binary and install it to $INSTALL_DIR."
    echo "Refuses a second copy under /usr/bin, /usr/local/bin, or ~/.local/bin"
    echo "unless --replace-other is passed or you confirm on a TTY."
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --replace-other)
            [ "$REPLACE_OTHER" -eq 0 ] || die "--replace-other was specified more than once"
            REPLACE_OTHER=1
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            die "unexpected argument: $1"
            ;;
    esac
    shift
done

echo "================================"
echo "   Wayscriber Installation"
echo "================================"
echo ""

ensure_replacement() {
    local file="$1"
    local search="$2"
    local replacement="$3"
    local description="$4"

    if ! grep -q -- "$search" "$file"; then
        die "Expected pattern '$search' not found in $file while preparing $description."
    fi

    sed -i "s|$search|$replacement|" "$file"

    if ! grep -q -- "$replacement" "$file"; then
        die "Failed to set '$replacement' in $file for $description."
    fi
}

canonical_path() {
    readlink -f "$1" 2>/dev/null || printf '%s' "$1"
}

same_file() {
    local left right
    { [ -e "$1" ] || [ -L "$1" ]; } || return 1
    left="$(canonical_path "$1")"
    right="$(canonical_path "$2")"
    [ -n "$left" ] && { [ "$left" = "$right" ] || [ "$left" = "$2" ]; }
}

systemd_user_dir() {
    if [ -n "${XDG_CONFIG_HOME:-}" ]; then
        printf '%s\n' "${XDG_CONFIG_HOME}/systemd/user"
    elif [ -n "${HOME:-}" ]; then
        printf '%s\n' "${HOME}/.config/systemd/user"
    fi
}

known_wayscriber_binaries() {
    printf '%s\n' /usr/bin/wayscriber /usr/local/bin/wayscriber
    if [ -n "${HOME:-}" ]; then
        printf '%s\n' "$HOME/.local/bin/wayscriber"
    fi
}

known_prefix_units() {
    printf '%s\n' \
        /usr/lib/systemd/user/wayscriber.service \
        /usr/local/lib/systemd/user/wayscriber.service
}

user_unit_files() {
    local dir conf
    dir="$(systemd_user_dir)"
    [ -n "$dir" ] || return 0
    [ -f "$dir/wayscriber.service" ] && printf '%s\n' "$dir/wayscriber.service"
    if [ -d "$dir/wayscriber.service.d" ]; then
        for conf in "$dir/wayscriber.service.d/"*.conf; do
            [ -f "$conf" ] || continue
            printf '%s\n' "$conf"
        done
    fi
}

file_has_conflicting_exec_start() {
    local file="$1" dest="$2" dest_canon other
    [ -f "$file" ] || return 1
    grep -Eq '^[[:space:]]*ExecStart=' "$file" || return 1
    dest_canon="$(canonical_path "$dest")"
    if grep -E '^[[:space:]]*ExecStart=' "$file" | grep -Fq "$dest"; then
        return 1
    fi
    while IFS= read -r other; do
        grep -E '^[[:space:]]*ExecStart=' "$file" | grep -Fq "$other" || continue
        if same_file "$other" "$dest"; then
            return 1
        fi
        return 0
    done < <(known_wayscriber_binaries)
    return 0
}

unit_paths_for_binary() {
    case "$1" in
        /usr/local/bin/wayscriber)
            printf '%s\n' /usr/local/lib/systemd/user/wayscriber.service
            ;;
        /usr/bin/wayscriber)
            printf '%s\n' /usr/lib/systemd/user/wayscriber.service
            ;;
    esac
}

path_is_package_owned() {
    command -v pacman >/dev/null 2>&1 || return 1
    pacman -Qoq "$1" >/dev/null 2>&1
}

user_unit_conflicts() {
    local file
    while IFS= read -r file; do
        file_has_conflicting_exec_start "$file" "$INSTALLED_BINARY" && return 0
    done < <(user_unit_files)
    return 1
}

collect_conflicts() {
    OTHER_BINARIES=()
    OTHER_UNITS=()
    local dest other unit
    dest="$(canonical_path "$INSTALLED_BINARY")"
    while IFS= read -r other; do
        [ -e "$other" ] || [ -L "$other" ] || continue
        if [ "$other" = "$INSTALLED_BINARY" ] || [ "$(canonical_path "$other")" = "$dest" ]; then
            continue
        fi
        OTHER_BINARIES+=("$other")
    done < <(known_wayscriber_binaries)
    while IFS= read -r unit; do
        [ -e "$unit" ] || [ -L "$unit" ] || continue
        file_has_conflicting_exec_start "$unit" "$INSTALLED_BINARY" || continue
        OTHER_UNITS+=("$unit")
    done < <(known_prefix_units)
}

removal_paths_for_binary() {
    local binary="$1" unit
    printf '%s\n' "$binary"
    while IFS= read -r unit; do
        [ -e "$unit" ] || [ -L "$unit" ] || continue
        printf '%s\n' "$unit"
    done < <(unit_paths_for_binary "$binary")
}

remove_path_if_unmanaged() {
    local path="$1"
    if [ ! -e "$path" ] && [ ! -L "$path" ]; then
        return 0
    fi
    if path_is_package_owned "$path"; then
        if [ "$(basename "$path")" = "wayscriber" ]; then
            die "${path} is owned by Arch package $(pacman -Qoq "$path"); remove or update that package instead"
        fi
        echo "Leaving package-owned $path ($(pacman -Qoq "$path"))"
        return 0
    fi
    ${SUDO:-} rm -f -- "$path"
    echo "Removed $path"
}

remove_conflicting_other_binary() {
    local other="$1" path
    while IFS= read -r path; do
        remove_path_if_unmanaged "$path"
    done < <(removal_paths_for_binary "$other")
}

remove_conflicting_user_units() {
    local file
    while IFS= read -r file; do
        file_has_conflicting_exec_start "$file" "$INSTALLED_BINARY" || continue
        rm -f -- "$file"
        echo "Removed user unit override $file"
    done < <(user_unit_files)
}

has_install_conflicts() {
    [ "${#OTHER_BINARIES[@]}" -gt 0 ] || [ "${#OTHER_UNITS[@]}" -gt 0 ] || user_unit_conflicts
}

collect_conflicts
if has_install_conflicts; then
    echo "Another wayscriber install would stay beside $INSTALLED_BINARY:"
    for other in "${OTHER_BINARIES[@]+"${OTHER_BINARIES[@]}"}"; do
        echo "  $other"
        while IFS= read -r path; do
            echo "    and $path"
        done < <(removal_paths_for_binary "$other" | tail -n +2)
    done
    for unit in "${OTHER_UNITS[@]+"${OTHER_UNITS[@]}"}"; do
        echo "  $unit (ExecStart is not $INSTALLED_BINARY)"
    done
    while IFS= read -r file; do
        file_has_conflicting_exec_start "$file" "$INSTALLED_BINARY" || continue
        echo "  $file (ExecStart is not $INSTALLED_BINARY)"
    done < <(user_unit_files)
    echo "Leaving both copies makes the overlay daemon follow a different file than the one you inspect."
    if [ "$REPLACE_OTHER" -eq 0 ] && [ -t 0 ]; then
        read -r -p "Remove those files so only $INSTALLED_BINARY remains? [y/N] " CONFIRM
        echo ""
        case "$CONFIRM" in
            y|Y|yes|YES)
                REPLACE_OTHER=1
                ;;
            *)
                die "Refusing to install beside the other copy. Re-run with --replace-other after removing it."
                ;;
        esac
    elif [ "$REPLACE_OTHER" -eq 0 ]; then
        die "Refusing to install beside the other copy. Re-run with --replace-other, or remove it first."
    fi
fi

# Ensure required binaries are built
echo "Building Wayscriber binary (release, default features)..."
(cd "$PROJECT_ROOT" && cargo build --release --bins)

if [ ! -d "$INSTALL_DIR" ] || [ ! -w "$INSTALL_DIR" ]; then
    if [ -d "$INSTALL_DIR" ] && [ -w "$INSTALL_DIR" ]; then
        :
    else
        if [ "$(id -u)" -ne 0 ]; then
            if command -v sudo >/dev/null 2>&1; then
                SUDO="sudo"
                echo "Using sudo to install into $INSTALL_DIR"
            else
                die "Write access to $INSTALL_DIR required. Re-run with sudo or set WAYSCRIBER_INSTALL_DIR."
            fi
        fi
    fi
fi

if has_install_conflicts && [ "$REPLACE_OTHER" -eq 1 ] && [ "$(id -u)" -ne 0 ]; then
    if [ -z "${SUDO:-}" ]; then
        if command -v sudo >/dev/null 2>&1; then
            SUDO="sudo"
            echo "Using sudo to remove the other Wayscriber copy"
        else
            die "sudo is required to remove the other Wayscriber copy"
        fi
    fi
fi

if [ "$REPLACE_OTHER" -eq 1 ]; then
    for other in "${OTHER_BINARIES[@]+"${OTHER_BINARIES[@]}"}"; do
        remove_conflicting_other_binary "$other"
    done
    for unit in "${OTHER_UNITS[@]+"${OTHER_UNITS[@]}"}"; do
        remove_path_if_unmanaged "$unit"
    done
    remove_conflicting_user_units
fi

# Ensure install directory exists
${SUDO:-} install -d "$INSTALL_DIR"

# Copy binaries
echo "Installing binary to $INSTALL_DIR/$BINARY_NAME"
${SUDO:-} install -Dm755 "$PROJECT_ROOT/target/release/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"

if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "⚠️  Warning: $INSTALL_DIR is not in your PATH"
    echo "   Add this line to your shell config:"
    echo "   export PATH=\"$INSTALL_DIR:\$PATH\""
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
echo "  Binary check"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Installed: $INSTALLED_BINARY"
ls -l "$INSTALLED_BINARY"
echo "SHA256:  $(sha256sum "$INSTALLED_BINARY" | cut -d' ' -f1)"
if command -v "$BINARY_NAME" >/dev/null 2>&1; then
    PATH_BINARY="$(command -v "$BINARY_NAME")"
    echo "PATH:     $PATH_BINARY"
    if [ "$(readlink -f "$PATH_BINARY" 2>/dev/null || printf '%s' "$PATH_BINARY")" != \
        "$(readlink -f "$INSTALLED_BINARY" 2>/dev/null || printf '%s' "$INSTALLED_BINARY")" ]; then
        echo "⚠️  PATH resolves $PATH_BINARY, which is not the file just installed."
        echo "    Overlay spawn follows the running daemon, not this PATH lookup."
    fi
else
    echo "PATH:     not found in PATH"
fi
while IFS= read -r other; do
    [ -e "$other" ] || [ -L "$other" ] || continue
    if [ "$other" = "$INSTALLED_BINARY" ]; then
        continue
    fi
    if [ "$(canonical_path "$other")" = "$(canonical_path "$INSTALLED_BINARY")" ]; then
        continue
    fi
    echo "⚠️  $other still exists beside $INSTALLED_BINARY"
done < <(known_wayscriber_binaries)
while IFS= read -r unit; do
    [ -e "$unit" ] || [ -L "$unit" ] || continue
    file_has_conflicting_exec_start "$unit" "$INSTALLED_BINARY" || continue
    echo "⚠️  $unit still has ExecStart that is not $INSTALLED_BINARY"
done < <(known_prefix_units)
while IFS= read -r file; do
    file_has_conflicting_exec_start "$file" "$INSTALLED_BINARY" || continue
    echo "⚠️  $file still has ExecStart that is not $INSTALLED_BINARY"
done < <(user_unit_files)
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Setup Instructions"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "1. Test the installation:"
echo "   $INSTALLED_BINARY --help"
echo ""
echo "2. Run in daemon mode (recommended):"
echo "   $INSTALLED_BINARY --daemon &"
echo ""
echo "3. For Hyprland integration, add to $HYPR_CONFIG:"
echo ""
echo "   # Autostart wayscriber daemon"
echo "   exec-once = $INSTALLED_BINARY --daemon"
echo ""
echo "   # Toggle overlay with Super+D"
echo "   bind = SUPER, D, exec, $BIND_COMMAND"
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
        SYSTEMD_USER_DIR="$(systemd_user_dir)"
        SYSTEMD_SYSTEM_DIR="/usr/lib/systemd/user"
        USER_SERVICE_FILE="$SYSTEMD_USER_DIR/wayscriber.service"
        SYSTEM_SERVICE_FILE="$SYSTEMD_SYSTEM_DIR/wayscriber.service"

        echo "Setting up systemd user service..."

        if [ -f "$USER_SERVICE_FILE" ]; then
            echo "Removing old service override at $USER_SERVICE_FILE"
            rm -f "$USER_SERVICE_FILE"
        fi

        if [ -f "$PROJECT_ROOT/packaging/wayscriber.service" ]; then
            TARGET_SERVICE="$SYSTEM_SERVICE_FILE"
            TARGET_DIR="$SYSTEMD_SYSTEM_DIR"
            TARGET_SUDO="${SUDO:-}"

            if [ "$INSTALL_DIR" != "/usr/bin" ]; then
                TARGET_SERVICE="$USER_SERVICE_FILE"
                TARGET_DIR="$SYSTEMD_USER_DIR"
                TARGET_SUDO=""
            fi

            ${TARGET_SUDO} install -d "$TARGET_DIR"
            ${TARGET_SUDO} install -Dm644 "$PROJECT_ROOT/packaging/wayscriber.service" "$TARGET_SERVICE"

            if [ "$TARGET_SERVICE" = "$USER_SERVICE_FILE" ]; then
                ensure_replacement \
                    "$TARGET_SERVICE" \
                    "ExecStart=/usr/bin/wayscriber --daemon" \
                    "ExecStart=$INSTALLED_BINARY --daemon" \
                    "ExecStart override"

                ensure_replacement \
                    "$TARGET_SERVICE" \
                    "Environment=\"PATH=/usr/local/bin:/usr/bin:/bin\"" \
                    "Environment=\"PATH=$INSTALL_DIR:/usr/local/bin:/usr/bin:/bin\"" \
                    "PATH override"
            fi

            echo "✅ Service file installed to $TARGET_SERVICE"

            # Enable and start the service
            systemctl --user daemon-reload
            systemctl --user enable wayscriber.service
            if systemctl --user restart wayscriber.service; then
                echo "✅ Service restarted"
            else
                echo "⚠️  Restart failed; attempting start"
                systemctl --user start wayscriber.service
            fi

            echo "✅ Service enabled and started"
            echo ""
            echo "Service status:"
            systemctl --user status wayscriber.service --no-pager -l
            echo ""
            echo "Commands:"
            echo "  Restart: systemctl --user restart wayscriber.service"
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
                if grep -Fq "pkill -SIGUSR1 $BINARY_NAME" "$HYPR_CONFIG" \
                    || grep -Fq "$BINARY_NAME --daemon-toggle" "$HYPR_CONFIG" \
                    || grep -Fq "$BIND_COMMAND" "$HYPR_CONFIG"; then
                    echo "⚠️  Keybind already configured"
                else
                    echo "" >> "$HYPR_CONFIG"
                    echo "# wayscriber toggle keybind" >> "$HYPR_CONFIG"
                    echo "bind = SUPER, D, exec, $BIND_COMMAND" >> "$HYPR_CONFIG"
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
                echo "exec-once = $INSTALLED_BINARY --daemon" >> "$HYPR_CONFIG"
                echo "bind = SUPER, D, exec, $BIND_COMMAND" >> "$HYPR_CONFIG"
                echo "✅ Added to Hyprland config"
            fi
            echo ""
            echo "Reload Hyprland to activate:"
            echo "  hyprctl reload"
        else
            echo "⚠️  Hyprland config not found at $HYPR_CONFIG"
            echo "Add these lines manually to your Hyprland config:"
            echo "  exec-once = $INSTALLED_BINARY --daemon"
            echo "  bind = SUPER, D, exec, $BIND_COMMAND"
        fi
        ;;

    3)
        echo "Skipping autostart setup."
        echo "To start manually: $INSTALLED_BINARY --daemon &"
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
echo "Daemon mode (background; bind a shortcut such as Super+D):"
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
echo "  - Help: F1/F10"
echo "  - Toolbar: F2/F9"
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
echo ""
echo "Happy annotating! 🎨"
echo ""
