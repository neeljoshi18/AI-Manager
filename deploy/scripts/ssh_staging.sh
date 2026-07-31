#!/usr/bin/env bash
# SSH to staging trying common paths when campus blocks port 22.
# Usage:
#   ./deploy/scripts/ssh_staging.sh
#   ./deploy/scripts/ssh_staging.sh 'docker ps'
set -euo pipefail
HOST="${STAGING_HOST:-206.189.129.31}"
USER="${STAGING_USER:-neel}"
KEY="${STAGING_SSH_KEY_FILE:-$HOME/.ssh/id_ed25519}"
REMOTE_CMD="${*:-}"

try_ssh() {
  local port="$1"
  shift
  ssh -o ConnectTimeout=8 -o BatchMode=yes -o StrictHostKeyChecking=accept-new \
    -i "$KEY" -p "$port" "${USER}@${HOST}" "$@"
}

echo "Trying SSH ports (22 = normal, 2222 = campus-friendly alt)…"
for port in 22 2222 443; do
  if try_ssh "$port" "echo OK_PORT_$port" 2>/dev/null; then
    echo "Connected on port $port"
    if [[ -n "$REMOTE_CMD" ]]; then
      try_ssh "$port" "$REMOTE_CMD"
    else
      # Interactive
      ssh -o StrictHostKeyChecking=accept-new -i "$KEY" -p "$port" "${USER}@${HOST}"
    fi
    exit 0
  fi
  echo "  port $port: failed"
done

echo "All SSH ports timed out."
echo "Campus Wi‑Fi likely blocks SSH. Options:"
echo "  1) git push origin main  → GitHub Actions deploy (see deploy/scripts/setup_ssh_via_https_port.md)"
echo "  2) Switch to mobile hotspot and retry"
echo "  3) Configure sshd Port 2222 on droplet (one-time hotspot)"
exit 1
