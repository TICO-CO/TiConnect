#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib.sh"

display_name_is() {
    [ "$(display_name node-a)" = "$1" ]
}

begin_scenario control
start_nodes

log "checking control surface"
assert_eq "$(cli node-a ping)" "pong" "ping"
VERSION="$(cli node-a version)"
[ -n "$VERSION" ] || fail "daemon returned an empty version"

STATUS="$(cli node-a status)"
assert_contains "$STATUS" "version: $VERSION" "human status version"
assert_contains "$STATUS" "peer_count: 0" "human status peer count"
assert_contains "$STATUS" "outbox_depth: 0" "human status outbox"
assert_contains "$STATUS" "transport_state: idle" "human status transport"
assert_contains "$STATUS" "active_sessions: 0" "human status sessions"

assert_eq "$(status_json_value node-a version)" "$VERSION" "json version"
assert_eq "$(status_json_value node-a peer_count)" "0" "json peer count"
assert_eq "$(status_json_value node-a outbox_depth)" "0" "json outbox"
assert_eq "$(status_json_value node-a transport_state)" "idle" "json transport"
assert_eq "$(status_json_value node-a active_sessions)" "0" "json sessions"

IDENTITY="$(cli node-a id show)"
assert_contains "$IDENTITY" "node_id: $A_ID" "identity node id"
assert_eq "$(display_name node-a)" "node-a" "identity display name"
A_FP="$(fingerprint_raw node-a)"
assert_eq "${#A_FP}" "64" "fingerprint length"
assert_matches "$A_FP" '^[0-9a-f]{64}$' "fingerprint format"
assert_matches "$(printf '%s\n' "$IDENTITY" | sed -n 's/^fingerprint: //p')" \
    '^[0-9a-f]{4}( [0-9a-f]{4}){15}$' "display fingerprint grouping"

log "checking settings file operations"
assert_eq "$(cli node-a settings get dnd.enabled)" "false" "default dnd"
cli node-a settings set dnd.enabled true >/dev/null
assert_eq "$(cli node-a settings get dnd.enabled)" "true" "saved dnd"
assert_contains "$(cli node-a settings list)" "dnd.enabled = true" "settings list"
cli_expect_fail node-a settings set outbox.ttl_days 0
assert_contains "$LAST_OUTPUT" "invalid setting" "invalid setting error"
cli_expect_fail node-a settings set missing.key value
cli node-a settings set dnd.enabled false >/dev/null

log "checking daemon settings reload"
cli node-a settings set identity.display_name control-renamed >/dev/null
wait_for 20 setting_string_is node-a identity.display_name control-renamed \
    || fail "daemon did not reload identity.display_name"
wait_for 20 display_name_is control-renamed \
    || fail "id show did not expose reloaded display name"
assert_contains "$(dbus_call node-a ListSettings)" "'appearance.color_scheme'" \
    "ListSettings keys"
cli_expect_fail node-a settings get missing.key

log "checking filesystem permissions and schema"
assert_mode node-a /data/config 700
assert_mode node-a /data/data 700
assert_mode node-a /data/config/identity.key 600
assert_mode node-a /data/config/identity.toml 600
assert_mode node-a /data/config/config.toml 600
assert_mode node-a /data/data/ticonnect.db 600
assert_eq "$(sqlite_query node-a 'SELECT MAX(version) FROM schema_version;')" "4" \
    "schema version"

log "checking graceful shutdown and restart"
monitor_start node-a control-signals
cli node-a quit >/dev/null
wait_for 30 daemon_down node-a || fail "daemon did not stop after quit"
wait_for 15 volume_contains node-a /data/control-signals.signals DaemonStopping \
    || fail "DaemonStopping signal not observed"

start_node node-a
assert_eq "$(node_id node-a)" "$A_ID" "node id after restart"
assert_eq "$(fingerprint_raw node-a)" "$A_FP" "fingerprint after restart"
assert_eq "$(display_name node-a)" "control-renamed" "display name after restart"
assert_eq "$(cli node-a settings get identity.display_name)" "control-renamed" \
    "persisted display name"
assert_eq "$(cli node-a ping)" "pong" "ping after restart"

log "control scenario passed"
