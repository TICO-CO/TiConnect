#!/usr/bin/env bash
set -euo pipefail

peer="${1:?usage: remote.sh PEER_ID SECONDS}"
seconds="${2:-15}"

session="$(ticonnect-cli remote request "$peer")"
echo "$session"
session_id="$(printf '%s\n' "$session" | awk -F': ' '/session_id/ {print $2; exit}')"
echo "Enter the PIN shown on the host:"
read -r pin
ticonnect-cli remote accept "$session_id" "$pin"
sleep "$seconds"
pid="$(pgrep -n ticonnectd || true)"
if [ -n "$pid" ]; then
    ps -o pid,rss,%cpu,etime,comm -p "$pid"
fi
ticonnect-cli remote end "$session_id"
