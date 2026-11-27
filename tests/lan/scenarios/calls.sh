#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib.sh"

begin_scenario calls

export LAN_TEST_DISCOVERY_ENABLED=
export LAN_TEST_DISCOVERY_MDNS=
export LAN_TEST_DISCOVERY_BROADCAST=

start_nodes
wait_for 30 candidate_visible node-a "$B_ID" || fail "node-a did not discover node-b"
wait_for 30 candidate_visible node-b "$A_ID" || fail "node-b did not discover node-a"
pair_with_pin

call_state() {
    local node="$1"
    cli "$node" call status 2>/dev/null | awk '/^state:/ { print $2; exit }'
}

call_metrics() {
    local node="$1"
    cli "$node" call metrics 2>/dev/null
}

metric_value() {
    call_metrics "$1" | awk -v key="$2:" '$1 == key { print $2; exit }'
}

metrics_flowing() {
    local node="$1"
    local sent received
    sent="$(metric_value "$node" frames_sent)"
    received="$(metric_value "$node" frames_received)"
    [ "${sent:-0}" -gt 0 ] && [ "${received:-0}" -gt 0 ]
}

run_call() {
    local caller="$1"
    local callee="$2"
    local callee_id="$3"
    local label="$4"
    local call state

    log "checking $label call"
    call="$(cli "$caller" call start "$callee_id")"
    [ -n "$call" ] || fail "$label call start failed"
    wait_for 30 call_ringing "$callee" "$call" \
        || fail "$label callee did not ring"
    cli "$callee" call accept "$call"
    wait_for 30 call_connected "$caller" \
        || fail "$label caller did not enter in-call"
    wait_for 30 call_connected "$callee" \
        || fail "$label callee did not enter in-call"
    sleep 2
    state="$(call_state "$caller")"
    [ "$state" = "in-call" ] || fail "$label caller left in-call: $state"
    state="$(call_state "$callee")"
    [ "$state" = "in-call" ] || fail "$label callee left in-call: $state"
    wait_for 30 metrics_flowing "$caller" \
        || fail "$label caller audio did not flow"
    wait_for 30 metrics_flowing "$callee" \
        || fail "$label callee audio did not flow"
    log "$label audio caller sent=$(metric_value "$caller" frames_sent) received=$(metric_value "$caller" frames_received) callee sent=$(metric_value "$callee" frames_sent) received=$(metric_value "$callee" frames_received)"

    cli "$caller" call hangup "$call"
    wait_for 30 call_idle "$caller" || fail "$label caller did not end"
    wait_for 30 call_idle "$callee" || fail "$label callee did not end"
}

call_ringing() {
    cli "$1" call status 2>/dev/null | grep -q "call_id: $2"
}

call_connected() {
    [ "$(call_state "$1")" = "in-call" ]
}

call_idle() {
    [ -z "$(call_state "$1")" ]
}

run_call node-a node-b "$B_ID" "a-to-b"
run_call node-b node-a "$A_ID" "b-to-a"

log "calls scenario passed"
