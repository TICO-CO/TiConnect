#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib.sh"

public_key_present() {
    local length
    length="$(user_public_key_len "$1" "$2")"
    [ -n "$length" ] && [ "$length" -gt 0 ]
}

begin_scenario security

export LAN_TEST_DISCOVERY_ENABLED=
export LAN_TEST_DISCOVERY_MDNS=
export LAN_TEST_DISCOVERY_BROADCAST=

start_nodes
wait_for 30 candidate_visible node-a "$B_ID" || fail "node-a did not discover node-b"
wait_for 30 candidate_visible node-b "$A_ID" || fail "node-b did not discover node-a"

A_FP="$(fingerprint_raw node-a)"
B_FP="$(fingerprint_raw node-b)"
assert_eq "$(candidate_port node-b "$A_ID")" "45871" "discovered candidate port"
assert_contains "$(dbus_call node-b ListCandidates)" "'fingerprint': <'$A_FP'>" \
    "discovered fingerprint matches identity"

log "checking automatic trust"
wait_trusted
wait_online
assert_contains "$(dbus_call node-a ListUsers)" "'id': <'$B_ID'>" "trusted user"

log "checking trust, key binding, and permissions"
wait_for 30 public_key_present node-a "$B_ID" \
    || fail "node-a did not store node-b public key"
wait_for 30 public_key_present node-b "$A_ID" \
    || fail "node-b did not store node-a public key"
assert_eq "$(fingerprint_raw node-a)" "$A_FP" "node-a fingerprint changed"
assert_eq "$(fingerprint_raw node-b)" "$B_FP" "node-b fingerprint changed"

assert_mode node-a /data/config 700
assert_mode node-a /data/data 700
assert_mode node-a /data/config/identity.key 600
assert_mode node-a /data/config/identity.toml 600
assert_mode node-a /data/config/config.toml 600
assert_mode node-a /data/data/ticonnect.db 600

log "checking log redaction"
log_contains node-a "daemon started" || fail "node-a log is missing startup output"
TOKEN_SECRET="$(unique_token secret)"
MSG_SECRET="$(cli node-a send "$B_ID" "$TOKEN_SECRET")"
wait_for 30 message_body_is node-b "$A_ID" "$MSG_SECRET" "$TOKEN_SECRET" \
    || fail "secret test message was not delivered"
assert_log_absent node-a "$TOKEN_SECRET" "message body leaked"
assert_log_absent node-b "$TOKEN_SECRET" "message body leaked"

log "checking identity, trust, and history persistence"
restart_nodes
wait_for 90 session_online node-a || fail "node-a did not reconnect after restart"
wait_for 90 session_online node-b || fail "node-b did not reconnect after restart"
assert_eq "$(node_id node-a)" "$A_ID" "node-a id after restart"
assert_eq "$(node_id node-b)" "$B_ID" "node-b id after restart"
assert_eq "$(fingerprint_raw node-a)" "$A_FP" "node-a fingerprint after restart"
assert_eq "$(fingerprint_raw node-b)" "$B_FP" "node-b fingerprint after restart"
user_known node-a "$B_ID" || fail "node-a trust did not survive restart"
user_known node-b "$A_ID" || fail "node-b trust did not survive restart"
TOKEN_RESTART="$(unique_token restart)"
MSG_RESTART="$(cli node-a send "$B_ID" "$TOKEN_RESTART")"
wait_for 60 message_body_is node-b "$A_ID" "$MSG_RESTART" "$TOKEN_RESTART" \
    || fail "messaging failed after restart"
assert_log_absent node-a "$TOKEN_SECRET" "message body leaked after restart"
assert_log_absent node-b "$TOKEN_SECRET" "message body leaked after restart"

log "security scenario passed"
