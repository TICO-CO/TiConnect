#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="${TICONNECT_BIN:-$ROOT/target/debug/ticonnect}"
SMOKE=0
HOT_RELOAD="${LAB_HOT_RELOAD:-1}"
DEV_URL="http://127.0.0.1:5173"

usage() {
    cat >&2 <<'EOF'
usage: tests/local/lab.sh [--smoke]

Runs two isolated TiConnect nodes. Inside Zellij it opens a new tab with one
client pane per node. Outside Zellij use --smoke for the automated self-test.
EOF
    exit 2
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --smoke) SMOKE=1 ;;
        -h | --help) usage ;;
        *) usage ;;
    esac
    shift
done

fail() {
    printf 'lab: %s\n' "$*" >&2
    exit 1
}

[ -x "$BIN" ] || fail "missing $BIN; run: CARGO_PROFILE_DEV_DEBUG=0 cargo build --bin ticonnect"

if ! "$BIN" --version >/dev/null 2>&1; then
    glib_lib="$(find /nix/store -maxdepth 3 -name 'libgio-2.0.so.0' -printf '%h\n' 2>/dev/null | sort | tail -n 1 || true)"
    libs=()
    [ -d "$HOME/.nix-profile/lib" ] && libs+=("$HOME/.nix-profile/lib")
    [ -n "$glib_lib" ] && libs+=("$glib_lib")
    if [ "${#libs[@]}" -gt 0 ]; then
        joined="$(IFS=:; printf '%s' "${libs[*]}")"
        export LD_LIBRARY_PATH="$joined${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    fi
fi
"$BIN" --version >/dev/null || fail "cannot execute $BIN; set LD_LIBRARY_PATH or rebuild"

if [ "$SMOKE" = "0" ]; then
    [ -n "${ZELLIJ:-}" ] || fail "run inside Zellij or use --smoke"
    command -v zellij >/dev/null 2>&1 || fail "zellij CLI is not available"
fi

WORK="$(mktemp -d "${TMPDIR:-/tmp}/ticonnect-lab.XXXXXX")"
BUS_A="$WORK/bus-a"
BUS_B="$WORK/bus-b"
NODE_A="$WORK/node-a"
NODE_B="$WORK/node-b"
TAB_NAME="ticonnect-lab-$$"
PIDS=()
SEEN_TAB=0
CALL_AUDIO_DISCARD='Discard all samples (playback) or generate zero samples (capture)'

zellij_action() {
    if [ -n "${ZELLIJ:-}" ] || [ -z "${ZELLIJ_SESSION_NAME:-}" ]; then
        zellij action "$@"
    else
        zellij --session "$ZELLIJ_SESSION_NAME" action "$@"
    fi
}

zellij_query_tabs() {
    if [ -n "${ZELLIJ:-}" ] || [ -z "${ZELLIJ_SESSION_NAME:-}" ]; then
        timeout 2 zellij action query-tab-names
    else
        timeout 2 zellij --session "$ZELLIJ_SESSION_NAME" action query-tab-names
    fi
}

zellij_tab_exists() {
    zellij_query_tabs 2>/dev/null | grep -qx "$TAB_NAME"
}

cleanup() {
    local status=$?
    trap - EXIT INT TERM HUP
    if [ -n "${ZELLIJ:-}" ] && zellij_tab_exists; then
        zellij_action go-to-tab-name "$TAB_NAME" >/dev/null 2>&1 || true
        zellij_action close-tab >/dev/null 2>&1 || true
    fi
    for pid_file in "$WORK"/node-*/ui.pid "$WORK"/node-*/overlay.pid; do
        [ -f "$pid_file" ] || continue
        kill "$(cat "$pid_file")" 2>/dev/null || true
    done
    for pid in "${PIDS[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    sleep 0.2
    for pid in "${PIDS[@]}"; do
        kill -9 "$pid" 2>/dev/null || true
    done
    if [ "${LAB_KEEP:-0}" = "1" ]; then
        printf 'lab: kept state in %s\n' "$WORK"
    else
        rm -rf "$WORK"
    fi
    exit "$status"
}
trap cleanup EXIT INT TERM HUP

configure_audio() {
    if [ -n "${ALSA_PLUGIN_DIR:-}" ] || [ -n "${ALSA_CONFIG_PATH:-}" ]; then
        return 0
    fi
    local plugin_dir
    plugin_dir="$(ls -d /nix/store/*alsa-plugins*/lib/alsa-lib 2>/dev/null | head -n 1 || true)"
    [ -n "$plugin_dir" ] || return 0
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
    printf 'lab: ALSA audio plugins: %s\n' "$plugin_dir"
}

start_bus() {
    local bus="$1"
    local name="${bus##*/}"
    local log="$WORK/dbus-$name.log"
    local dbus_bin config_file pid
    command -v dbus-daemon >/dev/null 2>&1 || fail "dbus-daemon missing"
    dbus_bin="$(command -v dbus-daemon)"
    config_file="${dbus_bin%/*}/../share/dbus-1/session.conf"
    if [ -r "$config_file" ]; then
        dbus-daemon --config-file="$config_file" --nofork --nopidfile --nosyslog \
            --address="unix:path=$bus" >"$log" 2>&1 &
    else
        dbus-daemon --session --nofork --nopidfile --nosyslog \
            --address="unix:path=$bus" >"$log" 2>&1 &
    fi
    pid=$!
    PIDS+=("$pid")
    for _ in $(seq 1 400); do
        [ -S "$bus" ] && return 0
        if ! kill -0 "$pid" 2>/dev/null; then
            printf 'lab: private D-Bus exited before creating %s\n' "$bus" >&2
            tail -n 80 "$log" >&2 || true
            fail "private D-Bus did not start"
        fi
        sleep 0.05
    done
    printf 'lab: private D-Bus did not create %s within 20s\n' "$bus" >&2
    tail -n 80 "$log" >&2 || true
    fail "private D-Bus did not start"
}

stop_existing_dev_server() {
    local pids
    pids="$(ss -ltnpH "sport = :5173" 2>/dev/null |
        sed -n 's/.*pid=\([0-9][0-9]*\).*/\1/p' | sort -u || true)"
    if [ -z "$pids" ] && command -v fuser >/dev/null 2>&1; then
        pids="$(fuser 5173/tcp 2>/dev/null | tr ' ' '\n' |
            grep -E '^[0-9]+$' | sort -u || true)"
    fi
    [ -n "$pids" ] ||
        fail "port 5173 is in use but the owning process could not be identified"
    local pid args cwd
    for pid in $pids; do
        args="$(ps -o args= -p "$pid" 2>/dev/null || true)"
        cwd="$(readlink -f "/proc/$pid/cwd" 2>/dev/null || true)"
        if [ "$cwd" != "$ROOT/crates/ui/web" ] &&
            [[ "$args" != *"$ROOT/crates/ui/web"* ]]; then
            fail "port 5173 is held by an unrelated process (pid $pid): $args"
        fi
    done
    printf 'lab: restarting UI dev server on port 5173\n'
    kill $pids 2>/dev/null || true
    for _ in $(seq 1 50); do
        if ! (exec 3<>"/dev/tcp/127.0.0.1/5173") 2>/dev/null; then
            return 0
        fi
        sleep 0.1
    done
    kill -9 $pids 2>/dev/null || true
    sleep 0.2
    if (exec 3<>"/dev/tcp/127.0.0.1/5173") 2>/dev/null; then
        fail "port 5173 is still in use after stopping the existing dev server"
    fi
}

start_dev_server() {
    command -v bun >/dev/null 2>&1 ||
        fail "bun is required for UI hot reload; set LAB_HOT_RELOAD=0 to use embedded assets"
    command -v curl >/dev/null 2>&1 ||
        fail "curl is required for UI hot reload; set LAB_HOT_RELOAD=0 to use embedded assets"
    if (exec 3<>"/dev/tcp/127.0.0.1/5173") 2>/dev/null; then
        stop_existing_dev_server
    fi
    (
        cd "$ROOT/crates/ui/web"
        exec bun run dev
    ) >"$WORK/vite.log" 2>&1 &
    PIDS+=("$!")
    local ready=0 i
    for i in $(seq 1 900); do
        if curl --noproxy '*' --max-time 1 -fsS "$DEV_URL/" >/dev/null 2>&1; then
            ready=1
            break
        fi
        if ! kill -0 "${PIDS[-1]}" 2>/dev/null; then
            tail -n 80 "$WORK/vite.log" >&2 || true
            fail "Vite dev server exited"
        fi
        if [ $((i % 100)) -eq 0 ]; then
            printf 'lab: still waiting for Vite dev server (%ss)\n' "$((i / 10))" >&2
        fi
        sleep 0.1
    done
    if [ "$ready" = "0" ]; then
        printf 'lab: Vite did not answer %s within 90s\n' "$DEV_URL" >&2
        printf 'lab: recent vite log (%s):\n' "$WORK/vite.log" >&2
        tail -n 80 "$WORK/vite.log" >&2 || true
        if command -v ss >/dev/null 2>&1; then
            printf 'lab: port 5173 listeners:\n' >&2
            ss -ltnpH 'sport = :5173' >&2 2>/dev/null || true
        fi
        fail "Vite dev server did not become ready; log: $WORK/vite.log"
    fi
    printf 'lab: UI hot reload at %s (log: %s)\n' "$DEV_URL" "$WORK/vite.log"
}

cli() {
    local node="$1"
    shift
    local dir="$WORK/node-$node"
    local bus="$BUS_A"
    [ "$node" = b ] && bus="$BUS_B"
    TICONNECT_IPC_BUS_ADDRESS="unix:path=$bus" \
    TICONNECT_CONFIG_DIR="$dir/config" \
    TICONNECT_DATA_DIR="$dir/data" \
    TICONNECT_NO_AUTOSTART=1 \
    TICONNECT_LOG_LEVEL=off \
        "$BIN" cli "$@"
}

start_node() {
    local node="$1" bus="$2" dir="$3" port="$4"
    mkdir -p "$dir/config" "$dir/data"
    DBUS_SESSION_BUS_ADDRESS="unix:path=$bus" \
    TICONNECT_IPC_BUS_ADDRESS="unix:path=$bus" \
    TICONNECT_CONFIG_DIR="$dir/config" \
    TICONNECT_DATA_DIR="$dir/data" \
    TICONNECT_LOG_LEVEL="${TICONNECT_LAB_LOG_LEVEL:-warn}" \
    TICONNECT_LOG_FILE="$dir/daemon.log" \
    TICONNECT_TRANSPORT__PORT="$port" \
    TICONNECT_DISCOVERY__BEACON_INTERVAL_SECS=1 \
        "$BIN" daemon >"$dir/stdout.log" 2>&1 &
    PIDS+=("$!")
}

wait_for() {
    local command="$1" timeout="$2"
    for _ in $(seq 1 "$timeout"); do
        if eval "$command" >/dev/null 2>&1; then
            return 0
        fi
        sleep 0.1
    done
    return 1
}

node_id() {
    cli "$1" id show | awk '/^node_id:/ {print $2}'
}

gio_env() {
    if [ -n "${GIO_EXTRA_MODULES:-}" ]; then
        return 0
    fi
    local dir
    dir="$(ls -d /nix/store/*-glib-networking-*/lib/gio/modules 2>/dev/null |
        head -n 1)"
    [ -n "$dir" ] ||
        fail "glib-networking is not available; run make lab from nix develop"
    GIO_EXTRA_MODULES="$dir"
    export GIO_EXTRA_MODULES
}

gstreamer_env() {
    if [ -n "${GST_PLUGIN_SYSTEM_PATH_1_0:-}" ]; then
        GST_PLUGIN_SYSTEM_PATH="${GST_PLUGIN_SYSTEM_PATH:-$GST_PLUGIN_SYSTEM_PATH_1_0}"
        export GST_PLUGIN_SYSTEM_PATH_1_0 GST_PLUGIN_SYSTEM_PATH
        return 0
    fi
    local lib version dir
    lib="$(ldd "$BIN" 2>/dev/null | awk '/libgstreamer-1\.0\.so/ {print $3; exit}')"
    [ -n "$lib" ] ||
        fail "cannot resolve libgstreamer-1.0; run make lab from nix develop"
    version="$(basename "$(dirname "$(dirname "$lib")")" |
        sed -n 's/.*-gstreamer-\(.*\)$/\1/p')"
    [ -n "$version" ] || fail "cannot determine GStreamer version from $lib"
    local -a dirs=()
    for dir in /nix/store/*-gstreamer-"$version"/lib/gstreamer-1.0 \
        /nix/store/*-gst-plugins-*-"$version"/lib/gstreamer-1.0 \
        /nix/store/*-gst-libav-"$version"/lib/gstreamer-1.0; do
        [ -d "$dir" ] && dirs+=("$dir")
    done
    [ "${#dirs[@]}" -gt 0 ] ||
        fail "no GStreamer $version plugins found; run make lab from nix develop"
    GST_PLUGIN_SYSTEM_PATH_1_0="$(IFS=:; printf '%s' "${dirs[*]}")"
    GST_PLUGIN_SYSTEM_PATH="$GST_PLUGIN_SYSTEM_PATH_1_0"
    export GST_PLUGIN_SYSTEM_PATH_1_0 GST_PLUGIN_SYSTEM_PATH
}

write_node_files() {
    local node="$1" bus="$2" dir="$3" id="$4" user_id="$5" label="$6" user_label="$7"
    mkdir -p "$dir/bin"
    cat >"$dir/bin/ticonnect" <<EOF
#!/bin/sh
exec env \\
  TICONNECT_IPC_BUS_ADDRESS='unix:path=$bus' \\
  TICONNECT_CONFIG_DIR='$dir/config' \\
  TICONNECT_DATA_DIR='$dir/data' \\
  TICONNECT_NO_AUTOSTART=1 \\
  TICONNECT_LOG_LEVEL=off \\
  '$BIN' cli "\$@"
EOF
    chmod 755 "$dir/bin/ticonnect"

    cat >"$dir/env" <<EOF
export TICONNECT_IPC_BUS_ADDRESS='unix:path=$bus'
export TICONNECT_CONFIG_DIR='$dir/config'
export TICONNECT_DATA_DIR='$dir/data'
export TICONNECT_NO_AUTOSTART=1
export TICONNECT_LOG_LEVEL=off
export TICONNECT_BIN='$BIN'
export TICONNECT_UI_TITLE='TiConnect · $label'
export LD_LIBRARY_PATH='${LD_LIBRARY_PATH:-}'
export TICONNECT_UI_TIMING='${TICONNECT_UI_TIMING:-}'
export GST_PLUGIN_SYSTEM_PATH_1_0='${GST_PLUGIN_SYSTEM_PATH_1_0:-}'
export GST_PLUGIN_SYSTEM_PATH='${GST_PLUGIN_SYSTEM_PATH:-}'
export GIO_EXTRA_MODULES='${GIO_EXTRA_MODULES:-}'
export TICONNECT_YTDLP='${TICONNECT_YTDLP:-}'
export TICONNECT_DISABLE_GPU='${TICONNECT_DISABLE_GPU:-}'
export WEBKIT_DISABLE_DMABUF_RENDERER='${WEBKIT_DISABLE_DMABUF_RENDERER:-}'
export LIBGL_ALWAYS_SOFTWARE='${LIBGL_ALWAYS_SOFTWARE:-}'
export GALLIUM_DRIVER='${GALLIUM_DRIVER:-}'
export LAB_DIR='$WORK'
export LAB_NODE_DIR='$dir'
export LAB_NODE='$node'
export LAB_LABEL='$label'
export LAB_USER_LABEL='$user_label'
export NODE_ID='$id'
export USER_ID='$user_id'
export PATH='$dir/bin':"\$PATH"
EOF

    if [ -n "$DEV_URL" ] && [ "$SMOKE" = "0" ] && [ "$HOT_RELOAD" = "1" ]; then
        printf "export TICONNECT_DEV_URL='%s'\nexport LAB_DEV_URL='%s'\n" \
            "$DEV_URL" "$DEV_URL" >>"$dir/env"
    fi

    cat >"$dir/bashrc" <<EOF
source '$dir/env'
export PS1='[$node] \\w \$ '
user() { printf '%s\\n' "\$USER_ID"; }
send() { ticonnect send "\$USER_ID" "\$*"; }
inbox() { ticonnect messages "\$USER_ID" --limit "\${1:-20}"; }
pair() { '$WORK/bin/lab-pair'; }
ui() {
    if [ -f "\$LAB_NODE_DIR/ui.pid" ] && kill -0 "\$(cat "\$LAB_NODE_DIR/ui.pid")" 2>/dev/null; then
        echo "UI already running"
        return 0
    fi
    "\$TICONNECT_BIN" ui >>"\$LAB_NODE_DIR/ui.log" 2>&1 &
    local pid=\$!
    echo "\$pid" >"\$LAB_NODE_DIR/ui.pid"
    disown "\$pid" 2>/dev/null || true
    sleep 1
    if ! kill -0 "\$pid" 2>/dev/null; then
        echo "UI exited; log:"
        tail -n 20 "\$LAB_NODE_DIR/ui.log"
        rm -f "\$LAB_NODE_DIR/ui.pid"
        return 1
    fi
    echo "UI started; log: \$LAB_NODE_DIR/ui.log"
}
stop-ui() {
    if [ -f "\$LAB_NODE_DIR/ui.pid" ]; then
        kill "\$(cat "\$LAB_NODE_DIR/ui.pid")" 2>/dev/null || true
        rm -f "\$LAB_NODE_DIR/ui.pid"
        echo "UI stopped"
    fi
}
ovl() {
    if [ -f "\$LAB_NODE_DIR/overlay.pid" ] && kill -0 "\$(cat "\$LAB_NODE_DIR/overlay.pid")" 2>/dev/null; then
        echo "Overlay already running"
        return 0
    fi
    "\$TICONNECT_BIN" overlay >>"\$LAB_NODE_DIR/overlay.log" 2>&1 &
    local pid=\$!
    echo "\$pid" >"\$LAB_NODE_DIR/overlay.pid"
    disown "\$pid" 2>/dev/null || true
    sleep 1
    if ! kill -0 "\$pid" 2>/dev/null; then
        echo "Overlay exited; log:"
        tail -n 20 "\$LAB_NODE_DIR/overlay.log"
        rm -f "\$LAB_NODE_DIR/overlay.pid"
        return 1
    fi
    echo "Overlay started; log: \$LAB_NODE_DIR/overlay.log"
}
stop-ovl() {
    if [ -f "\$LAB_NODE_DIR/overlay.pid" ]; then
        kill "\$(cat "\$LAB_NODE_DIR/overlay.pid")" 2>/dev/null || true
        rm -f "\$LAB_NODE_DIR/overlay.pid"
        echo "Overlay stopped"
    fi
}
logs() { tail -n "\${1:-50}" "\$LAB_NODE_DIR/daemon.log"; }
call-audio() {
    case "\${1:-}" in
        silent)
            ticonnect settings set calls.input_device '${CALL_AUDIO_DISCARD}'
            ticonnect settings set calls.output_device '${CALL_AUDIO_DISCARD}'
            echo "Call audio muted; next call uses the null device"
            ;;
        hardware)
            ticonnect settings set calls.input_device ''
            ticonnect settings set calls.output_device ''
            echo "Call audio restored; use headphones to avoid feedback"
            ;;
        *)
            echo "usage: call-audio silent|hardware" >&2
            return 1
            ;;
    esac
}
lab-stop() { zellij action close-tab; }
help-lab() {
    cat <<'HELP'
node commands:
  ticonnect <args>   talk to this node's daemon
  trust              wait for automatic LAN trust
  user               print the user node id
  send "hello"       send to the user
  inbox [limit]      show messages from the user
  ui / stop-ui       start or stop this node's UI
  ovl / stop-ovl     start or stop this node's overlay
  call-audio silent|hardware  route calls to null devices or hardware
  logs [lines]       show the daemon log
  lab-stop           close the lab tab
HELP
}
echo "Node \$LAB_LABEL"
echo "  id:   \$NODE_ID"
echo "  user: \$USER_ID"
echo "  type 'help-lab' for commands"
EOF
}

lab_pair() {
    cat >"$WORK/bin/lab-pair" <<EOF
#!/usr/bin/env bash
set -euo pipefail
cli_a() { '$NODE_A/bin/ticonnect' "\$@"; }
cli_b() { '$NODE_B/bin/ticonnect' "\$@"; }
A_ID='$A_ID'
B_ID='$B_ID'
if cli_a users list | grep -q "\$B_ID"; then
    echo "already paired"
    exit 0
fi
for _ in \$(seq 1 600); do
    cli_a discover list | grep -q "\$B_ID" && break
    sleep 0.1
done
cli_a discover list | grep -q "\$B_ID" || { echo "user not discovered" >&2; exit 1; }
for _ in \$(seq 1 300); do
    cli_a users list | grep -q "\$B_ID" && cli_b users list | grep -q "\$A_ID" && break
    sleep 0.1
done
cli_a users list | grep -q "\$B_ID" && cli_b users list | grep -q "\$A_ID"
echo "trusted"
EOF
    chmod 755 "$WORK/bin/lab-pair"
}

configure_audio
start_bus "$BUS_A"
start_bus "$BUS_B"

PORT_A=$((40000 + RANDOM % 20000))
PORT_B=$((PORT_A + 1))
[ "$PORT_B" -lt 65535 ] || PORT_B=40000

start_node a "$BUS_A" "$NODE_A" "$PORT_A"
start_node b "$BUS_B" "$NODE_B" "$PORT_B"

wait_for "cli a ping" 600 || fail "node-a did not become ready"
wait_for "cli b ping" 600 || fail "node-b did not become ready"

A_ID="$(node_id a)"
B_ID="$(node_id b)"
[ -n "$A_ID" ] || fail "node-a has no id"
[ -n "$B_ID" ] || fail "node-b has no id"
[ "$A_ID" != "$B_ID" ] || fail "nodes share an identity"

cli a settings set identity.display_name "Lab Node A" >/dev/null
cli b settings set identity.display_name "Lab Node B" >/dev/null
mkdir -p "$WORK/bin"
if [ "$SMOKE" = "0" ] && [ "$HOT_RELOAD" = "1" ]; then
    start_dev_server
fi
gstreamer_env
gio_env
if [ -z "${TICONNECT_YTDLP:-}" ]; then
    TICONNECT_YTDLP="$(ls -d /nix/store/*-yt-dlp-*/bin/yt-dlp 2>/dev/null |
        sort | tail -n 1 || true)"
fi
[ -n "${TICONNECT_YTDLP:-}" ] ||
    fail "yt-dlp is required; run make lab from nix develop"
export TICONNECT_YTDLP
write_node_files a "$BUS_A" "$NODE_A" "$A_ID" "$B_ID" "Lab Node A" "Lab Node B"
write_node_files b "$BUS_B" "$NODE_B" "$B_ID" "$A_ID" "Lab Node B" "Lab Node A"
lab_pair

case "${LAB_CALL_AUDIO:-silent}" in
    silent)
        cli a settings set calls.input_device "$CALL_AUDIO_DISCARD"
        cli b settings set calls.input_device "$CALL_AUDIO_DISCARD"
        cli a settings set calls.output_device "$CALL_AUDIO_DISCARD"
        cli b settings set calls.output_device "$CALL_AUDIO_DISCARD"
        printf 'lab: call audio uses null devices to avoid same-host feedback\n'
        printf 'lab: run call-audio hardware in both panes and use headphones for real audio\n'
        ;;
    hardware)
        printf 'lab: call audio uses host devices; use headphones to avoid feedback\n'
        ;;
    *)
        fail "LAB_CALL_AUDIO must be silent or hardware"
        ;;
esac

if [ "$SMOKE" = "1" ]; then
    wait_for "cli a users list | grep -q '$B_ID'" 300 ||
        fail "node-a did not auto-trust node-b"
    wait_for "cli b users list | grep -q '$A_ID'" 300 ||
        fail "node-b did not auto-trust node-a"
    wait_for "cli b users list | grep -q 'Lab Node A'" 300 ||
        fail "node-b did not receive node-a display name"
    cli a settings set identity.display_name "Lab Node A Renamed" >/dev/null
    wait_for "cli b users list | grep -q 'Lab Node A Renamed'" 300 ||
        fail "node-b did not receive renamed display name"
    message_id="$(cli a send "$B_ID" "lab-smoke-$(date +%s)")"
    for _ in $(seq 1 100); do
        cli b messages "$A_ID" --limit 20 | grep -q "$message_id" && break
        sleep 0.1
    done
    cli b messages "$A_ID" --limit 20 | grep -q "$message_id" ||
        fail "smoke message did not arrive"

    B_FP="$(cli a users list | awk -v id="$B_ID" '$1 == id {print $2}')"
    [ -n "$B_FP" ] || fail "missing user fingerprint"
    cli a users forget "$B_ID" >/dev/null
    sleep 1
    if cli a users list | grep -q "$B_ID"; then
        fail "blocked user was re-trusted"
    fi
    cli a users unblock "$B_FP" >/dev/null
    wait_for "cli a users list | grep -q '$B_ID'" 300 ||
        fail "unblocked user did not re-trust"

    printf 'lab smoke passed: a=%s b=%s\n' "$A_ID" "$B_ID"
    exit 0
fi

zellij_action new-tab --name "$TAB_NAME" --cwd "$WORK" --close-on-exit -- \
    bash --rcfile "$NODE_A/bashrc" >/dev/null
zellij_action rename-pane node-a >/dev/null 2>&1 || true
zellij_action new-pane --direction right --name node-b --cwd "$WORK" --close-on-exit -- \
    bash --rcfile "$NODE_B/bashrc" >/dev/null

for _ in $(seq 1 100); do
    if zellij_tab_exists; then
        SEEN_TAB=1
        break
    fi
    sleep 0.1
done
[ "$SEEN_TAB" = "1" ] || fail "Zellij tab did not appear"

printf 'Lab tab %s is ready.\n' "$TAB_NAME"
printf 'Close the tab, run lab-stop, or press Ctrl-C here to stop.\n'
misses=0
while :; do
    if zellij_tab_exists; then
        misses=0
    else
        misses=$((misses + 1))
        [ "$misses" -ge 3 ] && break
    fi
    sleep 1
done
