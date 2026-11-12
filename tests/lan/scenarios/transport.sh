#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib.sh"

begin_scenario transport

export LAN_TEST_DISCOVERY_ENABLED=
export LAN_TEST_DISCOVERY_MDNS=
export LAN_TEST_DISCOVERY_BROADCAST=

start_nodes
wait_for 30 candidate_visible node-a "$B_ID" || fail "node-a did not discover node-b"
wait_for 30 candidate_visible node-b "$A_ID" || fail "node-b did not discover node-a"
pair_with_pin

log "checking connected transport state"
assert_eq "$(status_value node-a transport_state)" "online" "transport state"
assert_eq "$(status_value node-a active_sessions)" "1" "active sessions"
assert_contains "$(dbus_call node-a ListPeers)" "'connected': <true>" "peer connected"
monitor_start node-a transport-signals
monitor_start node-b transport-signals

log "checking reconnect and outbox retry after network loss"
network_disconnect node-b
wait_for 90 session_idle node-a || fail "node-a kept a session after network loss"
monitor_expect node-a transport-signals "PeerDisconnected"
assert_contains "$(dbus_call node-a ListPeers)" "'connected': <false>" "peer disconnected"

TOKEN_NET="$(unique_token net)"
MSG_NET="$(cli node-a send "$B_ID" "$TOKEN_NET")"
[ -n "$MSG_NET" ] || fail "send returned an empty message id"
wait_for 20 outbox_positive node-a || fail "offline message was not queued"
assert_eq "$(message_state node-a "$B_ID" "$MSG_NET")" "queued" "queued message state"

network_connect node-b
wait_for 60 candidate_visible node-a "$B_ID" || fail "node-b was not rediscovered"
wait_for 90 session_online node-a || fail "node-a did not reconnect"
wait_for 90 session_online node-b || fail "node-b did not reconnect"
monitor_expect node-a transport-signals "PeerConnected"
wait_for 60 outbox_empty node-a || fail "outbox did not drain after reconnect"
wait_for 60 message_body_is node-b "$A_ID" "$MSG_NET" "$TOKEN_NET" \
    || fail "node-b did not receive the queued message"

log "checking forget closes the session and blocks reconnect"
monitor_start node-a forget-signals
forget_peer node-a "$B_ID"
wait_for 30 session_idle node-a || fail "node-a kept a session after forget"
monitor_expect node-a forget-signals "PeerDisconnected"
assert_eq "$(cli node-a peers list)" "no peers" "forgotten peer"
peer_known node-b "$A_ID" || fail "node-b lost its trust row unexpectedly"
sleep 16
assert_eq "$(session_count node-a)" "0" "forgotten peer reconnected"
assert_eq "$(session_count node-b)" "0" "forgotten peer session stayed open"
pair_with_pin

log "checking custom transport port and restart requirement"
cli node-a settings set transport.port 45999 >/dev/null
cli node-b settings set transport.port 45999 >/dev/null
wait_for 20 setting_uint_is node-a transport.port 45999 \
    || fail "node-a did not reload transport.port"
wait_for 20 setting_uint_is node-b transport.port 45999 \
    || fail "node-b did not reload transport.port"

restart_nodes
wait_for 90 session_online node-a || fail "node-a did not reconnect after restart"
wait_for 90 session_online node-b || fail "node-b did not reconnect after restart"
assert_socket node-a 45999
assert_socket node-b 45999
wait_for 30 candidate_port_is node-a "$B_ID" 45999 \
    || fail "node-a did not advertise the new transport port"
wait_for 30 candidate_port_is node-b "$A_ID" 45999 \
    || fail "node-b did not advertise the new transport port"
assert_eq "$(candidate_port node-a "$B_ID")" "45999" "candidate port after restart"
TOKEN_PORT="$(unique_token port)"
MSG_PORT="$(cli node-a send "$B_ID" "$TOKEN_PORT")"
wait_for 60 message_body_is node-b "$A_ID" "$MSG_PORT" "$TOKEN_PORT" \
    || fail "messaging failed on the custom transport port"

log "transport scenario passed"
