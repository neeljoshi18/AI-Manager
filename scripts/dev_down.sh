#!/usr/bin/env bash
# Stop processes started by dev_up.sh (best-effort by port + pidfile).
set -euo pipefail
LOGDIR="${LOGDIR:-/tmp/ai-manager-dev}"
PIDFILE="$LOGDIR/pids"

echo "== AI Manager dev_down =="

if [[ -f "$PIDFILE" ]]; then
  while read -r pid; do
    [[ -z "$pid" ]] && continue
    kill "$pid" 2>/dev/null || true
  done < "$PIDFILE"
  rm -f "$PIDFILE"
fi

for p in 18080 18082 18083 18090; do
  if command -v lsof >/dev/null 2>&1; then
    pids=$(lsof -ti ":$p" 2>/dev/null || true)
    if [[ -n "${pids:-}" ]]; then
      # shellcheck disable=SC2086
      kill $pids 2>/dev/null || true
    fi
  fi
done

echo "Stopped listeners on 18080/18082/18083/18090 (if any)."
echo "If github_live_bridge.py remains: ps aux | grep github_live  then kill <pid>"
