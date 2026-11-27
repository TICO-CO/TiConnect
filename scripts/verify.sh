#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

log() {
    printf '==> verify: %s\n' "$*"
}

XVFB_PID=""
cleanup() {
    if [ -n "$XVFB_PID" ]; then
        kill "$XVFB_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

log "format"
cargo fmt --all -- --check

log "clippy"
cargo clippy --workspace --all-targets -- -D warnings

log "tests"
dbus-run-session -- cargo test --workspace --all-targets

log "architecture graph"
cargo xtask systems --check

log "web checks"
make web-check

log "web build"
make web-build
git diff --exit-code -- crates/ui/assets

if [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
    command -v Xvfb >/dev/null 2>&1 || {
        printf 'verify: no display and Xvfb is unavailable\n' >&2
        exit 1
    }
    Xvfb :99 -screen 0 1280x720x24 >"${TMPDIR:-/tmp}/ticonnect-verify-xvfb.log" 2>&1 &
    XVFB_PID=$!
    export DISPLAY=:99
    export WEBKIT_DISABLE_DMABUF_RENDERER=1
    sleep 1
fi

log "build binaries"
cargo build --workspace --bins

log "local integration"
TICONNECT_TEST_AUDIO="${TICONNECT_TEST_AUDIO:-null}" \
    tests/local/run.sh "${TICONNECT_VERIFY_SESSION:-x11}" all

log "lab smoke"
make lab-smoke

log "passed"
