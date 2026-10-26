#!/usr/bin/env bash
# Builds TDLib's shared `tdjson` library at the commit pinned in tdlib/COMMIT
# and installs it into a prefix that crates/td-sys links against.
#
# Usage: scripts/build-tdlib.sh [prefix]
#   prefix          install dir (default: $RGRAM_TDLIB_DIR or ~/.cache/rgram/tdlib-<short commit>)
#   TD_SOURCE       optional path to an existing tdlib/td git checkout; it is only read
#                   (`git archive`), never modified. Without it the script clones tdlib/td.
#   TD_BUILD_JOBS   parallel compile jobs (default 6; td_api.cpp needs ~3 GB per job)
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
commit="$(tr -d '[:space:]' < "$repo_root/tdlib/COMMIT")"
short="${commit:0:9}"
prefix="${1:-${RGRAM_TDLIB_DIR:-$HOME/.cache/rgram/tdlib-$short}}"
jobs="${TD_BUILD_JOBS:-6}"

if [[ -f "$prefix/.rgram-tdlib-commit" ]] && [[ "$(cat "$prefix/.rgram-tdlib-commit")" == "$commit" ]]; then
  echo "TDLib $short already installed in $prefix"
  exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
src="$work/td"
mkdir -p "$src"

if [[ -n "${TD_SOURCE:-}" ]]; then
  # TDLib's CMake writes generated sources into its source tree, so build from a copy.
  git -C "$TD_SOURCE" archive "$commit" | tar -x -C "$src"
else
  git clone --quiet --filter=blob:none https://github.com/tdlib/td.git "$work/clone"
  git -C "$work/clone" archive "$commit" | tar -x -C "$src"
fi

cmake_args=(
  -S "$src" -B "$work/build"
  -DCMAKE_BUILD_TYPE=Release
  -DCMAKE_INSTALL_PREFIX="$prefix"
  -DTD_ENABLE_JNI=OFF
)
case "$(uname -s)" in
  Darwin)
    cmake_args+=(
      -G Ninja
      -DOPENSSL_ROOT_DIR="$(brew --prefix openssl@3)"
      # Static OpenSSL: the dylib must not depend on Homebrew at run time.
      -DOPENSSL_USE_STATIC_LIBS=TRUE
      -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0
      -DCMAKE_INSTALL_NAME_DIR=@rpath
    )
    ;;
  MINGW* | MSYS* | CYGWIN*)
    : "${VCPKG_ROOT:?VCPKG_ROOT must point to a vcpkg checkout with openssl, zlib and gperf installed}"
    cmake_args+=(
      -A x64
      -DCMAKE_TOOLCHAIN_FILE="$VCPKG_ROOT/scripts/buildsystems/vcpkg.cmake"
    )
    ;;
  *)
    cmake_args+=(-G Ninja)
    ;;
esac

cmake "${cmake_args[@]}"
cmake --build "$work/build" --config Release --target install --parallel "$jobs"

case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*)
    # tdjson.dll loads OpenSSL and zlib from its own directory; td-sys copies all of them.
    cp "$VCPKG_ROOT"/installed/x64-windows/bin/*.dll "$prefix/bin/"
    ;;
esac

echo "$commit" > "$prefix/.rgram-tdlib-commit"
echo "TDLib $short installed in $prefix"
