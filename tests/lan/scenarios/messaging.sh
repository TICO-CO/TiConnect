#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib.sh"

begin_scenario messaging

export LAN_TEST_DISCOVERY_ENABLED=
export LAN_TEST_DISCOVERY_MDNS=
export LAN_TEST_DISCOVERY_BROADCAST=

start_nodes
wait_for 30 candidate_visible node-a "$B_ID" || fail "node-a did not discover node-b"
pair_with_pin

monitor_start node-a messaging-signals
monitor_start node-b messaging-signals

log "checking bidirectional text and receipts"
TOKEN_AB="$(unique_token ab)"
MSG_AB="$(cli node-a send "$B_ID" "$TOKEN_AB")"
[ -n "$MSG_AB" ] || fail "send returned an empty message id"
wait_for 30 message_body_is node-b "$A_ID" "$MSG_AB" "$TOKEN_AB" \
    || fail "node-b did not receive the text message"
wait_for 30 message_state_is node-a "$B_ID" "$MSG_AB" delivered \
    || fail "node-a did not observe the delivered receipt"
monitor_expect node-a messaging-signals "MessageStateChanged"
monitor_expect node-b messaging-signals "MessageReceived"
monitor_line_contains node-b messaging-signals "$TOKEN_AB" "'notify': <true>"
cli node-b mark-read "$A_ID" "$MSG_AB" >/dev/null
wait_for 30 message_state_is node-a "$B_ID" "$MSG_AB" read \
    || fail "node-a did not observe the read receipt"
assert_contains "$(dbus_call node-a GetMessageState "$MSG_AB")" "read" \
    "GetMessageState read"

TOKEN_BA="$(unique_token ba)"
MSG_BA="$(cli node-b send "$A_ID" "$TOKEN_BA")"
wait_for 30 message_body_is node-a "$B_ID" "$MSG_BA" "$TOKEN_BA" \
    || fail "node-a did not receive the reverse message"

log "checking history pagination and counts"
BEFORE_COUNT="$(message_count node-b "$A_ID")"
HISTORY_IDS=()
for index in 1 2 3 4 5; do
    token="$(unique_token "hist-$index")"
    id="$(cli node-a send "$B_ID" "$token")"
    HISTORY_IDS+=("$id")
    wait_for 30 message_body_is node-b "$A_ID" "$id" "$token" \
        || fail "history message $index was not received"
done
wait_for 30 message_count_is node-b "$A_ID" "$((BEFORE_COUNT + 5))" \
    || fail "history count did not increase by five"
assert_eq "$(cli node-b messages "$A_ID" --limit 2 | wc -l | tr -d ' ')" "2" \
    "history limit"
assert_eq "$(cli node-b messages "$A_ID" --offset 2 --limit 2 | wc -l | tr -d ' ')" "2" \
    "history offset"
assert_eq "$(cli node-b messages "$A_ID" --limit 0 | wc -l | tr -d ' ')" "1" \
    "history limit clamp"
assert_eq "$(cli node-b messages "$A_ID" --offset 100)" "no messages" \
    "history offset beyond end"
CONVERSATION="$(conversation_for_message node-b "$MSG_AB")"
assert_eq "$(dbus_uint_text node-b CountMessages "$CONVERSATION")" \
    "$(message_count node-b "$A_ID")" "CountMessages"

log "checking message body limits"
cli_expect_fail node-a send "$B_ID" ""
assert_contains "$LAST_OUTPUT" "empty" "empty body error"
BODY_OK="$(head -c 32768 /dev/zero | tr '\0' 'a')"
BEFORE_COUNT="$(message_count node-b "$A_ID")"
MSG_OK="$(cli node-a send "$B_ID" "$BODY_OK")"
[ -n "$MSG_OK" ] || fail "32 KiB message was rejected"
wait_for 30 message_count_is node-b "$A_ID" "$((BEFORE_COUNT + 1))" \
    || fail "32 KiB message was not received"
BODY_TOO_LARGE="$(head -c 32769 /dev/zero | tr '\0' 'b')"
cli_expect_fail node-a send "$B_ID" "$BODY_TOO_LARGE"
assert_contains "$LAST_OUTPUT" "too large" "oversized body error"

log "checking read receipt toggle"
cli node-b settings set read_receipts.enabled false >/dev/null
wait_for 20 setting_bool_is node-b read_receipts.enabled false \
    || fail "node-b did not disable read receipts"
TOKEN_NORECEIPT="$(unique_token noreceipt)"
MSG_NORECEIPT="$(cli node-a send "$B_ID" "$TOKEN_NORECEIPT")"
wait_for 30 message_body_is node-b "$A_ID" "$MSG_NORECEIPT" "$TOKEN_NORECEIPT" \
    || fail "message was not stored with receipts disabled"
sleep 2
assert_eq "$(message_state node-a "$B_ID" "$MSG_NORECEIPT")" "sent" \
    "delivered receipt sent while disabled"
cli node-b mark-read "$A_ID" "$MSG_NORECEIPT" >/dev/null
sleep 2
assert_eq "$(message_state node-a "$B_ID" "$MSG_NORECEIPT")" "sent" \
    "read receipt sent while disabled"
cli node-b settings set read_receipts.enabled true >/dev/null
wait_for 20 setting_bool_is node-b read_receipts.enabled true \
    || fail "node-b did not re-enable read receipts"

TOKEN_RECEIPT="$(unique_token receipt)"
MSG_RECEIPT="$(cli node-a send "$B_ID" "$TOKEN_RECEIPT")"
wait_for 30 message_state_is node-a "$B_ID" "$MSG_RECEIPT" delivered \
    || fail "delivered receipt did not resume"
cli node-b mark-read "$A_ID" "$MSG_RECEIPT" >/dev/null
wait_for 30 message_state_is node-a "$B_ID" "$MSG_RECEIPT" read \
    || fail "read receipt did not resume"

cli node-a settings set read_receipts.enabled false >/dev/null
wait_for 20 setting_bool_is node-a read_receipts.enabled false \
    || fail "node-a did not disable read receipts"
TOKEN_INBOUND="$(unique_token inbound)"
MSG_INBOUND="$(cli node-a send "$B_ID" "$TOKEN_INBOUND")"
wait_for 30 message_state_is node-a "$B_ID" "$MSG_INBOUND" delivered \
    || fail "node-a ignored inbound delivered receipts while disabled"
cli node-b mark-read "$A_ID" "$MSG_INBOUND" >/dev/null
wait_for 30 message_state_is node-a "$B_ID" "$MSG_INBOUND" read \
    || fail "node-a ignored inbound read receipts while disabled"
TOKEN_OUTBOUND="$(unique_token outbound)"
MSG_OUTBOUND="$(cli node-b send "$A_ID" "$TOKEN_OUTBOUND")"
wait_for 30 message_body_is node-a "$B_ID" "$MSG_OUTBOUND" "$TOKEN_OUTBOUND" \
    || fail "node-a did not receive while receipts were disabled"
sleep 2
assert_eq "$(message_state node-b "$A_ID" "$MSG_OUTBOUND")" "sent" \
    "node-a sent a receipt while disabled"
cli node-a settings set read_receipts.enabled true >/dev/null
wait_for 20 setting_bool_is node-a read_receipts.enabled true \
    || fail "node-a did not re-enable read receipts"

log "checking policy and DND"
cli node-b policy set "$A_ID" mute >/dev/null
wait_for 20 peer_policy_is node-b "$A_ID" mute || fail "mute policy did not apply"
monitor_expect node-b messaging-signals "PolicyChanged"
TOKEN_MUTE="$(unique_token mute)"
MSG_MUTE="$(cli node-a send "$B_ID" "$TOKEN_MUTE")"
wait_for 30 message_body_is node-b "$A_ID" "$MSG_MUTE" "$TOKEN_MUTE" \
    || fail "mute policy dropped the message"
sleep 2
assert_eq "$(message_state node-a "$B_ID" "$MSG_MUTE")" "sent" \
    "mute policy sent a delivered receipt"
monitor_line_contains node-b messaging-signals "$TOKEN_MUTE" "'notify': <false>"
cli node-b mark-read "$A_ID" "$MSG_MUTE" >/dev/null
wait_for 30 message_state_is node-a "$B_ID" "$MSG_MUTE" read \
    || fail "mute policy suppressed the read receipt"

cli node-b policy set "$A_ID" ignore >/dev/null
wait_for 20 peer_policy_is node-b "$A_ID" ignore || fail "ignore policy did not apply"
BEFORE_COUNT="$(message_count node-b "$A_ID")"
TOKEN_IGNORE="$(unique_token ignore)"
MSG_IGNORE="$(cli node-a send "$B_ID" "$TOKEN_IGNORE")"
sleep 3
assert_eq "$(message_count node-b "$A_ID")" "$BEFORE_COUNT" \
    "ignore policy stored the message"
history_lacks_token node-b "$A_ID" "$TOKEN_IGNORE" \
    || fail "ignored message appeared in history"
assert_eq "$(message_state node-a "$B_ID" "$MSG_IGNORE")" "sent" \
    "ignore policy sent a receipt"
assert_eq "$(monitor_line node-b messaging-signals "$TOKEN_IGNORE")" "" \
    "ignore policy emitted MessageReceived"
cli node-b mark-read "$A_ID" "$MSG_IGNORE" >/dev/null
sleep 2
assert_eq "$(message_state node-a "$B_ID" "$MSG_IGNORE")" "sent" \
    "ignore policy sent a read receipt"

cli node-b policy set "$A_ID" accept >/dev/null
wait_for 20 peer_policy_is node-b "$A_ID" accept || fail "accept policy did not apply"
TOKEN_ACCEPT="$(unique_token accept)"
MSG_ACCEPT="$(cli node-a send "$B_ID" "$TOKEN_ACCEPT")"
wait_for 30 message_state_is node-a "$B_ID" "$MSG_ACCEPT" delivered \
    || fail "accept policy did not send a delivered receipt"
monitor_line_contains node-b messaging-signals "$TOKEN_ACCEPT" "'notify': <true>"
cli_expect_fail node-b policy set "$A_ID" block
assert_contains "$LAST_OUTPUT" "invalid" "invalid policy error"
assert_eq "$(peer_policy node-b "$A_ID")" "accept" "invalid policy changed state"

cli node-b policy set "$A_ID" mute >/dev/null
wait_for 20 peer_policy_is node-b "$A_ID" mute || fail "mute policy did not persist"
restart_node node-b
wait_for 90 session_online node-b || fail "node-b did not reconnect after policy restart"
assert_eq "$(peer_policy node-b "$A_ID")" "mute" "policy did not survive restart"
cli node-b policy set "$A_ID" accept >/dev/null
wait_for 20 peer_policy_is node-b "$A_ID" accept || fail "accept policy did not restore"
monitor_start node-b messaging-signals

cli node-b policy dnd on >/dev/null
wait_for 20 setting_bool_is node-b dnd.enabled true || fail "DND did not enable"
TOKEN_DND="$(unique_token dnd)"
MSG_DND="$(cli node-a send "$B_ID" "$TOKEN_DND")"
wait_for 30 message_body_is node-b "$A_ID" "$MSG_DND" "$TOKEN_DND" \
    || fail "DND dropped the message"
wait_for 30 message_state_is node-a "$B_ID" "$MSG_DND" delivered \
    || fail "DND suppressed the delivered receipt"
monitor_line_contains node-b messaging-signals "$TOKEN_DND" "'notify': <false>"
cli node-b mark-read "$A_ID" "$MSG_DND" >/dev/null
wait_for 30 message_state_is node-a "$B_ID" "$MSG_DND" read \
    || fail "DND suppressed the read receipt"
restart_node node-b
wait_for 90 session_online node-b || fail "node-b did not reconnect after DND restart"
assert_eq "$(setting_bool_text node-b dnd.enabled)" "true" "DND did not survive restart"
cli node-b policy dnd off >/dev/null
wait_for 20 setting_bool_is node-b dnd.enabled false || fail "DND did not disable"

log "checking multiple and idempotent mark-read"
TOKEN_M1="$(unique_token multi1)"
TOKEN_M2="$(unique_token multi2)"
MSG_M1="$(cli node-a send "$B_ID" "$TOKEN_M1")"
MSG_M2="$(cli node-a send "$B_ID" "$TOKEN_M2")"
wait_for 30 message_body_is node-b "$A_ID" "$MSG_M1" "$TOKEN_M1" \
    || fail "first multi-read message was not received"
wait_for 30 message_body_is node-b "$A_ID" "$MSG_M2" "$TOKEN_M2" \
    || fail "second multi-read message was not received"
cli node-b mark-read "$A_ID" "$MSG_M1" "$MSG_M2" >/dev/null
wait_for 30 message_state_is node-a "$B_ID" "$MSG_M1" read \
    || fail "first multi-read message was not marked read"
wait_for 30 message_state_is node-a "$B_ID" "$MSG_M2" read \
    || fail "second multi-read message was not marked read"
cli node-b mark-read "$A_ID" "$MSG_M1" "$MSG_M2" >/dev/null
cli node-a mark-read "$B_ID" "$MSG_M1" >/dev/null
assert_eq "$(message_state node-a "$B_ID" "$MSG_M1")" "read" \
    "mark-read changed outbound state"

log "messaging scenario passed"
