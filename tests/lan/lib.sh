#!/usr/bin/env bash

set -euo pipefail

LAN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$LAN_DIR/compose.yaml"
BUS_ADDRESS="unix:path=/tmp/ticonnect-bus"

SCENARIO=""
PROJECT=""
COMPOSE=()
A_ID=""
B_ID=""
LAST_OUTPUT=""
PAIR_START=""
PAIR_PIN=""
PAIR_QR=""
PAIR_EXPIRES=""
PAIR_PORT=""

log() {
    printf '%s\n' "$*"
}

fail() {
    printf 'FAIL [%s]: %s\n' "${SCENARIO:-lan}" "$*" >&2
    exit 1
}

begin_scenario() {
    SCENARIO="$1"
    PROJECT="${LAN_TEST_PROJECT:-ticonnect-lan}-${SCENARIO}-$$"
    COMPOSE=(docker compose -f "$COMPOSE_FILE" -p "$PROJECT")
    trap cleanup EXIT
}

cleanup() {
    local status=$?

    trap - EXIT

    if [ "$status" -ne 0 ] || [ "${LAN_TEST_DUMP:-0}" = "1" ]; then
        dump_state
    fi

    if [ "${LAN_TEST_KEEP:-0}" = "1" ]; then
        log "keeping containers for project $PROJECT"
        exit "$status"
    fi

    "${COMPOSE[@]}" down -v --remove-orphans >/dev/null 2>&1 || true
    exit "$status"
}

dump_state() {
    log "--- compose ps ---"
    "${COMPOSE[@]}" ps || true
    log "--- compose logs ---"
    "${COMPOSE[@]}" logs --no-color --tail=200 || true

    local node
    for node in node-a node-b; do
        log "--- $node daemon log ---"
        "${COMPOSE[@]}" exec -T "$node" tail -n 300 /data/ticonnect.log || true
        log "--- $node status ---"
        cli "$node" status || true
        log "--- $node discover status ---"
        dbus_call "$node" GetDiscoveryStatus || true
        log "--- $node candidates ---"
        cli "$node" discover list || true
        log "--- $node peers ---"
        cli "$node" peers list || true
        log "--- $node interfaces ---"
        "${COMPOSE[@]}" exec -T "$node" ip -4 addr show eth0 || true
        log "--- $node sockets ---"
        "${COMPOSE[@]}" exec -T "$node" ss -lunp || true
        log "--- $node signal monitors ---"
        "${COMPOSE[@]}" exec -T "$node" sh -c '
            for file in /data/*.signals /data/*.watch; do
                [ -e "$file" ] || continue
                printf "== %s ==\n" "$file"
                cat "$file"
            done
        ' || true
        log "--- $node database ---"
        "${COMPOSE[@]}" exec -T "$node" sqlite3 /data/data/ticonnect.db \
            '.tables' 2>/dev/null || true
    done
}

start_nodes() {
    log "starting $SCENARIO nodes"
    "${COMPOSE[@]}" up -d --remove-orphans

    wait_for 90 daemon_ready node-a || fail "node-a daemon did not start"
    wait_for 90 daemon_ready node-b || fail "node-b daemon did not start"

    A_ID="$(node_id node-a)"
    B_ID="$(node_id node-b)"
    [ -n "$A_ID" ] || fail "node-a returned an empty node id"
    [ -n "$B_ID" ] || fail "node-b returned an empty node id"
    [ "$A_ID" != "$B_ID" ] || fail "nodes returned the same node id"

    log "node-a: $A_ID"
    log "node-b: $B_ID"
}

reset_nodes() {
    "${COMPOSE[@]}" down -v --remove-orphans >/dev/null 2>&1 || true
    start_nodes
}

start_node() {
    "${COMPOSE[@]}" start "$1" >/dev/null
    wait_for 90 daemon_ready "$1" || fail "$1 daemon did not start"
}

stop_node() {
    "${COMPOSE[@]}" stop "$1" >/dev/null
}

restart_node() {
    stop_node "$1"
    start_node "$1"
}

restart_nodes() {
    stop_node node-a
    stop_node node-b
    start_node node-a
    start_node node-b
}

container_id() {
    "${COMPOSE[@]}" ps -q "$1"
}

network_name() {
    docker network ls --format '{{.Name}}' | grep -Fx "${PROJECT}_lan"
}

network_disconnect() {
    local node="$1"
    local network
    local id
    network="$(network_name)" || fail "compose network not found for $PROJECT"
    id="$(container_id "$node")"
    [ -n "$id" ] || fail "container id not found for $node"
    docker network disconnect "$network" "$id"
}

network_connect() {
    local node="$1"
    local network
    local id
    network="$(network_name)" || fail "compose network not found for $PROJECT"
    id="$(container_id "$node")"
    [ -n "$id" ] || fail "container id not found for $node"
    docker network connect "$network" "$id"
}

cli() {
    local node="$1"
    shift

    timeout "${LAN_TEST_CLI_TIMEOUT:-60}" \
        "${COMPOSE[@]}" exec -T \
        -e "DBUS_SESSION_BUS_ADDRESS=$BUS_ADDRESS" \
        -e "TICONNECT_LOG_LEVEL=off" \
        "$node" ticonnect-cli "$@"
}

cli_expect_fail() {
    local node="$1"
    shift
    local output

    if output="$(cli "$node" "$@" 2>&1)"; then
        fail "expected command to fail: $node $*"
    fi
    LAST_OUTPUT="$output"
}

daemon_ready() {
    local output
    output="$(cli "$1" ping 2>/dev/null || true)"
    [ "$output" = "pong" ]
}

daemon_down() {
    ! daemon_ready "$1"
}

node_id() {
    cli "$1" id show 2>/dev/null | awk '/^node_id:/ { print $2; exit }'
}

fingerprint_raw() {
    cli "$1" id show 2>/dev/null \
        | awk -F': ' '/^fingerprint:/ { gsub(/ /, "", $2); print $2; exit }'
}

display_name() {
    cli "$1" id show 2>/dev/null \
        | awk -F': ' '/^display_name:/ { print $2; exit }'
}

status_value() {
    local node="$1"
    local key="$2"
    local output
    output="$(cli "$node" status 2>/dev/null || true)"
    printf '%s\n' "$output" | awk -v key="$key:" '$1 == key { print $2; exit }'
}

status_json_value() {
    cli "$1" status --json | "${COMPOSE[@]}" exec -T -i "$1" jq -r ".$2"
}

candidate_visible() {
    local output
    output="$(cli "$1" discover list 2>/dev/null || true)"
    printf '%s\n' "$output" | awk -v peer="$2" '$1 == peer { found = 1 } END { exit !found }'
}

candidate_source() {
    local output
    output="$(cli "$1" discover list 2>/dev/null || true)"
    printf '%s\n' "$output" | awk -v peer="$2" '$1 == peer { print $3; exit }'
}

candidate_gone() {
    ! candidate_visible "$1" "$2"
}

source_is() {
    [ "$(candidate_source "$1" "$2")" = "$3" ]
}

candidate_port() {
    local output
    output="$(dbus_call "$1" ListCandidates 2>/dev/null || true)"
    printf '%s\n' "$output" | tr '}' '\n' \
        | grep -F "'node_id': <'$2'>" \
        | sed -n "s/.*'port': <uint32 \([0-9]*\)>.*/\1/p" \
        | head -n 1
}

candidate_port_is() {
    [ "$(candidate_port "$1" "$2")" = "$3" ]
}

candidate_has_field() {
    local output
    output="$(dbus_call "$1" ListCandidates 2>/dev/null || true)"
    printf '%s\n' "$output" | grep -Fq "$2"
}

peer_known() {
    local output
    output="$(cli "$1" peers list 2>/dev/null || true)"
    printf '%s\n' "$output" | awk -v peer="$2" '$1 == peer { found = 1 } END { exit !found }'
}

peer_policy() {
    local output
    output="$(cli "$1" peers list 2>/dev/null || true)"
    printf '%s\n' "$output" | awk -v peer="$2" '$1 == peer { print $3; exit }'
}

peer_policy_is() {
    [ "$(peer_policy "$1" "$2")" = "$3" ]
}

peer_field_contains() {
    local output
    output="$(dbus_call "$1" ListPeers 2>/dev/null || true)"
    printf '%s\n' "$output" | grep -Fq "$2"
}

session_count() {
    status_value "$1" "active_sessions"
}

session_online() {
    local sessions
    sessions="$(session_count "$1")"
    [ -n "$sessions" ] && [ "$sessions" -ge 1 ]
}

session_idle() {
    [ "$(session_count "$1")" = "0" ]
}

outbox_depth() {
    status_value "$1" "outbox_depth"
}

outbox_positive() {
    local depth
    depth="$(outbox_depth "$1")"
    [ -n "$depth" ] && [ "$depth" -ge 1 ]
}

outbox_empty() {
    [ "$(outbox_depth "$1")" = "0" ]
}

message_body_is() {
    local output
    output="$(cli "$1" messages "$2" 2>/dev/null || true)"
    printf '%s\n' "$output" | awk -v id="$3" -v token="$4" \
        '$1 == id && index($0, token) { found = 1 } END { exit !found }'
}

message_state() {
    local output
    output="$(cli "$1" messages "$2" 2>/dev/null || true)"
    printf '%s\n' "$output" | awk -v id="$3" '$1 == id { print $2; exit }'
}

message_state_is() {
    [ "$(message_state "$1" "$2" "$3")" = "$4" ]
}

message_count() {
    local output
    output="$(cli "$1" messages "$2" 2>/dev/null || true)"
    if [ -z "$output" ] || [ "$output" = "no messages" ]; then
        printf '0\n'
    else
        printf '%s\n' "$output" | wc -l | tr -d ' '
    fi
}

message_count_is() {
    [ "$(message_count "$1" "$2")" = "$3" ]
}

history_has_token() {
    local output
    output="$(cli "$1" messages "$2" 2>/dev/null || true)"
    printf '%s\n' "$output" | grep -Fq "$3"
}

history_lacks_token() {
    ! history_has_token "$1" "$2" "$3"
}

start_pairing() {
    PAIR_START="$(cli node-a pair start)"
    PAIR_PIN="$(printf '%s\n' "$PAIR_START" | sed -n 's/^pin: //p' | head -n 1)"
    PAIR_QR="$(printf '%s\n' "$PAIR_START" | sed -n 's/^qr: //p' | head -n 1)"
    PAIR_EXPIRES="$(printf '%s\n' "$PAIR_START" | sed -n 's/^expires_at: //p' | head -n 1)"
    PAIR_PORT="$(printf '%s\n' "$PAIR_START" | sed -n 's/^port: //p' | head -n 1)"
    [ -n "$PAIR_PIN" ] || fail "pair start returned an empty PIN"
    [ -n "$PAIR_QR" ] || fail "pair start returned an empty QR payload"
    [ -n "$PAIR_EXPIRES" ] || fail "pair start returned an empty expiry"
    [ -n "$PAIR_PORT" ] || fail "pair start returned an empty port"
}

wait_trusted() {
    wait_for 60 peer_known node-a "$B_ID" || fail "node-a did not trust node-b"
    wait_for 60 peer_known node-b "$A_ID" || fail "node-b did not trust node-a"
}

wait_online() {
    wait_for 60 session_online node-a || fail "node-a did not open a session"
    wait_for 60 session_online node-b || fail "node-b did not open a session"
}

wait_idle() {
    wait_for 30 session_idle node-a || fail "node-a session did not close"
    wait_for 30 session_idle node-b || fail "node-b session did not close"
}

pair_with_pin() {
    start_pairing
    cli node-b pair with-pin "$A_ID" "$PAIR_PIN" >/dev/null
    wait_trusted
    wait_online
}

pair_with_qr() {
    start_pairing
    cli node-b pair with-qr "$PAIR_QR" >/dev/null
    wait_trusted
    wait_online
}

forget_peer() {
    local node="$1"
    local peer="$2"
    cli "$node" pair forget "$peer" >/dev/null
}

forget_both() {
    forget_peer node-a "$B_ID" || true
    forget_peer node-b "$A_ID" || true
    wait_idle
}

monitor_start() {
    local node="$1"
    local name="$2"

    monitor_stop "$node" "$name" >/dev/null 2>&1 || true

    "${COMPOSE[@]}" exec -T \
        -e "DBUS_SESSION_BUS_ADDRESS=$BUS_ADDRESS" \
        "$node" sh -c "
            rm -f /data/${name}.signals
            stdbuf -oL gdbus monitor --session --dest org.ticonnect.Daemon \
                --object-path /org/ticonnect/Daemon > /data/${name}.signals 2>&1 &
            echo \$! > /data/${name}.pid
        "
    sleep 0.5
}

monitor_stop() {
    local node="$1"
    local name="$2"

    "${COMPOSE[@]}" exec -T "$node" sh -c "
        if [ -f /data/${name}.pid ]; then
            kill \"\$(cat /data/${name}.pid)\" 2>/dev/null || true
            rm -f /data/${name}.pid
        fi
    " >/dev/null 2>&1 || true
}

signal_seen() {
    "${COMPOSE[@]}" exec -T "$1" grep -qF "$3" "/data/$2.signals"
}

monitor_expect() {
    local node="$1"
    local name="$2"
    local pattern="$3"
    local timeout="${4:-15}"

    wait_for "$timeout" signal_seen "$node" "$name" "$pattern" \
        || fail "signal '$pattern' not seen on $node"
}

monitor_line() {
    "${COMPOSE[@]}" exec -T "$1" grep -F "$3" "/data/$2.signals" || true
}

monitor_line_contains() {
    local node="$1"
    local name="$2"
    local token="$3"
    local needle="$4"

    local lines
    lines="$(monitor_line "$node" "$name" "$token")"
    printf '%s\n' "$lines" | grep -Fq "$needle"
}

cli_watch_start() {
    local node="$1"
    local name="$2"
    shift 2

    "${COMPOSE[@]}" exec -d -T \
        -e "DBUS_SESSION_BUS_ADDRESS=$BUS_ADDRESS" \
        -e "TICONNECT_LOG_LEVEL=off" \
        "$node" sh -c "rm -f /data/${name}.watch; timeout 90 stdbuf -oL ticonnect-cli $* > /data/${name}.watch 2>&1"
}

watch_seen() {
    "${COMPOSE[@]}" exec -T "$1" grep -qF "$3" "/data/$2.watch"
}

watch_expect() {
    local node="$1"
    local name="$2"
    local pattern="$3"
    local timeout="${4:-15}"

    wait_for "$timeout" watch_seen "$node" "$name" "$pattern" \
        || fail "watch output '$pattern' not seen on $node"
}

dbus_call() {
    local node="$1"
    local method="$2"
    shift 2

    timeout "${LAN_TEST_CLI_TIMEOUT:-60}" \
        "${COMPOSE[@]}" exec -T \
        -e "DBUS_SESSION_BUS_ADDRESS=$BUS_ADDRESS" \
        "$node" gdbus call --session \
        --dest org.ticonnect.Daemon \
        --object-path /org/ticonnect/Daemon \
        --method "org.ticonnect.Daemon.$method" "$@"
}

dbus_uint_text() {
    dbus_call "$@" | grep -o 'uint32 [0-9]*' | awk '{print $2}' | head -n 1
}

dbus_map_entry() {
    dbus_call "$1" "$2" | tr ',' '\n' | grep -F "'$3':" | head -n 1
}

dbus_map_bool() {
    dbus_map_entry "$1" "$2" "$3" | grep -q '<true>'
}

dbus_map_uint() {
    dbus_map_entry "$1" "$2" "$3" \
        | sed -n 's/.*uint32 \([0-9]*\).*/\1/p'
}

dbus_map_number() {
    dbus_map_entry "$1" "$2" "$3" \
        | sed -n 's/.*uint\(32\|64\) \([0-9]*\).*/\2/p'
}

assert_map_bool() {
    local node="$1"
    local method="$2"
    local key="$3"
    local expected="$4"
    local actual

    if dbus_map_bool "$node" "$method" "$key"; then
        actual=true
    else
        actual=false
    fi
    assert_eq "$actual" "$expected" "$method.$key"
}

dbus_map_string() {
    dbus_map_entry "$1" "$2" "$3" \
        | sed -n "s/.*<'\([^']*\)'>.*/\1/p"
}

setting_bool_text() {
    dbus_call "$1" GetSetting "$2" \
        | sed -n 's/.*<\(true\|false\)>.*/\1/p'
}

setting_bool_is() {
    [ "$(setting_bool_text "$1" "$2")" = "$3" ]
}

setting_string_text() {
    dbus_call "$1" GetSetting "$2" \
        | sed -n "s/^('\(.*\)',)$/\1/p; s/.*<'\(.*\)'>.*/\1/p"
}

setting_string_is() {
    [ "$(setting_string_text "$1" "$2")" = "$3" ]
}

setting_uint_text() {
    dbus_call "$1" GetSetting "$2" \
        | sed -n 's/.*uint32 \([0-9]*\).*/\1/p'
}

setting_uint_is() {
    [ "$(setting_uint_text "$1" "$2")" = "$3" ]
}

sqlite_query() {
    "${COMPOSE[@]}" exec -T "$1" sqlite3 /data/data/ticonnect.db "$2"
}

sqlite_exec_stopped() {
    local node="$1"
    local sql="$2"

    docker run --rm --entrypoint sqlite3 \
        -v "${PROJECT}_${node}-data:/data" \
        "$LAN_TEST_IMAGE" /data/data/ticonnect.db "$sql"
}

volume_read() {
    local node="$1"
    local path="$2"

    docker run --rm --entrypoint sh \
        -v "${PROJECT}_${node}-data:/data" \
        "$LAN_TEST_IMAGE" -c "cat '$path'"
}

volume_contains() {
    volume_read "$1" "$2" | grep -qF "$3"
}

conversation_for_message() {
    sqlite_query "$1" "SELECT conversation_id FROM messages WHERE id='$2' LIMIT 1;"
}

message_seq() {
    sqlite_query "$1" "SELECT seq FROM messages WHERE id='$2' LIMIT 1;"
}

peer_public_key_len() {
    sqlite_query "$1" "SELECT length(public_key) FROM peers WHERE id='$2' LIMIT 1;"
}

node_ip() {
    "${COMPOSE[@]}" exec -T "$1" sh -c \
        "ip -4 -o addr show eth0 | awk '{print \$4}' | cut -d/ -f1"
}

udp_send_garbage() {
    local from="$1"
    local to_ip="$2"
    "${COMPOSE[@]}" exec -T "$from" bash -c \
        "printf 'not-a-ticonnect-beacon' > /dev/udp/${to_ip}/45870"
}

assert_eq() {
    local actual="$1"
    local expected="$2"
    local message="${3:-assertion failed}"
    [ "$actual" = "$expected" ] || fail "$message: expected '$expected', got '$actual'"
}

assert_ne() {
    local actual="$1"
    local unexpected="$2"
    local message="${3:-assertion failed}"
    [ "$actual" != "$unexpected" ] || fail "$message: did not expect '$unexpected'"
}

assert_contains() {
    local haystack="$1"
    local needle="$2"
    local message="${3:-assertion failed}"
    case "$haystack" in
        *"$needle"*) ;;
        *) fail "$message: missing '$needle' in '$haystack'" ;;
    esac
}

assert_not_contains() {
    local haystack="$1"
    local needle="$2"
    local message="${3:-assertion failed}"
    case "$haystack" in
        *"$needle"*) fail "$message: unexpected '$needle' in '$haystack'" ;;
        *) ;;
    esac
}

assert_matches() {
    local actual="$1"
    local pattern="$2"
    local message="${3:-assertion failed}"
    [[ "$actual" =~ $pattern ]] || fail "$message: '$actual' does not match '$pattern'"
}

assert_mode() {
    local node="$1"
    local path="$2"
    local expected="$3"
    local actual
    actual="$("${COMPOSE[@]}" exec -T "$node" stat -c '%a' "$path")"
    assert_eq "$actual" "$expected" "$node $path mode"
}

assert_socket() {
    local node="$1"
    local port="$2"
    if ! "${COMPOSE[@]}" exec -T "$node" sh -c "ss -lun | grep -q ':$port '"; then
        fail "$node is not listening on UDP port $port"
    fi
}

log_contains() {
    "${COMPOSE[@]}" exec -T "$1" grep -qF "$2" /data/ticonnect.log
}

assert_log_absent() {
    local node="$1"
    local needle="$2"
    local message="${3:-secret leaked to log}"
    if log_contains "$node" "$needle"; then
        fail "$message: '$needle' found in $node log"
    fi
}

wait_for() {
    local timeout="$1"
    shift
    local deadline=$((SECONDS + timeout))

    while [ "$SECONDS" -lt "$deadline" ]; do
        if "$@"; then
            return 0
        fi
        sleep 1
    done
    return 1
}

unique_token() {
    printf '%s-%s-%s-%s' "$1" "$(date +%s)" "$$" "$RANDOM"
}
