#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="${TICONNECT_BIN:-$ROOT/target/debug/ticonnect}"
SMOKE=0

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

start_bus() {
    local bus="$1"
    dbus-daemon --session --nofork --nopidfile --nosyslog \
        --address="unix:path=$bus" >"$WORK/dbus.log" 2>&1 &
    PIDS+=("$!")
    for _ in $(seq 1 100); do
        [ -S "$bus" ] && return 0
        sleep 0.05
    done
    fail "private D-Bus did not start"
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
export LAB_DIR='$WORK'
export LAB_NODE_DIR='$dir'
export LAB_NODE='$node'
export LAB_LABEL='$label'
export LAB_USER_LABEL='$user_label'
export NODE_ID='$id'
export USER_ID='$user_id'
export PATH='$dir/bin':"\$PATH"
EOF

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
write_node_files a "$BUS_A" "$NODE_A" "$A_ID" "$B_ID" "Lab Node A" "Lab Node B"
write_node_files b "$BUS_B" "$NODE_B" "$B_ID" "$A_ID" "Lab Node B" "Lab Node A"
lab_pair

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
while zellij_tab_exists; do
    misses=0
    sleep 1
done
# Three failed queries in a row mean the Zellij server is gone.
while ! zellij_query_tabs >/dev/null 2>&1; do
    misses=$((misses + 1))
    [ "$misses" -ge 3 ] && break
    sleep 1
done
