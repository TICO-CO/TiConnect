#!/usr/bin/env bash
set -euo pipefail

LAN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCENARIO_DIR="$LAN_DIR/scenarios"
BUILD_PROJECT="${LAN_TEST_BUILD_PROJECT:-ticonnect-lan-build-$$}"
IMAGE="${LAN_TEST_IMAGE:-ticonnect-lan-test:${BUILD_PROJECT}}"
COMPOSE=(docker compose -f "$LAN_DIR/compose.yaml" -p "$BUILD_PROJECT")

export LAN_TEST_IMAGE="$IMAGE"

log() {
    printf '%s\n' "$*"
}

cleanup() {
    local status=$?
    trap - EXIT

    if [ "${LAN_TEST_KEEP:-0}" != "1" ]; then
        docker image rm -f "$IMAGE" >/dev/null 2>&1 || true
    fi

    exit "$status"
}
trap cleanup EXIT

if [ "$#" -gt 0 ]; then
    LAN_TEST_SCENARIO="$(printf '%s,' "$@")"
    LAN_TEST_SCENARIO="${LAN_TEST_SCENARIO%,}"
fi

if [ "${LAN_TEST_SKIP_BUILD:-0}" != "1" ]; then
    log "building LAN test image $IMAGE"
    "${COMPOSE[@]}" build
fi

[ -d "$SCENARIO_DIR" ] || {
    log "no scenario directory at $SCENARIO_DIR"
    exit 1
}

ran=0
for script in "$SCENARIO_DIR"/*.sh; do
    [ -e "$script" ] || continue
    name="$(basename "$script" .sh)"

    if [ -n "${LAN_TEST_SCENARIO:-}" ]; then
        case ",${LAN_TEST_SCENARIO}," in
            *",$name,"*) ;;
            *) continue ;;
        esac
    fi

    ran=1
    log "=== $name ==="
    bash "$script"
done

[ "$ran" -eq 1 ] || {
    log "no LAN e2e scenarios matched '${LAN_TEST_SCENARIO:-}'"
    exit 1
}

log "all LAN e2e scenarios passed"
