#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib.sh"

public_key_present() {
    local length
    length="$(peer_public_key_len "$1" "$2")"
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

log "checking trust gate before pairing"
assert_eq "$(cli node-a peers list)" "no peers" "peers before pairing"
assert_not_contains "$(dbus_call node-a ListTrustedPeers)" "'id':" "trust before pairing"
cli_expect_fail node-a send "$B_ID" "untrusted"
assert_contains "$LAST_OUTPUT" "unknown peer" "untrusted send error"

log "checking QR pinning and expiry"
start_pairing
WRONG_FP_QR="$(printf '%s' "$PAIR_QR" | sed 's/fp=[0-9a-f]*/fp=0000000000000000000000000000000000000000000000000000000000000000/')"
cli_expect_fail node-b pair with-qr "$WRONG_FP_QR"
assert_eq "$(cli node-a peers list)" "no peers" "wrong fingerprint created trust"
cli node-a pair cancel >/dev/null
start_pairing
EXPIRED_QR="$(printf '%s' "$PAIR_QR" | sed 's/exp=[0-9]*/exp=1/')"
cli_expect_fail node-b pair with-qr "$EXPIRED_QR"
assert_contains "$LAST_OUTPUT" "expired" "expired QR error"
assert_eq "$(cli node-a peers list)" "no peers" "expired QR created trust"
cli node-a pair cancel >/dev/null

log "checking pairing, key binding, and permissions"
start_pairing
LEAK_PIN="$PAIR_PIN"
LEAK_TOKEN="$(printf '%s' "$PAIR_QR" | sed -n 's/.*[?&]token=\([^&]*\).*/\1/p')"
cli node-b pair with-pin "$A_ID" "$PAIR_PIN" >/dev/null
wait_trusted
wait_online
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
assert_log_absent node-a "$LEAK_PIN" "pairing PIN leaked"
assert_log_absent node-b "$LEAK_PIN" "pairing PIN leaked"
assert_log_absent node-a "$LEAK_TOKEN" "QR token leaked"
assert_log_absent node-b "$LEAK_TOKEN" "QR token leaked"

log "checking identity, trust, and history persistence"
restart_nodes
wait_for 90 session_online node-a || fail "node-a did not reconnect after restart"
wait_for 90 session_online node-b || fail "node-b did not reconnect after restart"
assert_eq "$(node_id node-a)" "$A_ID" "node-a id after restart"
assert_eq "$(node_id node-b)" "$B_ID" "node-b id after restart"
assert_eq "$(fingerprint_raw node-a)" "$A_FP" "node-a fingerprint after restart"
assert_eq "$(fingerprint_raw node-b)" "$B_FP" "node-b fingerprint after restart"
peer_known node-a "$B_ID" || fail "node-a trust did not survive restart"
peer_known node-b "$A_ID" || fail "node-b trust did not survive restart"
TOKEN_RESTART="$(unique_token restart)"
MSG_RESTART="$(cli node-a send "$B_ID" "$TOKEN_RESTART")"
wait_for 60 message_body_is node-b "$A_ID" "$MSG_RESTART" "$TOKEN_RESTART" \
    || fail "messaging failed after restart"
assert_log_absent node-a "$TOKEN_SECRET" "message body leaked after restart"
assert_log_absent node-b "$TOKEN_SECRET" "message body leaked after restart"

log "security scenario passed"
