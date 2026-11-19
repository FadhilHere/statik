#!/usr/bin/env bash
# Build the release executable and copy it to dist/Statik.exe.
#
#   bash tools/release.sh
#
# Optional settings (environment variables, or put them in the git-ignored
# tools/release.local.sh, which is sourced when present):
#   RC              full path to rc.exe (resource compiler, for the embedded icon)
#   STATIK_VS_DIR   a Visual Studio / Windows SDK folder to search for rc.exe
#   CARGO_HOME, TMP, TEMP, CARGO_BUILD_JOBS   usual cargo/temp settings (e.g. when C: is full
#                   or memory is short)
# Without rc.exe the build still succeeds; the exe then has no embedded icon (the window icon
# is set at runtime anyway).
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -f tools/release.local.sh ]; then
    # shellcheck disable=SC1091
    . tools/release.local.sh
fi

if [ -z "${RC:-}" ] && [ -n "${STATIK_VS_DIR:-}" ]; then
    RC="$(find "$STATIK_VS_DIR" -iname rc.exe 2>/dev/null | head -n 1 || true)"
    [ -n "$RC" ] && RC="$(cygpath -w "$RC" 2>/dev/null || echo "$RC")"
fi
if [ -n "${RC:-}" ]; then
    export RC
    echo "release: using resource compiler $RC"
else
    echo "release: no rc.exe found; building without an embedded icon"
fi

cargo build --release -p statik-app

mkdir -p dist
cp target/release/statik-app.exe dist/Statik.exe
size=$(stat -c %s dist/Statik.exe)
echo "release: dist/Statik.exe ($size bytes, $((size / 1024 / 1024)) MB)"
