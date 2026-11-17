#!/usr/bin/env bash
set -euo pipefail

user="${1:?usage: messaging.sh USER_ID [COUNT]}"
count="${2:-20}"

start="$(date +%s%N)"
for i in $(seq 1 "$count"); do
    ticonnect-cli send "$user" "benchmark $i" >/dev/null
done
end="$(date +%s%N)"
ms=$(( (end - start) / 1000000 ))
echo "messages=$count total_ms=$ms avg_ms=$((ms / (count + 1)))"
