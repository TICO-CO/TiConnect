#!/usr/bin/env bash
set -euo pipefail

bin="${1:-target/release/ticonnectd}"
start="$(date +%s%N)"
"$bin" --help >/dev/null 2>&1 || true
end="$(date +%s%N)"
echo "startup_ms=$(( (end - start) / 1000000 ))"
