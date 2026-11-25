#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib.sh"

begin_scenario open-lan

user_gone() {
    ! user_known "$1" "$2"
}

export LAN_TEST_DISCOVERY_ENABLED=
export LAN_TEST_DISCOVERY_MDNS=
export LAN_TEST_DISCOVERY_BROADCAST=

log "checking automatic LAN trust"
start_nodes
wait_trusted
wait_online
assert_contains "$(dbus_call node-a ListUsers)" "'id': <'$B_ID'>" "user id"
assert_contains "$(dbus_call node-a ListUsers)" "'connected': <true>" "user connected"

log "checking message delivery after automatic trust"
TOKEN="open-lan-$(date +%s)"
cli node-a send "$B_ID" "$TOKEN" >/dev/null
wait_for 30 history_has_token node-b "$A_ID" "$TOKEN" || fail "message did not arrive"

log "checking block and unblock"
B_FP="$(fingerprint_raw node-b)"
forget_user node-a "$B_ID"
wait_for 30 user_gone node-a "$B_ID" || fail "blocked user stayed trusted"
sleep 3
if user_known node-a "$B_ID"; then fail "blocked user was re-trusted"; fi
cli node-a users unblock "$B_FP" >/dev/null
wait_trusted
wait_online

log "automatic trust scenario passed"
