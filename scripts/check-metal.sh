#!/bin/sh
# Fails early, with the fix, when Xcode's Metal Toolchain is missing: gpui
# compiles its shaders at build time and otherwise fails deep in its build
# script. (`--features oxigram/runtime-shaders` builds without it, at about
# 200 ms on a cold start; docs/perf.md.)
[ "$(uname -s)" = Darwin ] || exit 0
if ! xcrun -sdk macosx metal --version >/dev/null 2>&1; then
    echo "The Metal Toolchain is missing. Install it with:" >&2
    echo "    xcodebuild -downloadComponent MetalToolchain" >&2
    exit 1
fi
