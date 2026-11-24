#!/usr/bin/env bash
set -euo pipefail

[ "$#" -eq 1 ] || {
    echo "usage: $0 <AppImage>" >&2
    exit 2
}

APPIMAGE="$(realpath "$1")"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/ticonnect-appimage-smoke.XXXXXX")"
XVFB_PID=""
cleanup() {
    if [ -n "$XVFB_PID" ]; then
        kill "$XVFB_PID" 2>/dev/null || true
    fi
    rm -rf "$WORK"
}
trap cleanup EXIT

export TICONNECT_CONFIG_DIR="$WORK/config"
export TICONNECT_DATA_DIR="$WORK/data"
export TICONNECT_LOG_LEVEL=warn
export TICONNECT_APPIMAGE="$APPIMAGE"
mkdir -p "$TICONNECT_CONFIG_DIR" "$TICONNECT_DATA_DIR"

"$APPIMAGE" --appimage-extract-and-run --version
help="$("$APPIMAGE" --appimage-extract-and-run --help)"
printf '%s\n' "$help" | grep -q "Launch TiConnect"
if printf '%s\n' "$help" | grep -Eq '^[[:space:]]*(daemon|overlay|cli)\b'; then
    echo "internal command exposed in user help" >&2
    exit 1
fi

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

doctor_json="$(TICONNECT_DOCTOR_BUS=present \
    TICONNECT_DOCTOR_DISPLAY=x11 \
    TICONNECT_DOCTOR_AUDIO=missing \
    TICONNECT_DOCTOR_GSTREAMER=missing \
    "$APPIMAGE" --appimage-extract-and-run doctor --json)"
printf '%s\n' "$doctor_json" | python3 -c '
import json
import sys

report = json.load(sys.stdin)
checks = {check["name"]: check["status"] for check in report["checks"]}
assert report["failed"] is False, report
assert checks["executable"] == "pass", report
assert checks["youtube"] == "pass", report
'

if [ "${TICONNECT_SMOKE_GUI:-1}" = "1" ]; then
    gui_log="$WORK/ui.log"
    gui_command=(dbus-run-session -- bash -c '
set -euo pipefail
"$0" --appimage-extract-and-run ui >"$1" 2>&1 &
ui_pid=$!
trap '\''kill "$ui_pid" 2>/dev/null || true'\'' EXIT
for _ in $(seq 1 600); do
    if grep -q "window presented" "$1"; then
        break
    fi
    sleep 0.1
done
grep -q "window presented" "$1" || {
    cat "$1"
    exit 1
}
"$0" --appimage-extract-and-run cli status >/dev/null
"$0" --appimage-extract-and-run cli quit >/dev/null || true
kill "$ui_pid" 2>/dev/null || true
wait "$ui_pid" 2>/dev/null || true
' "$APPIMAGE" "$gui_log")
    if [ -n "${DISPLAY:-}" ]; then
        "${gui_command[@]}"
    elif command -v xvfb-run >/dev/null; then
        xvfb-run -a "${gui_command[@]}"
    elif command -v Xvfb >/dev/null; then
        : "${TICONNECT_SMOKE_DISPLAY:=:99}"
        Xvfb "$TICONNECT_SMOKE_DISPLAY" -screen 0 1280x720x24 >/tmp/ticonnect-xvfb-smoke.log 2>&1 &
        XVFB_PID=$!
        DISPLAY="$TICONNECT_SMOKE_DISPLAY" "${gui_command[@]}"
    else
        echo "UI smoke test needs a display, xvfb-run, or Xvfb" >&2
        exit 1
    fi
fi
