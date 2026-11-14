#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "usage: $0 <binary> <tag> [output-dir]" >&2
    echo "environment: APPIMAGETOOL=/path/to/appimagetool [APPIMAGETOOL_EXTRACT_RUN=1]" >&2
    exit 2
}

[ "$#" -ge 2 ] || usage

BIN="$1"
TAG="$2"
OUT_DIR="${3:-dist}"

[ -f "$BIN" ] || {
    echo "binary not found: $BIN" >&2
    exit 1
}
[ -n "${APPIMAGETOOL:-}" ] || {
    echo "APPIMAGETOOL is not set" >&2
    exit 1
}

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$OUT_DIR"
OUT_DIR="$(cd "$OUT_DIR" && pwd)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/ticonnect-appimage.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

APPDIR="$WORK/AppDir"
mkdir -p \
    "$APPDIR/usr/bin" \
    "$APPDIR/usr/lib" \
    "$APPDIR/usr/share/applications" \
    "$APPDIR/usr/share/glib-2.0/schemas" \
    "$APPDIR/usr/share/icons/hicolor/scalable/apps"

declare -A COPIED=()

is_excluded() {
    case "$1" in
        linux-vdso.so.* | ld-linux* | libc.so.* | libm.so.* | libdl.so.* | \
            libpthread.so.* | librt.so.* | libresolv.so.* | libutil.so.* | \
            libanl.so.* | libnss_*.so.* | libfontconfig.so.* | libfreetype.so.* | \
            libGL.so.* | libEGL.so.* | \
            libGLX.so.* | libOpenGL.so.* | libGLdispatch.so.* | \
            libvulkan.so.* | libdrm.so.* | libgbm.so.*)
            return 0
            ;;
    esac
    return 1
}

link_deps() {
    ldd "$1" 2>/dev/null | awk '
        $2 == "=>" && $3 ~ /^\// { print $3 }
        $1 ~ /^\// { print $1 }
    '
}

copy_lib() {
    local src="$1"
    local name
    local dest
    local dep

    [ -e "$src" ] || return 0
    name="$(basename "$src")"
    is_excluded "$name" && return 0
    [ "${COPIED[$name]:-0}" = "1" ] && return 0
    COPIED["$name"]=1

    dest="$APPDIR/usr/lib/$name"
    cp -L --preserve=mode "$src" "$dest"
    while IFS= read -r dep; do
        copy_lib "$dep"
    done < <(link_deps "$src")
}

missing="$(ldd "$BIN" 2>/dev/null | awk '/not found/ { print }')"
if [ -n "$missing" ]; then
    echo "unresolved libraries for $BIN:" >&2
    printf '%s\n' "$missing" >&2
    exit 1
fi

install -m755 "$BIN" "$APPDIR/usr/bin/ticonnect"
while IFS= read -r dep; do
    copy_lib "$dep"
done < <(link_deps "$BIN")

LOADERS_DIR="$(pkg-config --variable=gdk_pixbuf_moduledir gdk-pixbuf-2.0 2>/dev/null || true)"
if [ -z "$LOADERS_DIR" ] || [ ! -d "$LOADERS_DIR" ]; then
    LOADERS_DIR="/usr/lib/x86_64-linux-gnu/gdk-pixbuf-2.0/2.10.0/loaders"
fi
LOADERS_DEST="$APPDIR/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders"
if [ -d "$LOADERS_DIR" ]; then
    mkdir -p "$LOADERS_DEST"
    cp -L "$LOADERS_DIR"/*.so "$LOADERS_DEST/"
    for loader in "$LOADERS_DEST"/*.so; do
        copy_lib "$loader"
    done
    if command -v gdk-pixbuf-query-loaders >/dev/null 2>&1; then
        gdk-pixbuf-query-loaders "$LOADERS_DEST"/*.so \
            | sed "s|$APPDIR|@APPDIR@|g" > "$LOADERS_DEST/loaders.cache.in"
    fi
fi

GIO_DIR="$(pkg-config --variable=giomoduledir gio-2.0 2>/dev/null || true)"
if [ -z "$GIO_DIR" ] || [ ! -d "$GIO_DIR" ]; then
    GIO_DIR="/usr/lib/x86_64-linux-gnu/gio/modules"
fi
if [ -d "$GIO_DIR" ]; then
    mkdir -p "$APPDIR/usr/lib/gio/modules"
    cp -L "$GIO_DIR"/*.so "$APPDIR/usr/lib/gio/modules/"
    for module in "$APPDIR"/usr/lib/gio/modules/*.so; do
        copy_lib "$module"
    done
    if command -v gio-querymodules >/dev/null 2>&1; then
        gio-querymodules "$APPDIR/usr/lib/gio/modules"
    fi
fi

if [ -f /usr/share/glib-2.0/schemas/gschemas.compiled ]; then
    cp /usr/share/glib-2.0/schemas/gschemas.compiled \
        "$APPDIR/usr/share/glib-2.0/schemas/gschemas.compiled"
fi

for theme in Adwaita hicolor; do
    if [ -d "/usr/share/icons/$theme" ]; then
        cp -a "/usr/share/icons/$theme" "$APPDIR/usr/share/icons/"
    fi
done

if [ -d /usr/share/alsa ]; then
    cp -a /usr/share/alsa "$APPDIR/usr/share/"
fi

if [ -d /usr/lib/x86_64-linux-gnu/alsa-lib ]; then
    cp -a /usr/lib/x86_64-linux-gnu/alsa-lib "$APPDIR/usr/lib/"
fi

install -m644 "$ROOT/packaging/flatpak/org.ticonnect.TiConnect.svg" \
    "$APPDIR/org.ticonnect.TiConnect.svg"
install -m644 "$ROOT/packaging/flatpak/org.ticonnect.TiConnect.svg" \
    "$APPDIR/usr/share/icons/hicolor/scalable/apps/org.ticonnect.TiConnect.svg"

if command -v rsvg-convert >/dev/null 2>&1; then
    mkdir -p "$APPDIR/usr/share/icons/hicolor/256x256/apps"
    rsvg-convert -w 256 -h 256 \
        "$ROOT/packaging/flatpak/org.ticonnect.TiConnect.svg" \
        -o "$APPDIR/org.ticonnect.TiConnect.png"
    install -m644 "$APPDIR/org.ticonnect.TiConnect.png" \
        "$APPDIR/usr/share/icons/hicolor/256x256/apps/org.ticonnect.TiConnect.png"
    install -m644 "$APPDIR/org.ticonnect.TiConnect.png" "$APPDIR/.DirIcon"
else
    install -m644 "$ROOT/packaging/flatpak/org.ticonnect.TiConnect.svg" "$APPDIR/.DirIcon"
fi

cat > "$APPDIR/org.ticonnect.TiConnect.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=TiConnect
Comment=LAN-only messaging, calls, and remote desktop
Exec=ticonnect
Icon=org.ticonnect.TiConnect
Terminal=false
Categories=Network;Chat;InstantMessaging;
StartupNotify=true
EOF
install -m644 "$APPDIR/org.ticonnect.TiConnect.desktop" \
    "$APPDIR/usr/share/applications/org.ticonnect.TiConnect.desktop"

cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
set -eu

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

export LD_LIBRARY_PATH="$HERE/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export XDG_DATA_DIRS="$HERE/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
export GSETTINGS_SCHEMA_DIR="$HERE/usr/share/glib-2.0/schemas"

if [ -d "$HERE/usr/lib/gio/modules" ]; then
    export GIO_MODULE_DIR="$HERE/usr/lib/gio/modules"
fi

if [ -f "$HERE/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders/loaders.cache.in" ]; then
    cache="${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/ticonnect-gdk-pixbuf-loaders.cache"
    sed "s|@APPDIR@|$HERE|g" \
        "$HERE/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders/loaders.cache.in" > "$cache"
    export GDK_PIXBUF_MODULE_FILE="$cache"
fi

if [ -f "$HERE/usr/share/alsa/alsa.conf" ]; then
    export ALSA_CONFIG_PATH="$HERE/usr/share/alsa/alsa.conf"
fi

exec "$HERE/usr/bin/ticonnect" "$@"
EOF
chmod 755 "$APPDIR/AppRun"

APPIMAGE="$OUT_DIR/ticonnect-${TAG}-x86_64.AppImage"
if [ "${APPIMAGETOOL_EXTRACT_RUN:-0}" = "1" ]; then
    ARCH=x86_64 "$APPIMAGETOOL" --appimage-extract-and-run --no-appstream \
        "$APPDIR" "$APPIMAGE"
else
    ARCH=x86_64 "$APPIMAGETOOL" --no-appstream "$APPDIR" "$APPIMAGE"
fi
chmod 755 "$APPIMAGE"

printf '%s\n' "$APPIMAGE"
