#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "usage: $0 <app-id> <manifest> <tag> [output-dir]" >&2
}

[ "$#" -ge 3 ] || {
    usage
    exit 2
}
[ "$#" -le 4 ] || {
    usage
    exit 2
}

APP_ID="$1"
MANIFEST="$2"
TAG="$3"
OUT_DIR="${4:-dist}"

command -v flatpak >/dev/null || {
    echo "flatpak is not installed" >&2
    exit 1
}
command -v flatpak-builder >/dev/null || {
    echo "flatpak-builder is not installed" >&2
    exit 1
}
[ -f "$MANIFEST" ] || {
    echo "manifest not found: $MANIFEST" >&2
    exit 1
}

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$(realpath "$MANIFEST")"
mkdir -p "$OUT_DIR"
OUT_DIR="$(cd "$OUT_DIR" && pwd)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/ticonnect-flatpak.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

BUILD_DIR="$WORK/build"
REPO_DIR="$WORK/repo"
BUNDLE="$OUT_DIR/${APP_ID}-${TAG}.flatpak"
[ -e "$BUNDLE" ] && rm -f "$BUNDLE"

flatpak-builder --force-clean --repo="$REPO_DIR" "$BUILD_DIR" "$MANIFEST"
flatpak build-bundle "$REPO_DIR" "$BUNDLE" "$APP_ID"
printf '%s\n' "$BUNDLE"
