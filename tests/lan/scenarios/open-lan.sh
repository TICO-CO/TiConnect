#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib.sh"

begin_scenario open-lan

peer_gone() {
    ! peer_known "$1" "$2"
}

export LAN_TEST_TRUST_OPEN_LAN_A=true
export LAN_TEST_TRUST_OPEN_LAN_B=true
export LAN_TEST_DISCOVERY_ENABLED=
export LAN_TEST_DISCOVERY_MDNS=
export LAN_TEST_DISCOVERY_BROADCAST=

log "checking automatic trust with Open LAN mode"
start_nodes
wait_trusted
wait_online
assert_contains "$(dbus_call node-a ListPeers)" "'id': <'$B_ID'>" "peer id"
assert_contains "$(dbus_call node-a ListPeers)" "'connected': <true>" "peer connected"

log "checking message delivery without pairing"
TOKEN="open-lan-$(date +%s)"
cli node-a send "$B_ID" "$TOKEN" >/dev/null
wait_for 30 history_has_token node-b "$A_ID" "$TOKEN" || fail "message did not arrive"

log "checking block and unblock"
B_FP="$(fingerprint_raw node-b)"
forget_peer node-a "$B_ID"
wait_for 30 peer_gone node-a "$B_ID" || fail "blocked peer stayed trusted"
sleep 3
if peer_known node-a "$B_ID"; then fail "blocked peer was re-trusted"; fi
cli node-a pair unblock "$B_FP" >/dev/null
wait_trusted
wait_online

log "checking one disabled side does not auto-trust"
unset LAN_TEST_TRUST_OPEN_LAN_A
unset LAN_TEST_TRUST_OPEN_LAN_B
export LAN_TEST_TRUST_OPEN_LAN_A=false
export LAN_TEST_TRUST_OPEN_LAN_B=true
reset_nodes
sleep 5
if peer_known node-a "$B_ID"; then fail "disabled node auto-trusted a peer"; fi
if peer_known node-b "$A_ID"; then fail "enabled node trusted a disabled peer"; fi

log "open LAN scenario passed"
