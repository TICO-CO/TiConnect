#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib.sh"

qr_field() {
    printf '%s' "$1" | sed -n "s/.*[?&]$2=\([^&]*\).*/\1/p"
}

peer_gone() {
    ! peer_known "$1" "$2"
}

begin_scenario pairing

export LAN_TEST_DISCOVERY_ENABLED=
export LAN_TEST_DISCOVERY_MDNS=
export LAN_TEST_DISCOVERY_BROADCAST=

start_nodes
wait_for 30 candidate_visible node-a "$B_ID" || fail "node-a did not discover node-b"
wait_for 30 candidate_visible node-b "$A_ID" || fail "node-b did not discover node-a"

log "checking unpaired state"
assert_eq "$(cli node-a peers list)" "no peers" "peers before pairing"
assert_eq "$(status_value node-a peer_count)" "0" "peer count before pairing"
assert_not_contains "$(dbus_call node-a ListTrustedPeers)" "'id':" "trusted peers before pairing"
cli_expect_fail node-a send "$B_ID" "before pairing"
assert_contains "$LAST_OUTPUT" "unknown peer" "unpaired send error"
assert_eq "$(session_count node-a)" "0" "sessions before pairing"

log "checking PIN and QR payload"
monitor_start node-a pairing-signals
monitor_start node-b pairing-signals
start_pairing
assert_matches "$PAIR_PIN" '^[0-9]{6}$' "PIN format"
assert_contains "$PAIR_QR" "ticonnect://pair/v1?" "QR prefix"
assert_eq "$(qr_field "$PAIR_QR" node_id)" "$A_ID" "QR node id"
assert_eq "$(qr_field "$PAIR_QR" fp)" "$(fingerprint_raw node-a)" "QR fingerprint"
assert_eq "$(qr_field "$PAIR_QR" port)" "45871" "QR port"
assert_matches "$(qr_field "$PAIR_QR" token)" '^[0-9a-f]{64}$' "QR token"
NOW="$(date +%s)"
[ "$PAIR_EXPIRES" -ge $((NOW + 100)) ] || fail "pairing expiry is too soon"
[ "$PAIR_EXPIRES" -le $((NOW + 130)) ] || fail "pairing expiry is too far in the future"
assert_eq "$PAIR_PORT" "45871" "pairing port"
monitor_expect node-a pairing-signals "PairingStarted"

log "checking PIN pairing"
cli node-b pair with-pin "$A_ID" "$PAIR_PIN" >/dev/null
wait_trusted
wait_online
monitor_expect node-a pairing-signals "PairingCompleted"
monitor_expect node-b pairing-signals "PairingCompleted"
assert_eq "$(status_value node-a active_sessions)" "1" "active sessions after pairing"
assert_contains "$(dbus_call node-a ListPeers)" "'id': <'$B_ID'>" "peer id"
assert_contains "$(dbus_call node-a ListPeers)" "'fingerprint': <'$(fingerprint_raw node-b)'>" \
    "peer fingerprint"
assert_contains "$(dbus_call node-a ListPeers)" "'policy': <'accept'>" "peer policy"
assert_contains "$(dbus_call node-a ListPeers)" "'connected': <true>" "peer connected"
assert_contains "$(dbus_call node-a ListPeers)" "'trusted_at': <uint64" "peer trusted_at"

log "checking revocation and re-pair"
monitor_start node-a transport-signals
forget_peer node-a "$B_ID"
wait_for 30 peer_gone node-a "$B_ID" || fail "node-a kept the forgotten peer"
wait_for 30 session_idle node-a || fail "node-a kept a session after forget"
monitor_expect node-a transport-signals "PeerDisconnected"
peer_known node-b "$A_ID" || fail "node-b lost its trust row unexpectedly"
sleep 16
assert_eq "$(session_count node-a)" "0" "forgotten peer reconnected"
pair_with_pin

log "checking QR pairing without discovery"
reset_nodes
monitor_start node-a pairing-signals
monitor_start node-b pairing-signals
cli node-a settings set discovery.enabled false >/dev/null
cli node-b settings set discovery.enabled false >/dev/null
wait_for 30 candidate_gone node-a "$B_ID" || fail "node-a kept candidates after disable"
wait_for 30 candidate_gone node-b "$A_ID" || fail "node-b kept candidates after disable"
start_pairing
cli node-b pair with-qr "$PAIR_QR" >/dev/null
wait_trusted
wait_online
monitor_expect node-a pairing-signals "PairingCompleted"

log "checking expired and wrong-fingerprint QR"
forget_both
start_pairing
EXPIRED_QR="$(printf '%s' "$PAIR_QR" | sed 's/exp=[0-9]*/exp=1/')"
cli_expect_fail node-b pair with-qr "$EXPIRED_QR"
assert_contains "$LAST_OUTPUT" "expired" "expired QR error"
cli node-a pair cancel >/dev/null
start_pairing
WRONG_FP_QR="$(printf '%s' "$PAIR_QR" | sed 's/fp=[0-9a-f]*/fp=0000000000000000000000000000000000000000000000000000000000000000/')"
cli_expect_fail node-b pair with-qr "$WRONG_FP_QR"
assert_eq "$(cli node-a peers list)" "no peers" "wrong QR created trust"
cli node-a pair cancel >/dev/null

log "checking QR replay"
start_pairing
REPLAY_QR="$PAIR_QR"
cli node-b pair with-qr "$REPLAY_QR" >/dev/null
wait_trusted
wait_online
forget_both
cli_expect_fail node-b pair with-qr "$REPLAY_QR"
assert_eq "$(cli node-a peers list)" "no peers" "replayed QR created trust"

log "checking PIN cancel, missing candidate, and attempt limit"
export LAN_TEST_DISCOVERY_ENABLED=
export LAN_TEST_DISCOVERY_MDNS=
export LAN_TEST_DISCOVERY_BROADCAST=
reset_nodes
wait_for 30 candidate_visible node-a "$B_ID" || fail "node-a did not rediscover node-b"
wait_for 30 candidate_visible node-b "$A_ID" || fail "node-b did not rediscover node-a"
monitor_start node-a pairing-signals

start_pairing
cli node-a pair cancel >/dev/null
cli_expect_fail node-b pair with-pin "$A_ID" "$PAIR_PIN"
assert_eq "$(cli node-a peers list)" "no peers" "cancelled pairing created trust"

cli node-b settings set discovery.enabled false >/dev/null
wait_for 30 candidate_gone node-b "$A_ID" || fail "node-b kept candidates after disable"
start_pairing
cli_expect_fail node-b pair with-pin "$A_ID" "$PAIR_PIN"
assert_contains "$LAST_OUTPUT" "no discovery candidate" "missing candidate error"
cli node-a pair cancel >/dev/null
cli node-b settings set discovery.enabled true >/dev/null
wait_for 60 candidate_visible node-b "$A_ID" || fail "node-b did not rediscover node-a"

start_pairing
WRONG_PIN="000000"
if [ "$PAIR_PIN" = "$WRONG_PIN" ]; then
    WRONG_PIN="000001"
fi
for _ in 1 2 3; do
    cli_expect_fail node-b pair with-pin "$A_ID" "$WRONG_PIN"
    sleep 1
done
monitor_expect node-a pairing-signals "PairingFailed"
cli_expect_fail node-b pair with-pin "$A_ID" "$PAIR_PIN"
assert_eq "$(cli node-a peers list)" "no peers" "exhausted pairing created trust"

start_pairing
cli node-b pair with-pin "$A_ID" "$PAIR_PIN" >/dev/null
wait_trusted
wait_online

log "pairing scenario passed"
