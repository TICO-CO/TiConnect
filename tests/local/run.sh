#!/usr/bin/env bash
set -euo pipefail
set +m
export WORK

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SESSION="${1:-x11}"
SCENARIO="${2:-all}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/ticonnect-local.XXXXXX")"
BUS_A="$WORK/bus-a"
BUS_B="$WORK/bus-b"
RUNTIME="$WORK/runtime"
NODE_A="$WORK/node-a"
NODE_B="$WORK/node-b"
PIDS=()
WESTON_PID=""
DBUS_PID=""
UI_PID=""
OVL_PID=""
PORTAL_PIDS=()

cleanup() {
    local status=$?
    trap - EXIT
    for pid in "$UI_PID" "$OVL_PID" "${PIDS[@]}"; do
        [ -n "$pid" ] || continue
        kill "$pid" 2>/dev/null || true
    done
    for pid in "${PORTAL_PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
    # Portal and compositor helpers may be daemonized by D-Bus activation.
    python3 - "$WORK" <<'PYC' 2>/dev/null || true
import os, signal, sys
needle = sys.argv[1].encode()
for pid in [p for p in os.listdir('/proc') if p.isdigit()]:
    try:
        env = open(f'/proc/{pid}/environ', 'rb').read()
    except OSError:
        continue
    if needle in env:
        try:
            os.kill(int(pid), signal.SIGTERM)
        except OSError:
            pass
PYC
    [ -n "$WESTON_PID" ] && kill "$WESTON_PID" 2>/dev/null || true
    if [ "$status" -ne 0 ]; then
        echo "local test failed: session=$SESSION scenario=$SCENARIO work=$WORK" >&2
    else
        rm -rf "$WORK"
    fi
    exit "$status"
}
trap cleanup EXIT

log() { printf '== %s\n' "$*"; }
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }

command -v dbus-daemon >/dev/null || fail "dbus-daemon missing"
export TICONNECT_LOG_LEVEL="${TICONNECT_LOG_LEVEL:-info}"

configure_audio() {
    local plugin_dir
    plugin_dir="$(ls -d /nix/store/*alsa-plugins*/lib/alsa-lib 2>/dev/null | head -n 1 || true)"
    if [ -n "$plugin_dir" ]; then
        cat >"$WORK/asound.conf" <<'CONF'
pcm.!default {
    type pulse
}
ctl.!default {
    type pulse
}
CONF
        export ALSA_CONFIG_PATH="$WORK/asound.conf"
        export ALSA_PLUGIN_DIR="$plugin_dir"
    fi
}

start_bus() {
    mkdir -p "$WORK"
    configure_audio
    dbus-daemon --session --fork --nopidfile --nosyslog --address="unix:path=$BUS_A" >/dev/null
    dbus-daemon --session --fork --nopidfile --nosyslog --address="unix:path=$BUS_B" >/dev/null
    for _ in $(seq 1 50); do
        [ -S "$BUS_A" ] && [ -S "$BUS_B" ] && return
        sleep 0.05
    done
    fail "private D-Bus did not start"
}

start_session() {
    local host_runtime="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
    export XDG_RUNTIME_DIR="$RUNTIME"
    mkdir -p "$XDG_RUNTIME_DIR"
    chmod 700 "$XDG_RUNTIME_DIR"
    for socket in pipewire-0 pulse; do
        if [ -e "$host_runtime/$socket" ]; then
            ln -sfn "$host_runtime/$socket" "$XDG_RUNTIME_DIR/$socket"
        fi
    done
    export PULSE_SERVER="unix:$host_runtime/pulse/native"
    case "$SESSION" in
        x11)
            export DISPLAY="${DISPLAY:-:0}"
            [ -n "${DISPLAY:-}" ] || fail "DISPLAY is not set"
            export GDK_BACKEND=x11
            ;;
        weston)
            command -v weston >/dev/null || fail "weston missing"
            weston --backend=headless-backend.so --socket=wayland-ticonnect \
                --idle-time=0 --width=1280 --height=720 >"$WORK/weston.log" 2>&1 &
            WESTON_PID=$!
            export WAYLAND_DISPLAY=wayland-ticonnect
            export GDK_BACKEND=wayland
            for _ in $(seq 1 100); do [ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] && break; sleep 0.05; done
            [ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] || fail "wayland socket missing"
            ;;
        kwin)
            command -v kwin_wayland >/dev/null || fail "kwin_wayland missing"
            kwin_wayland --virtual --socket wayland-ticonnect --no-lockscreen --no-global-shortcuts \
                >"$WORK/kwin.log" 2>&1 &
            WESTON_PID=$!
            export WAYLAND_DISPLAY=wayland-ticonnect
            export GDK_BACKEND=wayland
            for _ in $(seq 1 100); do [ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] && break; sleep 0.05; done
            [ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] || fail "wayland socket missing"
            ;;
        *)
            fail "unknown session $SESSION (use x11, weston, kwin)"
            ;;
    esac
}

start_portal() {
    [ "$SESSION" = kwin ] || return 0
    local portal_unit="$HOME/.nix-profile/share/systemd/user/xdg-desktop-portal.service"
    local kde_unit="$HOME/.nix-profile/share/systemd/user/plasma-xdg-desktop-portal-kde.service"
    [ -f "$portal_unit" ] || return 0
    local portal_bin kde_bin
    portal_bin="$(sed -n 's/^ExecStart=//p' "$portal_unit")"
    kde_bin="$(sed -n 's/^ExecStart=//p' "$kde_unit")"
    [ -x "$portal_bin" ] && [ -x "$kde_bin" ] || return 0
    export XDG_CURRENT_DESKTOP=KDE
    export XDG_SESSION_TYPE=wayland
    export KDE_FULL_SESSION=true
    export KDE_SESSION_VERSION=6
    export QT_QPA_PLATFORM=wayland
    export XDG_DATA_DIRS="$HOME/.nix-profile/share:/run/current-system/sw/share:${XDG_DATA_DIRS:-}"
    DBUS_SESSION_BUS_ADDRESS="unix:path=$BUS_B" "$portal_bin" >"$WORK/portal.log" 2>&1 &
    PORTAL_PIDS+=("$!")
    DBUS_SESSION_BUS_ADDRESS="unix:path=$BUS_B" "$kde_bin" >"$WORK/portal-kde.log" 2>&1 &
    PORTAL_PIDS+=("$!")
    sleep 1
}

start_nodes() {
    mkdir -p "$NODE_A" "$NODE_B"
    PORT_A=$((40000 + RANDOM % 20000))
    PORT_B=$((PORT_A + 1))
    [ "$PORT_B" -lt 65535 ] || PORT_B=40000
    for node in a b; do
        dir="$WORK/node-$node"
        port=$([ "$node" = a ] && echo "$PORT_A" || echo "$PORT_B")
        DBUS_SESSION_BUS_ADDRESS="unix:path=$([ "$node" = a ] && echo "$BUS_A" || echo "$BUS_B")" \
        TICONNECT_CONFIG_DIR="$dir/config" \
        TICONNECT_DATA_DIR="$dir/data" \
        TICONNECT_TRANSPORT__PORT="$port" \
        TICONNECT_DISCOVERY__BEACON_INTERVAL_SECS=1 \
        "$ROOT/target/debug/ticonnectd" >"$dir/daemon.log" 2>&1 &
        PIDS+=("$!")
    done
    for node in a b; do
        wait_for "cli $node ping" "pong" 90 || fail "node $node did not start"
    done
    A_ID="$(cli a id show | awk '/^node_id:/ {print $2}')"
    B_ID="$(cli b id show | awk '/^node_id:/ {print $2}')"
    [ -n "$A_ID" ] && [ -n "$B_ID" ] || fail "missing node ids"
}

monitor_start() {
    local node="$1"
    local name="$2"
    local bus="$BUS_A"
    [ "$node" = b ] && bus="$BUS_B"
    DBUS_SESSION_BUS_ADDRESS="unix:path=$bus" \
        gdbus monitor --session --dest org.ticonnect.Daemon \
        --object-path /org/ticonnect/Daemon >"$WORK/$name.signals" 2>&1 &
    sleep 0.3
}

monitor_expect() {
    local name="$1"
    local pattern="$2"
    for _ in $(seq 1 30); do
        grep -qF "$pattern" "$WORK/$name.signals" 2>/dev/null && return 0
        sleep 0.5
    done
    return 1
}

cli() {
    local node="$1"
    shift
    local dir="$WORK/node-$node"
    local bus="$BUS_A"
    [ "$node" = b ] && bus="$BUS_B"
    DBUS_SESSION_BUS_ADDRESS="unix:path=$bus" \
    TICONNECT_CONFIG_DIR="$dir/config" TICONNECT_DATA_DIR="$dir/data" \
    TICONNECT_LOG_LEVEL=off "$ROOT/target/debug/ticonnect-cli" "$@"
}

wait_for() {
    local cmd="$1"
    local expected="$2"
    local timeout="$3"
    local out
    for _ in $(seq 1 "$timeout"); do
        out="$(eval "$cmd" 2>/dev/null || true)"
        if [ "$out" = "$expected" ]; then return 0; fi
        sleep 1
    done
    return 1
}

wait_until() {
    local cmd="$1"
    local timeout="$2"
    for _ in $(seq 1 "$timeout"); do
        if eval "$cmd" >/dev/null 2>&1; then return 0; fi
        sleep 1
    done
    return 1
}

pair_nodes() {
    local start pin
    wait_until "cli b discover list | grep -q '$A_ID'" 60 || fail "node-b did not discover node-a"
    wait_until "cli a discover list | grep -q '$B_ID'" 60 || fail "node-a did not discover node-b"
    start="$(cli a pair start)"
    pin="$(printf '%s\n' "$start" | sed -n 's/^pin: //p')"
    [ -n "$pin" ] || fail "pair start failed"
    cli b pair with-pin "$A_ID" "$pin" >/dev/null
    wait_until "cli a peers list | grep -q '$B_ID'" 30 || fail "node-a did not trust node-b"
    wait_until "cli b peers list | grep -q '$A_ID'" 30 || fail "node-b did not trust node-a"
    wait_until "[ \"\$(cli a status | awk '/^active_sessions:/ {print \$2}')\" -ge 1 ]" 30 || fail "no session"
}

scenario_attachments() {
    log "attachment transfer"
    monitor_start b attachment-signals
    head -c 262144 /dev/urandom >"$WORK/file.bin"
    local message hash
    message="$(cli a send-file "$B_ID" "$WORK/file.bin")"
    [ -n "$message" ] || fail "send-file failed"
    hash="$(cli a messages "$B_ID" | awk '/^'"$message"' / {print $NF}' | tr -d '[]')"
    for _ in $(seq 1 30); do
        if cli b messages "$A_ID" | grep -q "$message"; then break; fi
        sleep 1
    done
    cli b messages "$A_ID" | grep -q "$message" || fail "attachment message missing"
    find "$WORK/node-b/data" -type f -path '*attachments*' | grep -q . || fail "attachment not stored"
    monitor_expect attachment-signals "TransferProgress" || fail "missing TransferProgress signal"
    monitor_expect attachment-signals "TransferDone" || fail "missing TransferDone signal"
    log "attachment ok hash=$hash"
}

scenario_messaging() {
    log "message round trip"
    local id
    id="$(cli a send "$B_ID" "local-$SESSION-$(date +%s)")"
    for _ in $(seq 1 30); do
        if cli b messages "$A_ID" | grep -q "$id"; then break; fi
        sleep 1
    done
    cli b messages "$A_ID" | grep -q "$id" || fail "message missing"
    log "messaging ok id=$id"
}

scenario_call() {
    log "call signaling"
    monitor_start a call-signals
    monitor_start b call-signals
    local call state
    call="$(cli a call start "$B_ID")"
    [ -n "$call" ] || fail "call start failed"
    for _ in $(seq 1 20); do
        if cli b call status | grep -q "call_id: $call"; then break; fi
        sleep 0.5
    done
    cli b call status | grep -q "call_id: $call" || fail "callee did not ring"
    monitor_expect call-signals "IncomingCall" || fail "missing IncomingCall signal"
    cli b call accept "$call"
    for _ in $(seq 1 20); do
        state="$(cli a call status | awk '/^state:/ {print $2}')"
        [ "$state" = "in-call" ] && break
        sleep 0.5
    done
    [ "$state" = "in-call" ] || fail "caller did not enter in-call (state=$state)"
    local sent_a recv_a sent_b recv_b
    for _ in $(seq 1 40); do
        sent_a="$(cli a call metrics | awk '/^frames_sent:/ {print $2}')"
        recv_a="$(cli a call metrics | awk '/^frames_received:/ {print $2}')"
        sent_b="$(cli b call metrics | awk '/^frames_sent:/ {print $2}')"
        recv_b="$(cli b call metrics | awk '/^frames_received:/ {print $2}')"
        if [ "${sent_a:-0}" -gt 0 ] && [ "${recv_a:-0}" -gt 0 ] &&            [ "${sent_b:-0}" -gt 0 ] && [ "${recv_b:-0}" -gt 0 ]; then
            break
        fi
        sleep 0.5
    done
    [ "${sent_a:-0}" -gt 0 ] && [ "${sent_b:-0}" -gt 0 ] || fail "call audio frames not sent"
    [ "${recv_a:-0}" -gt 0 ] && [ "${recv_b:-0}" -gt 0 ] || fail "call audio frames not received"
    monitor_expect call-signals "CallStateChanged" || fail "missing CallStateChanged signal"
    log "audio sent a=$sent_a b=$sent_b received a=$recv_a b=$recv_b"
    cli a call hangup "$call"
    for _ in $(seq 1 20); do
        [ "$(cli a call status)" = "idle" ] && break
        sleep 0.5
    done
    [ "$(cli a call status)" = "idle" ] || fail "call did not end"
    monitor_expect call-signals "CallEnded" || fail "missing CallEnded signal"
    log "call ok"
}

scenario_remote() {
    log "remote consent/PIN/kill switch"
    monitor_start a remote-signals
    monitor_start b remote-signals
    local request session pin
    request="$(cli a remote request "$B_ID")"
    session="$(printf '%s\n' "$request" | awk -F': ' '/session_id/ {print $2}')"
    [ -n "$session" ] || fail "remote request failed"
    for _ in $(seq 1 20); do
        cli b remote status | grep -q "session_id: $session" && break
        sleep 0.5
    done
    monitor_expect remote-signals "RemoteSessionRequested" || fail "missing RemoteSessionRequested signal"
    pin="$(cli b remote status | awk '/^pin:/ {print $2}')"
    [ -n "$pin" ] || fail "host did not generate PIN"
    cli b remote accept "$session" "" >/dev/null
    cli a remote accept "$session" "$pin" >/dev/null
    for _ in $(seq 1 30); do
        if cli a remote status | grep -q '^state: active'; then break; fi
        sleep 0.5
    done
    cli a remote status | grep -q '^state: active' || fail "remote session not active"
    monitor_expect remote-signals "RemoteSessionStarted" || fail "missing RemoteSessionStarted signal"
    monitor_expect remote-signals "RemoteIndicatorChanged" || fail "missing indicator signal"
    if [ "${PORTAL_REQUIRE:-0}" = "1" ]; then
        log "approve the desktop portal dialog on the nested KWin output"
        for _ in $(seq 1 120); do
            grep -q "remote desktop portal session started" "$WORK/node-b/daemon.log" && break
            sleep 0.5
        done
        grep -q "remote desktop portal session started" "$WORK/node-b/daemon.log" \
            || fail "portal session did not start"
    fi
    cli b remote end "$session" >/dev/null
    for _ in $(seq 1 20); do
        [ "$(cli a remote status)" = "idle" ] && break
        sleep 0.5
    done
    [ "$(cli a remote status)" = "idle" ] || fail "remote session did not end"
    monitor_expect remote-signals "RemoteSessionEnded" || fail "missing RemoteSessionEnded signal"
    log "remote ok"
}

scenario_ui() {
    log "UI and overlay startup"
    [ -x "$ROOT/target/debug/ticonnect-ui" ] || fail "ticonnect-ui binary missing"
    DBUS_SESSION_BUS_ADDRESS="unix:path=$BUS_A" \
    TICONNECT_CONFIG_DIR="$NODE_A/config" TICONNECT_DATA_DIR="$NODE_A/data" \
    TICONNECT_LOG_LEVEL=off "$ROOT/target/debug/ticonnect-ui" >"$WORK/ui.log" 2>&1 &
    UI_PID=$!
    DBUS_SESSION_BUS_ADDRESS="unix:path=$BUS_A" \
    TICONNECT_CONFIG_DIR="$NODE_A/config" TICONNECT_DATA_DIR="$NODE_A/data" \
    TICONNECT_LOG_LEVEL=off "$ROOT/target/debug/ticonnect-ovl" >"$WORK/overlay.log" 2>&1 &
    OVL_PID=$!
    sleep 3
    kill -0 "$UI_PID" 2>/dev/null || fail "UI exited"
    if [ "$SESSION" = weston ]; then
        if ! kill -0 "$OVL_PID" 2>/dev/null; then
            log "Weston has no layer-shell support; overlay fallback exited as expected"
            OVL_PID=""
        fi
    else
        kill -0 "$OVL_PID" 2>/dev/null || fail "overlay exited"
    fi
    if [ "$SESSION" = x11 ]; then
        command -v xdotool >/dev/null && xdotool search --sync --name 'TiConnect' >/dev/null || true
    fi
    log "ui ok"
}

start_bus
start_session
start_portal
start_nodes
pair_nodes

case "$SCENARIO" in
    all)
        scenario_messaging
        scenario_attachments
        scenario_ui
        scenario_call
        scenario_remote
        ;;
    messaging) scenario_messaging ;;
    attachments) scenario_attachments ;;
    call) scenario_call ;;
    remote) scenario_remote ;;
    ui) scenario_ui ;;
    *) fail "unknown scenario $SCENARIO" ;;
esac

log "PASS session=$SESSION scenario=$SCENARIO"
