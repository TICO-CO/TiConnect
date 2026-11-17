#!/usr/bin/env bash
set -euo pipefail

user="${1:?usage: call.sh USER_ID SECONDS}"
seconds="${2:-15}"

call_id="$(ticonnect-cli call start "$user" | tr -d '[:space:]')"
echo "call_id=$call_id"
sleep "$seconds"
pid="$(pgrep -n ticonnectd || true)"
if [ -n "$pid" ]; then
    ps -o pid,rss,%cpu,etime,comm -p "$pid"
fi
ticonnect-cli call hangup "$call_id"
