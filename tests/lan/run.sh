#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPOSE_FILE="$ROOT/tests/lan/compose.yaml"
PROJECT="${LAN_TEST_PROJECT:-ticonnect-lan-$$}"
BUS_ADDRESS="unix:path=/tmp/ticonnect-bus"
DISCOVERY_ONLY="${LAN_TEST_DISCOVERY_ONLY:-0}"
EXPECTED_SOURCE="${LAN_TEST_EXPECT_SOURCE:-}"

export LAN_TEST_IMAGE="${LAN_TEST_IMAGE:-ticonnect-lan-test:${PROJECT}}"
export LAN_TEST_MDNS="${LAN_TEST_MDNS:-true}"
export LAN_TEST_BROADCAST="${LAN_TEST_BROADCAST:-true}"

COMPOSE=(docker compose -f "$COMPOSE_FILE" -p "$PROJECT")

log() {
    printf '%s\n' "$*"
}

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

cleanup() {
    local status=$?
    trap - EXIT

    if [ "$status" -ne 0 ]; then
        log "--- containers ---"
        "${COMPOSE[@]}" ps || true
        log "--- logs ---"
        "${COMPOSE[@]}" logs --no-color --tail=200 || true
        for node in node-a node-b; do
            log "--- $node daemon log ---"
            "${COMPOSE[@]}" exec -T "$node" tail -n 200 /data/ticonnect.log || true
            log "--- $node status ---"
            cli "$node" status || true
            log "--- $node candidates ---"
            cli "$node" discover list || true
            log "--- $node peers ---"
            cli "$node" peers list || true
            log "--- $node interfaces ---"
            "${COMPOSE[@]}" exec -T "$node" ip -4 addr show eth0 || true
            log "--- $node sockets ---"
            "${COMPOSE[@]}" exec -T "$node" ss -lunp || true
        done
    fi

    if [ "${LAN_TEST_KEEP:-0}" = "1" ]; then
        log "keeping containers for project $PROJECT"
        exit "$status"
    fi

    "${COMPOSE[@]}" down -v --remove-orphans >/dev/null 2>&1 || true
    docker image rm -f "$LAN_TEST_IMAGE" >/dev/null 2>&1 || true
    exit "$status"
}
trap cleanup EXIT

cli() {
    local node="$1"
    shift
    "${COMPOSE[@]}" exec -T \
        -e "DBUS_SESSION_BUS_ADDRESS=$BUS_ADDRESS" \
        -e "TICONNECT_LOG_LEVEL=off" \
        "$node" ticonnect-cli "$@"
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

node_id() {
    cli "$1" id show 2>/dev/null | awk '/^node_id:/ { print $2; exit }'
}

status_value() {
    local node="$1"
    local key="$2"
    local output
    output="$(cli "$node" status 2>/dev/null || true)"
    printf '%s\n' "$output" | awk -v key="$key:" '$1 == key { print $2; exit }'
}

daemon_ready() {
    local output
    output="$(cli "$1" ping 2>/dev/null || true)"
    [ "$output" = "pong" ]
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

peer_known() {
    local output
    output="$(cli "$1" peers list 2>/dev/null || true)"
    printf '%s\n' "$output" | awk -v peer="$2" '$1 == peer { found = 1 } END { exit !found }'
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

log "building LAN test image"
"${COMPOSE[@]}" build

log "starting nodes"
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

log "waiting for discovery"
wait_for 30 candidate_visible node-a "$B_ID" || fail "node-a did not discover node-b"
wait_for 30 candidate_visible node-b "$A_ID" || fail "node-b did not discover node-a"

A_SOURCE="$(candidate_source node-a "$B_ID")"
B_SOURCE="$(candidate_source node-b "$A_ID")"
log "discovery sources: node-a sees $A_SOURCE, node-b sees $B_SOURCE"

if [ -n "$EXPECTED_SOURCE" ]; then
    wait_for 15 source_is node-a "$B_ID" "$EXPECTED_SOURCE" \
        || fail "node-a discovery source was $A_SOURCE, expected $EXPECTED_SOURCE"
    wait_for 15 source_is node-b "$A_ID" "$EXPECTED_SOURCE" \
        || fail "node-b discovery source was $B_SOURCE, expected $EXPECTED_SOURCE"
fi

if [ "$DISCOVERY_ONLY" = "1" ]; then
    log "discovery-only test passed"
    exit 0
fi

log "pairing with PIN"
PAIR_OUTPUT="$(cli node-a pair start)"
A_PIN="$(printf '%s\n' "$PAIR_OUTPUT" | sed -n 's/^pin: //p' | head -n 1)"
[ -n "$A_PIN" ] || fail "node-a did not return a pairing PIN"
cli node-b pair with-pin "$A_ID" "$A_PIN" >/dev/null
wait_for 30 peer_known node-a "$B_ID" || fail "node-a did not trust node-b"
wait_for 30 peer_known node-b "$A_ID" || fail "node-b did not trust node-a"
wait_for 30 session_online node-a || fail "node-a did not open a transport session"
wait_for 30 session_online node-b || fail "node-b did not open a transport session"
log "PIN pairing passed"

log "pairing with QR"
cli node-a pair forget "$B_ID" >/dev/null
cli node-b pair forget "$A_ID" >/dev/null
wait_for 20 session_idle node-a || fail "node-a session did not close after forget"
wait_for 20 session_idle node-b || fail "node-b session did not close after forget"

PAIR_OUTPUT="$(cli node-a pair start)"
A_QR="$(printf '%s\n' "$PAIR_OUTPUT" | sed -n 's/^qr: //p' | head -n 1)"
[ -n "$A_QR" ] || fail "node-a did not return a QR payload"
cli node-b pair with-qr "$A_QR" >/dev/null
wait_for 30 peer_known node-a "$B_ID" || fail "node-a did not trust node-b after QR pairing"
wait_for 30 peer_known node-b "$A_ID" || fail "node-b did not trust node-a after QR pairing"
wait_for 30 session_online node-a || fail "node-a did not open a session after QR pairing"
wait_for 30 session_online node-b || fail "node-b did not open a session after QR pairing"
log "QR pairing passed"

TOKEN_A2B="lan-a2b-$(date +%s)-$$"
log "sending node-a -> node-b"
MSG_A2B="$(cli node-a send "$B_ID" "$TOKEN_A2B")"
[ -n "$MSG_A2B" ] || fail "node-a send returned an empty message id"
wait_for 30 message_body_is node-b "$A_ID" "$MSG_A2B" "$TOKEN_A2B" \
    || fail "node-b did not receive the message"
wait_for 30 message_state_is node-a "$B_ID" "$MSG_A2B" delivered \
    || fail "node-a did not observe the delivered receipt"

cli node-b mark-read "$A_ID" "$MSG_A2B" >/dev/null
wait_for 30 message_state_is node-a "$B_ID" "$MSG_A2B" read \
    || fail "node-a did not observe the read receipt"
log "node-a -> node-b messaging passed"

TOKEN_B2A="lan-b2a-$(date +%s)-$$"
log "sending node-b -> node-a"
MSG_B2A="$(cli node-b send "$A_ID" "$TOKEN_B2A")"
[ -n "$MSG_B2A" ] || fail "node-b send returned an empty message id"
wait_for 30 message_body_is node-a "$B_ID" "$MSG_B2A" "$TOKEN_B2A" \
    || fail "node-a did not receive the message"
log "node-b -> node-a messaging passed"

log "testing offline queue and reconnect"
"${COMPOSE[@]}" stop node-b >/dev/null
wait_for 20 candidate_gone node-a "$B_ID" || fail "node-a still sees node-b after stop"

TOKEN_OFFLINE="lan-offline-$(date +%s)-$$"
MSG_OFFLINE="$(cli node-a send "$B_ID" "$TOKEN_OFFLINE")"
[ -n "$MSG_OFFLINE" ] || fail "node-a send returned an empty offline message id"
wait_for 20 outbox_positive node-a || fail "node-a did not queue the offline message"

"${COMPOSE[@]}" start node-b >/dev/null
wait_for 90 daemon_ready node-b || fail "node-b daemon did not restart"
wait_for 60 candidate_visible node-a "$B_ID" || fail "node-a did not rediscover node-b"
wait_for 60 message_body_is node-b "$A_ID" "$MSG_OFFLINE" "$TOKEN_OFFLINE" \
    || fail "node-b did not receive the queued message after restart"
wait_for 60 outbox_empty node-a || fail "node-a outbox did not drain"
log "offline queue and reconnect passed"

log "LAN integration test passed"
