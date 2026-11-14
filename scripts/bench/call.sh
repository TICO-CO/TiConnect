#!/usr/bin/env bash
set -euo pipefail

peer="${1:?usage: call.sh PEER_ID SECONDS}"
seconds="${2:-15}"

call_id="$(ticonnect-cli call start "$peer" | tr -d '[:space:]')"
echo "call_id=$call_id"
sleep "$seconds"
pid="$(pgrep -n ticonnectd || true)"
if [ -n "$pid" ]; then
    ps -o pid,rss,%cpu,etime,comm -p "$pid"
fi
ticonnect-cli call hangup "$call_id"
