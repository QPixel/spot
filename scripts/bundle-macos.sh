#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

BUILD_DIR="$ROOT/BUILD-bundle"
APP_NAME="Riff"
APP="$ROOT/target/${APP_NAME}.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"
RES="$CONTENTS/Resources"
BREW_PREFIX="$(brew --prefix)"

VERSION="$(grep '^version' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')"
ICON_SVG="data/hicolor/scalable/apps/dev.diegovsky.Riff.svg"

die() {
    echo "error: $*" >&2
    exit 1
}

require() {
    local cmd="$1"
    local pkg="${2:-$1}"
    command -v "$cmd" >/dev/null 2>&1 || die "missing '$cmd' (install with: brew install $pkg)"
}

echo "==> Checking prerequisites"
require meson meson
require ninja ninja
require cargo rust
require dylibbundler dylibbundler
require rsvg-convert librsvg
require iconutil
require glib-compile-schemas glib
require gdk-pixbuf-query-loaders gdk-pixbuf
require blueprint-compiler blueprint-compiler
require msgfmt gettext

[[ -f "$ICON_SVG" ]] || die "icon not found at $ICON_SVG"

echo "==> Building release binary with meson"
if [[ ! -f "$BUILD_DIR/build.ninja" ]]; then
    meson setup "$BUILD_DIR" -Dbuildtype=release -Doffline=false --prefix="$HOME/.local"
else
    meson setup --reconfigure "$BUILD_DIR" -Dbuildtype=release -Doffline=false --prefix="$HOME/.local"
fi
meson compile -C "$BUILD_DIR"

BINARY="$BUILD_DIR/src/riff"
GRESOURCE="$BUILD_DIR/src/riff.gresource"
[[ -f "$BINARY" ]] || die "binary not found at $BINARY (meson build failed?)"
[[ -f "$GRESOURCE" ]] || die "gresource not found at $GRESOURCE (meson build failed?)"

echo "==> Assembling $APP"
rm -rf "$APP"
mkdir -p "$MACOS" "$RES/share/riff" "$RES/share/locale" "$RES/share/glib-2.0/schemas"
mkdir -p "$RES/share/icons" "$RES/lib"

cp "$BINARY" "$MACOS/riff"
chmod +x "$MACOS/riff"
cp "$GRESOURCE" "$RES/share/riff/riff.gresource"

echo "==> Generating app icon"
ICONSET="$(mktemp -d)/Riff.iconset"
mkdir -p "$ICONSET"
trap 'rm -rf "$(dirname "$ICONSET")"' EXIT
for size in 16 32 128 256 512; do
    rsvg-convert -w "$size" -h "$size" "$ICON_SVG" -o "$ICONSET/icon_${size}x${size}.png"
    double=$((size * 2))
    rsvg-convert -w "$double" -h "$double" "$ICON_SVG" -o "$ICONSET/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$ICONSET" -o "$RES/Riff.icns"

echo "==> Installing GSettings schemas"
cp "$BREW_PREFIX"/share/glib-2.0/schemas/*.xml "$RES/share/glib-2.0/schemas/" 2>/dev/null || true
cp data/dev.diegovsky.Riff.gschema.xml "$RES/share/glib-2.0/schemas/"
glib-compile-schemas "$RES/share/glib-2.0/schemas"

echo "==> Installing icon themes"
for theme in Adwaita hicolor; do
    if [[ -d "$BREW_PREFIX/share/icons/$theme" ]]; then
        cp -RL "$BREW_PREFIX/share/icons/$theme" "$RES/share/icons/"
    fi
done
mkdir -p "$RES/share/icons/hicolor/scalable/apps" "$RES/share/icons/hicolor/symbolic/apps"
cp data/hicolor/scalable/apps/dev.diegovsky.Riff.svg "$RES/share/icons/hicolor/scalable/apps/"
cp data/hicolor/symbolic/apps/dev.diegovsky.Riff-symbolic.svg "$RES/share/icons/hicolor/symbolic/apps/" 2>/dev/null || true
if command -v gtk4-update-icon-cache >/dev/null 2>&1; then
    gtk4-update-icon-cache -f -t "$RES/share/icons/hicolor" || true
    gtk4-update-icon-cache -f -t "$RES/share/icons/Adwaita" || true
fi

echo "==> Compiling translations"
shopt -s nullglob
for po in po/*.po; do
    lang="$(basename "$po" .po)"
    [[ "$lang" == "spot" ]] && continue
    dest="$RES/share/locale/$lang/LC_MESSAGES"
    mkdir -p "$dest"
    msgfmt "$po" -o "$dest/riff.mo"
done

echo "==> Bundling shared libraries"
bundle_libs() {
    local target="$1"
    dylibbundler -od -b -x "$target" \
        -d "$RES/lib" \
        -p @executable_path/../Resources/lib
}

bundle_libs "$MACOS/riff"

echo "==> Installing gdk-pixbuf loaders"
LOADERS_SRC="$BREW_PREFIX/lib/gdk-pixbuf-2.0/2.10.0/loaders"
LOADERS_DST="$RES/lib/gdk-pixbuf-2.0/2.10.0/loaders"
[[ -d "$LOADERS_SRC" ]] || die "gdk-pixbuf loaders not found at $LOADERS_SRC"
mkdir -p "$LOADERS_DST"
cp "$LOADERS_SRC"/*.so "$LOADERS_DST/"

# The SVG loader links against librsvg via @rpath; bundle it before fixing loaders.
RSVG="$BREW_PREFIX/lib/librsvg-2.2.dylib"
if [[ -f "$RSVG" ]]; then
    cp "$RSVG" "$RES/lib/"
    dylibbundler -of -b -x "$RES/lib/librsvg-2.2.dylib" \
        -d "$RES/lib" \
        -p @executable_path/../Resources/lib \
        -s "$BREW_PREFIX/lib"
fi

shopt -s nullglob
for loader in "$LOADERS_DST"/*.so; do
    # Dependencies are already bundled from the main binary; only fix loader paths.
    dylibbundler -x "$loader" \
        -d "$RES/lib" \
        -p @executable_path/../Resources/lib \
        -s "$BREW_PREFIX/lib"
done

(
    cd "$LOADERS_DST"
    gdk-pixbuf-query-loaders > ../loaders.cache
)
# gdk-pixbuf-query-loaders writes absolute source paths; keep basenames only so
# modules resolve relative to the cache directory inside the bundle.
sed -i '' -E 's|"[^"]*/(libpixbufloader[^"]+)"|"\1"|g' \
    "$RES/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache"

echo "==> Writing Info.plist"
sed "s/@VERSION@/$VERSION/g" macos/Info.plist > "$CONTENTS/Info.plist"

echo "==> Code signing"
codesign --force --deep --sign - "$APP"

echo
echo "Done: $APP"
echo "Open with: open \"$APP\""
