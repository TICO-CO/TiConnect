#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib.sh"

dropped_above() {
    local now
    now="$(dbus_map_number node-a GetDiscoveryStatus dropped_packets)"
    [ -n "$now" ] && [ "$now" -gt "$1" ]
}

begin_scenario discovery

export LAN_TEST_DISCOVERY_ENABLED=
export LAN_TEST_DISCOVERY_MDNS=
export LAN_TEST_DISCOVERY_BROADCAST=

start_nodes

log "checking default discovery backends"
wait_for 30 candidate_visible node-a "$B_ID" || fail "node-a did not discover node-b"
wait_for 30 candidate_visible node-b "$A_ID" || fail "node-b did not discover node-a"

A_SOURCE="$(candidate_source node-a "$B_ID")"
B_SOURCE="$(candidate_source node-b "$A_ID")"
case "$A_SOURCE" in
    mdns|broadcast) ;;
    *) fail "node-a reported unknown discovery source '$A_SOURCE'" ;;
esac
case "$B_SOURCE" in
    mdns|broadcast) ;;
    *) fail "node-b reported unknown discovery source '$B_SOURCE'" ;;
esac
log "default discovery sources: node-a=$A_SOURCE node-b=$B_SOURCE"

B_FP="$(fingerprint_raw node-b)"
B_IP="$(node_ip node-b)"
CANDIDATES="$(dbus_call node-a ListCandidates)"
assert_contains "$CANDIDATES" "'node_id': <'$B_ID'>" "candidate node id"
assert_contains "$CANDIDATES" "'fingerprint': <'$B_FP'>" "candidate fingerprint"
assert_contains "$CANDIDATES" "'display_name': <'node-b'>" "candidate display name"
assert_contains "$CANDIDATES" "'addresses': <'$B_IP'" "candidate address"
assert_contains "$CANDIDATES" "'protocol_version': <uint32 1>" "candidate protocol"
assert_contains "$CANDIDATES" "'last_seen': <uint64" "candidate last seen"
assert_contains "$CANDIDATES" "'source': <'$A_SOURCE'>" "candidate source"
assert_not_contains "$CANDIDATES" "'node_id': <'$A_ID'>" "self candidate filtered"
assert_eq "$(candidate_port node-a "$B_ID")" "45871" "default candidate port"

assert_socket node-a 5353
assert_socket node-a 45870
assert_socket node-a 45871
assert_socket node-b 5353
assert_socket node-b 45870
assert_socket node-b 45871

DISCOVERY_STATUS="$(dbus_call node-a GetDiscoveryStatus)"
assert_contains "$DISCOVERY_STATUS" "'enabled': <true>" "discovery enabled"
assert_contains "$DISCOVERY_STATUS" "'mdns_enabled': <true>" "mdns enabled"
assert_contains "$DISCOVERY_STATUS" "'broadcast_enabled': <true>" "broadcast enabled"
assert_contains "$DISCOVERY_STATUS" "'mdns_active': <true>" "mdns active"
assert_contains "$DISCOVERY_STATUS" "'broadcast_active': <true>" "broadcast active"

log "checking candidate watch and loss signals"
cli_watch_start node-a discovery-watch discover watch
sleep 1
monitor_start node-a discovery-signals
stop_node node-b
wait_for 20 candidate_gone node-a "$B_ID" || fail "node-a kept a stale candidate"
watch_expect node-a discovery-watch "lost $B_FP"
monitor_expect node-a discovery-signals "CandidateLost"
start_node node-b
wait_for 60 candidate_visible node-a "$B_ID" || fail "node-a did not rediscover node-b"
watch_expect node-a discovery-watch "found $B_ID"
monitor_expect node-a discovery-signals "CandidateFound"

log "checking display-name propagation"
cli node-a settings set identity.display_name discovery-a >/dev/null
restart_node node-a
wait_for 60 candidate_visible node-b "$A_ID" \
    || fail "node-b did not rediscover node-a after display-name restart"
wait_for 30 candidate_has_field node-b "'display_name': <'discovery-a'>" \
    || fail "node-b did not receive the updated display name"

log "checking malformed packet accounting"
BASELINE_DROPS="$(dbus_map_number node-a GetDiscoveryStatus dropped_packets)"
[ -n "$BASELINE_DROPS" ] || fail "could not read dropped_packets"
A_IP="$(node_ip node-a)"
udp_send_garbage node-b "$A_IP"
wait_for 10 dropped_above "$BASELINE_DROPS" \
    || fail "malformed discovery packet was not counted"

log "checking live discovery reconfigure"
cli node-a settings set discovery.broadcast false >/dev/null
wait_for 30 source_is node-a "$B_ID" mdns \
    || fail "node-a did not switch to mDNS-only discovery"
wait_for 30 candidate_visible node-b "$A_ID" \
    || fail "node-b lost node-a during mDNS-only reconfigure"
cli node-a settings set discovery.mdns false >/dev/null
wait_for 30 candidate_gone node-b "$A_ID" \
    || fail "node-b still saw node-a with discovery backends disabled"
cli node-a settings set discovery.broadcast true >/dev/null
wait_for 60 candidate_visible node-b "$A_ID" \
    || fail "node-b did not see node-a after broadcast was re-enabled"
wait_for 30 source_is node-b "$A_ID" broadcast \
    || fail "node-b did not use broadcast after mDNS was disabled"

log "checking mDNS-only backend"
export LAN_TEST_DISCOVERY_ENABLED=
export LAN_TEST_DISCOVERY_MDNS=true
export LAN_TEST_DISCOVERY_BROADCAST=false
reset_nodes
wait_for 45 candidate_visible node-a "$B_ID" || fail "mDNS-only: node-a did not discover node-b"
wait_for 45 candidate_visible node-b "$A_ID" || fail "mDNS-only: node-b did not discover node-a"
assert_eq "$(candidate_source node-a "$B_ID")" "mdns" "mDNS-only source"
assert_eq "$(candidate_source node-b "$A_ID")" "mdns" "mDNS-only source"
assert_map_bool node-a GetDiscoveryStatus mdns_active true
assert_map_bool node-a GetDiscoveryStatus broadcast_active false
assert_socket node-a 5353

log "checking broadcast-only backend"
export LAN_TEST_DISCOVERY_ENABLED=
export LAN_TEST_DISCOVERY_MDNS=false
export LAN_TEST_DISCOVERY_BROADCAST=true
reset_nodes
wait_for 45 candidate_visible node-a "$B_ID" || fail "broadcast-only: node-a did not discover node-b"
wait_for 45 candidate_visible node-b "$A_ID" || fail "broadcast-only: node-b did not discover node-a"
assert_eq "$(candidate_source node-a "$B_ID")" "broadcast" "broadcast-only source"
assert_eq "$(candidate_source node-b "$A_ID")" "broadcast" "broadcast-only source"
assert_map_bool node-a GetDiscoveryStatus mdns_active false
assert_map_bool node-a GetDiscoveryStatus broadcast_active true
assert_socket node-a 45870

log "checking disabled discovery and live re-enable"
export LAN_TEST_DISCOVERY_ENABLED=false
export LAN_TEST_DISCOVERY_MDNS=
export LAN_TEST_DISCOVERY_BROADCAST=
reset_nodes
sleep 3
candidate_gone node-a "$B_ID" || fail "disabled discovery still produced candidates"
candidate_gone node-b "$A_ID" || fail "disabled discovery still produced candidates"
assert_map_bool node-a GetDiscoveryStatus enabled false
cli node-a settings set discovery.enabled true >/dev/null
cli node-b settings set discovery.enabled true >/dev/null
wait_for 60 candidate_visible node-a "$B_ID" \
    || fail "node-a did not rediscover after enabling discovery"
wait_for 60 candidate_visible node-b "$A_ID" \
    || fail "node-b did not rediscover after enabling discovery"

log "discovery scenario passed"
