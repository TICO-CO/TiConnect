#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "usage: $0 <binary-tar.gz> <tag> [output-dir]" >&2
    echo "must not run as root; makepkg refuses root" >&2
    exit 2
}

[ "$#" -ge 2 ] || usage

BIN_TAR="$(realpath "$1")"
TAG="$2"
OUT_DIR="${3:-dist}"
PKGVER="${TAG#v}"
PKGREL=1
ARCH=x86_64

[ "$(id -u)" -ne 0 ] || {
    echo "do not run as root; makepkg refuses root" >&2
    exit 1
}
[ -f "$BIN_TAR" ] || {
    echo "binary tarball not found: $BIN_TAR" >&2
    exit 1
}

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$OUT_DIR"
OUT_DIR="$(cd "$OUT_DIR" && pwd)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/ticonnect-arch.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

BIN_NAME="ticonnect-${PKGVER}-linux-${ARCH}.tar.gz"
SUPPORT_NAME="ticonnect-support-${PKGVER}.tar.gz"
SUPPORT_DIR="$WORK/support"

tar -tzf "$BIN_TAR" | grep -qx 'ticonnect' || {
    echo "$BIN_TAR does not contain ./ticonnect" >&2
    exit 1
}
cp "$BIN_TAR" "$WORK/$BIN_NAME"

mkdir -p \
    "$SUPPORT_DIR/usr/share/applications" \
    "$SUPPORT_DIR/usr/share/icons/hicolor/scalable/apps" \
    "$SUPPORT_DIR/usr/share/metainfo" \
    "$SUPPORT_DIR/usr/lib/systemd/user" \
    "$SUPPORT_DIR/etc/xdg/autostart"

install -m644 "$ROOT/packaging/flatpak/org.ticonnect.TiConnect.desktop" \
    "$SUPPORT_DIR/usr/share/applications/org.ticonnect.TiConnect.desktop"
install -m644 "$ROOT/packaging/flatpak/org.ticonnect.TiConnect.svg" \
    "$SUPPORT_DIR/usr/share/icons/hicolor/scalable/apps/org.ticonnect.TiConnect.svg"
install -m644 "$ROOT/packaging/flatpak/org.ticonnect.TiConnect.metainfo.xml" \
    "$SUPPORT_DIR/usr/share/metainfo/org.ticonnect.TiConnect.metainfo.xml"

for unit in ticonnectd ticonnect-overlay ticonnect-ui; do
    sed 's|@BINDIR@|/usr/bin|g' "$ROOT/packaging/systemd/$unit.service" \
        > "$SUPPORT_DIR/usr/lib/systemd/user/$unit.service"
done

for entry in ticonnect ticonnect-overlay; do
    sed 's|@BINDIR@|/usr/bin|g' "$ROOT/packaging/autostart/$entry.desktop" \
        > "$SUPPORT_DIR/etc/xdg/autostart/$entry.desktop"
done

tar -czf "$WORK/$SUPPORT_NAME" -C "$WORK" support

BIN_SHA="$(sha256sum "$WORK/$BIN_NAME" | awk '{print $1}')"
SUPPORT_SHA="$(sha256sum "$WORK/$SUPPORT_NAME" | awk '{print $1}')"

cat > "$WORK/PKGBUILD" <<EOF
pkgname=ticonnect
pkgver=${PKGVER}
pkgrel=${PKGREL}
pkgdesc="LAN-only messaging, calls, and remote desktop"
arch=('${ARCH}')
url="https://github.com/TICO-CO/TiConnect"
license=('GPL-3.0-or-later')
depends=('gtk4' 'webkit2gtk-6.0' 'gtk4-layer-shell' 'alsa-lib' 'glib2')
options=('!debug')
source=('${BIN_NAME}' '${SUPPORT_NAME}')
sha256sums=('${BIN_SHA}' '${SUPPORT_SHA}')

package() {
    install -Dm755 "\$srcdir/ticonnect" "\$pkgdir/usr/bin/ticonnect"
    cp -a "\$srcdir/support/." "\$pkgdir/"
}
EOF

(
    cd "$WORK"
    PKGDEST="$WORK/out" makepkg --nodeps --noconfirm --force
)

PKG="$(find "$WORK/out" -maxdepth 1 -name "ticonnect-${PKGVER}-${PKGREL}-${ARCH}.pkg.tar.*" -print -quit)"
[ -n "$PKG" ] || {
    echo "package output not found" >&2
    exit 1
}
cp "$PKG" "$OUT_DIR/"
printf '%s\n' "$OUT_DIR/$(basename "$PKG")"
