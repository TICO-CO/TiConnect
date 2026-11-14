#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib.sh"

begin_scenario offline-sync

export LAN_TEST_DISCOVERY_ENABLED=
export LAN_TEST_DISCOVERY_MDNS=
export LAN_TEST_DISCOVERY_BROADCAST=

start_nodes
wait_for 30 candidate_visible node-a "$B_ID" || fail "node-a did not discover node-b"
wait_for 30 candidate_visible node-b "$A_ID" || fail "node-b did not discover node-a"
pair_with_pin

log "checking offline queue with multiple messages"
stop_node node-b
wait_for 20 candidate_gone node-a "$B_ID" || fail "node-b stayed visible after stop"
OFFLINE_TOKENS=()
OFFLINE_IDS=()
for index in 1 2 3; do
    token="$(unique_token "offline-$index")"
    id="$(cli node-a send "$B_ID" "$token")"
    OFFLINE_TOKENS+=("$token")
    OFFLINE_IDS+=("$id")
done
wait_for 20 outbox_positive node-a || fail "offline messages were not queued"
[ "$(outbox_depth node-a)" -ge 3 ] || fail "outbox depth is less than three"
for id in "${OFFLINE_IDS[@]}"; do
    assert_eq "$(message_state node-a "$B_ID" "$id")" "queued" "offline message state"
done
start_node node-b
wait_for 60 candidate_visible node-a "$B_ID" || fail "node-b was not rediscovered"
wait_for 90 session_online node-a || fail "node-a did not reconnect"
wait_for 90 session_online node-b || fail "node-b did not reconnect"
wait_for 60 outbox_empty node-a || fail "offline outbox did not drain"
for index in 0 1 2; do
    wait_for 60 message_body_is node-b "$A_ID" "${OFFLINE_IDS[$index]}" "${OFFLINE_TOKENS[$index]}" \
        || fail "offline message $index was not delivered"
done

log "checking gap sync after receiver data loss"
TOKEN_SYNC="$(unique_token sync)"
MSG_SYNC="$(cli node-a send "$B_ID" "$TOKEN_SYNC")"
wait_for 30 message_body_is node-b "$A_ID" "$MSG_SYNC" "$TOKEN_SYNC" \
    || fail "gap-sync seed message was not delivered"
SYNC_SEQ="$(message_seq node-b "$MSG_SYNC")"
[ -n "$SYNC_SEQ" ] || fail "could not read the message sequence"
stop_node node-b
sqlite_exec_stopped node-b "
    DELETE FROM messages WHERE id='$MSG_SYNC';
    UPDATE peer_state SET last_recv_seq=$((SYNC_SEQ - 1)) WHERE peer_id='$A_ID';
"
assert_eq "$(sqlite_exec_stopped node-b "SELECT COUNT(*) FROM messages WHERE id='$MSG_SYNC';")" \
    "0" "gap-sync seed message was not removed"
monitor_start node-a sync-signals
start_node node-b
wait_for 90 session_online node-b || fail "node-b did not reconnect for gap sync"
wait_for 60 message_body_is node-b "$A_ID" "$MSG_SYNC" "$TOKEN_SYNC" \
    || fail "gap sync did not restore the missing message"
monitor_expect node-a sync-signals "SyncCompleted"

log "checking outbox expiry"
stop_node node-b
TOKEN_EXPIRE="$(unique_token expire)"
MSG_EXPIRE="$(cli node-a send "$B_ID" "$TOKEN_EXPIRE")"
wait_for 20 outbox_positive node-a || fail "expiring message was not queued"
stop_node node-a
sqlite_exec_stopped node-a "
    UPDATE outbox SET expires_at=1 WHERE message_id='$MSG_EXPIRE';
"
start_node node-a
wait_for 30 message_state_is node-a "$B_ID" "$MSG_EXPIRE" expired \
    || fail "outbox row did not expire"
wait_for 20 outbox_empty node-a || fail "expired outbox row was not removed"
start_node node-b
wait_for 90 session_online node-a || fail "node-a did not reconnect after expiry"
wait_for 90 session_online node-b || fail "node-b did not reconnect after expiry"
sleep 2
history_lacks_token node-b "$A_ID" "$TOKEN_EXPIRE" \
    || fail "expired message was delivered"

log "checking persistence after both nodes restart"
restart_nodes
wait_for 90 session_online node-a || fail "node-a did not reconnect after restart"
wait_for 90 session_online node-b || fail "node-b did not reconnect after restart"
TOKEN_RESTART="$(unique_token restart)"
MSG_RESTART="$(cli node-a send "$B_ID" "$TOKEN_RESTART")"
wait_for 60 message_body_is node-b "$A_ID" "$MSG_RESTART" "$TOKEN_RESTART" \
    || fail "messaging failed after restart"
assert_eq "$(peer_policy node-a "$B_ID")" "accept" "peer policy after restart"
assert_eq "$(peer_policy node-b "$A_ID")" "accept" "peer policy after restart"

log "offline-sync scenario passed"
