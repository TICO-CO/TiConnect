#!/usr/bin/env bash
set -euo pipefail

peer="${1:?usage: transfer.sh PEER_ID SIZE_MIB}"
size_mib="${2:-64}"
file="$(mktemp --suffix=.bin)"
trap 'rm -f "$file"' EXIT
head -c "$((size_mib * 1024 * 1024))" /dev/urandom > "$file"
start="$(date +%s%N)"
ticonnect-cli send-file "$peer" "$file"
end="$(date +%s%N)"
ms=$(( (end - start) / 1000000 ))
echo "bytes=$((size_mib * 1024 * 1024)) ms=$ms throughput_mib_s=$((size_mib * 1000 / (ms + 1)))"
