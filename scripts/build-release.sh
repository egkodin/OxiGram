#!/usr/bin/env bash
# Builds and verifies a release on the current host. Archives go in target/dist.
set +x
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [[ -z "${OXIGRAM_RELEASE_API_ID:-}" && -z "${OXIGRAM_RELEASE_API_HASH:-}" ]]; then
    if [[ -f .env ]]; then
        source ./.env
    fi
    if [[ -z "${OXIGRAM_RELEASE_API_ID:-}" && -z "${OXIGRAM_RELEASE_API_HASH:-}" ]]; then
        OXIGRAM_RELEASE_API_ID="${RGRAM_API_ID:-}"
        OXIGRAM_RELEASE_API_HASH="${RGRAM_API_HASH:-}"
    fi
fi
if [[ ! "${OXIGRAM_RELEASE_API_ID:-}" =~ ^[1-9][0-9]*$ || ! "${OXIGRAM_RELEASE_API_HASH:-}" =~ ^[[:xdigit:]]{32}$ ]]; then
    printf '%s\n' 'Release credentials are missing or invalid. Check the repository .env or the OXIGRAM_RELEASE_API environment variables.' >&2
    exit 1
fi
export -n OXIGRAM_RELEASE_API_ID OXIGRAM_RELEASE_API_HASH RGRAM_API_ID RGRAM_API_HASH

work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT
export CARGO_TARGET_DIR="$work/cargo"
export CARGO_PROFILE_RELEASE_LTO="${CARGO_PROFILE_RELEASE_LTO:-thin}"
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS="${CARGO_PROFILE_RELEASE_CODEGEN_UNITS:-8}"
export CARGO_INCREMENTAL=0
unset FFMPEG_DIR

case "$(uname -s)" in
    Darwin|Linux)
        bash scripts/build-tdlib.sh
        ffmpeg_dir="${RGRAM_FFMPEG_DIR:-$PWD/target/release-ffmpeg}"
        bash scripts/build-ffmpeg.sh "$ffmpeg_dir"
        export PKG_CONFIG_PATH="$ffmpeg_dir/lib/pkgconfig"
        features=runtime-shaders,media/static-ffmpeg
        ;;
    MINGW*|MSYS*|CYGWIN*)
        export VCPKG_ROOT="$(cygpath -m "${VCPKG_ROOT:-${VCPKG_INSTALLATION_ROOT:?Set VCPKG_ROOT}}")"
        export CARGO_TARGET_DIR="$(cygpath -m "$work/cargo")"
        "$VCPKG_ROOT/vcpkg.exe" install \
            gperf:x64-windows openssl:x64-windows zlib:x64-windows \
            'ffmpeg[core,avcodec,avformat,swresample,swscale,vpx]:x64-windows'
        export PATH="$(cygpath -u "$VCPKG_ROOT")/installed/x64-windows/bin:$PATH"
        bash scripts/build-tdlib.sh
        features=runtime-shaders
        ;;
    *)
        printf 'Unsupported build host: %s\n' "$(uname -s)" >&2
        exit 1
        ;;
esac

OXIGRAM_RELEASE_API_ID="$OXIGRAM_RELEASE_API_ID" OXIGRAM_RELEASE_API_HASH="$OXIGRAM_RELEASE_API_HASH" \
    cargo build --locked --release -p oxigram --features "$features"
bash scripts/package-release.sh "$work/cargo/release" "$work/dist"
mkdir -p target/dist
mv -f -- "$work/dist/"* target/dist/
printf 'Release archive saved in %s/target/dist\n' "$PWD"
