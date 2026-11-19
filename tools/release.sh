#!/usr/bin/env bash
# Build the release executable and copy it to dist/Statik.exe.
#
#   bash tools/release.sh
#
# Machine-local settings for this laptop: cargo home and temp on D:, and the resource compiler
# (rc.exe, for the embedded icon) from the Visual Studio folder. Without rc.exe the build still
# succeeds; the exe then has no embedded icon (the window icon is set at runtime anyway).
# CARGO_BUILD_JOBS can be lowered (e.g. 2) when memory is short.
set -euo pipefail
cd "$(dirname "$0")/.."

export CARGO_HOME='D:\tmp\cargo-home'
export TMP='D:\tmp\tmp'
export TEMP='D:\tmp\tmp'

if [ -z "${RC:-}" ]; then
    for rc in "/d/Kuliah/Magang/VisualStudio/SDK/ScopeCppSDK/vc15/SDK/bin/rc.exe"; do
        if [ -f "$rc" ]; then
            RC="$(cygpath -w "$rc" 2>/dev/null || echo "$rc")"
            break
        fi
    done
    if [ -z "${RC:-}" ]; then
        RC="$(find /d/Kuliah/Magang/VisualStudio -iname rc.exe 2>/dev/null | head -n 1 || true)"
        [ -n "$RC" ] && RC="$(cygpath -w "$RC" 2>/dev/null || echo "$RC")"
    fi
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
