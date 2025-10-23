#!/bin/bash
# Reload wayscriber daemon
# This will kill the old daemon and start a new one with updated config

echo "Stopping wayscriber daemon..."
pkill wayscriber

# Wait a moment for clean shutdown
sleep 0.5

echo "Starting wayscriber daemon..."
wayscriber --daemon &

# Wait to verify it started
sleep 0.5

if pgrep -x wayscriber > /dev/null; then
    echo "✓ Daemon restarted successfully (PID: $(pgrep -x wayscriber))"
    echo "Press Super+D to toggle overlay"
else
    echo "✗ Failed to start daemon"
    exit 1
fi
