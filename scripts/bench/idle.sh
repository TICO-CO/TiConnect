#!/usr/bin/env bash
set -euo pipefail

pid="$(pgrep -n ticonnectd || true)"
if [ -z "$pid" ]; then
    echo "ticonnectd is not running" >&2
    exit 1
fi

echo "pid=$pid"
ps -o pid,rss,%cpu,etime,comm -p "$pid"
echo "rss_mb=$(ps -o rss= -p "$pid" | awk '{ printf "%.1f", $1 / 1024 }')"
