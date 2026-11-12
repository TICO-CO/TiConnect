#!/bin/sh
set -eu

config_dir="${TICONNECT_CONFIG_DIR:-/data/config}"
data_dir="${TICONNECT_DATA_DIR:-/data/data}"
bus_path="${TICONNECT_TEST_BUS_PATH:-/tmp/ticonnect-bus}"

mkdir -p "$config_dir" "$data_dir"
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
