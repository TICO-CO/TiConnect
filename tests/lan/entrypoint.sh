#!/bin/sh
set -eu

config_dir="${TICONNECT_CONFIG_DIR:-/data/config}"
data_dir="${TICONNECT_DATA_DIR:-/data/data}"
bus_path="${TICONNECT_TEST_BUS_PATH:-/tmp/ticonnect-bus}"

mkdir -p "$config_dir" "$data_dir"

set_setting() {
    key="$1"
    value="$2"
    if [ -z "$value" ]; then
        return 0
    fi
    TICONNECT_LOG_LEVEL=off /usr/local/bin/ticonnect-cli settings set "$key" "$value" >/dev/null
}

if [ ! -f "$config_dir/config.toml" ]; then
    set_setting identity.display_name "${LAN_TEST_DISPLAY_NAME:-}"
    set_setting discovery.enabled "${LAN_TEST_DISCOVERY_ENABLED:-}"
    set_setting discovery.mdns "${LAN_TEST_DISCOVERY_MDNS:-}"
    set_setting discovery.broadcast "${LAN_TEST_DISCOVERY_BROADCAST:-}"
    set_setting discovery.beacon_interval_secs "${LAN_TEST_BEACON_INTERVAL_SECS:-}"
    set_setting transport.port "${LAN_TEST_TRANSPORT_PORT:-}"
    set_setting read_receipts.enabled "${LAN_TEST_READ_RECEIPTS_ENABLED:-}"
    set_setting dnd.enabled "${LAN_TEST_DND_ENABLED:-}"
    set_setting outbox.ttl_days "${LAN_TEST_OUTBOX_TTL_DAYS:-}"
fi

rm -f "$bus_path"

dbus-daemon --session --fork --nopidfile --nosyslog \
    --address="unix:path=$bus_path"

attempt=0
while [ ! -S "$bus_path" ]; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 50 ]; then
        echo "D-Bus session bus did not start at $bus_path" >&2
        exit 1
    fi
    sleep 0.1
done

export DBUS_SESSION_BUS_ADDRESS="unix:path=$bus_path"
unset DISPLAY WAYLAND_DISPLAY

exec /usr/local/bin/ticonnectd
