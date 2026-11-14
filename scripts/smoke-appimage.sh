#!/usr/bin/env bash
set -euo pipefail

[ "$#" -eq 1 ] || {
    echo "usage: $0 <AppImage>" >&2
    exit 2
}

APPIMAGE="$(realpath "$1")"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/ticonnect-appimage-smoke.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

export TICONNECT_CONFIG_DIR="$WORK/config"
export TICONNECT_DATA_DIR="$WORK/data"
export TICONNECT_LOG_LEVEL=warn
export TICONNECT_APPIMAGE="$APPIMAGE"
mkdir -p "$TICONNECT_CONFIG_DIR" "$TICONNECT_DATA_DIR"

"$APPIMAGE" --appimage-extract-and-run --help >/dev/null

dbus-run-session -- bash -c '
set -euo pipefail
"$TICONNECT_APPIMAGE" --appimage-extract-and-run daemon &
pid=$!
for _ in $(seq 1 60); do
    "$TICONNECT_APPIMAGE" --appimage-extract-and-run cli ping >/dev/null 2>&1 && break
    sleep 0.1
done
"$TICONNECT_APPIMAGE" --appimage-extract-and-run cli status
"$TICONNECT_APPIMAGE" --appimage-extract-and-run cli quit
wait "$pid" || true
'
