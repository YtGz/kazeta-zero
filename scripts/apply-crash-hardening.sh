#!/bin/bash
# Apply crash-loop hardening on a live system: rate-limit kazeta-session restarts,
# avoid poweroff on failure, and disable auto panic reboot.

set -euo pipefail

if [ "$EUID" -ne 0 ]; then
    echo "Run as root (use sudo)." >&2
    exit 1
fi

SESSION_OVERRIDE_DIR="/etc/systemd/system/kazeta-session.service.d"
SESSION_OVERRIDE_FILE="$SESSION_OVERRIDE_DIR/override.conf"
SYSCTL_FILE="/etc/sysctl.d/99-kazeta-no-panic-reboot.conf"

echo "Writing systemd override for kazeta-session..."
mkdir -p "$SESSION_OVERRIDE_DIR"
cat > "$SESSION_OVERRIDE_FILE" <<'EOF'
[Service]
Restart=on-failure
RestartSec=5

[Unit]
StartLimitIntervalSec=300
StartLimitBurst=3
StartLimitAction=none
OnFailure=multi-user.target
EOF

echo "Disabling automatic kernel panic reboot..."
cat > "$SYSCTL_FILE" <<'EOF'
# Keep kernel panics on screen; do not auto-reboot.
kernel.panic = 0
EOF
sysctl -p "$SYSCTL_FILE" >/dev/null

echo "Reloading systemd units..."
systemctl daemon-reload
systemctl reset-failed kazeta-session || true

echo "Hardening applied. On failure, kazeta-session will restart up to StartLimit,"
echo "then drop to multi-user target instead of rebooting."
