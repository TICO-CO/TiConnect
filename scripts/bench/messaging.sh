#!/usr/bin/env bash
set -euo pipefail

peer="${1:?usage: messaging.sh PEER_ID [COUNT]}"
count="${2:-20}"

start="$(date +%s%N)"
for i in $(seq 1 "$count"); do
    ticonnect-cli send "$peer" "benchmark $i" >/dev/null
done
end="$(date +%s%N)"
ms=$(( (end - start) / 1000000 ))
echo "messages=$count total_ms=$ms avg_ms=$((ms / (count + 1)))"
