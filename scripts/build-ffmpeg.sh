#!/usr/bin/env bash
# Builds the minimal static FFmpeg rgram links in release builds: only the
# decoders, demuxers and parsers it plays, with the platform's hardware
# decoders and dav1d for software AV1. Sources and hashes: ffmpeg/SOURCES.
#
# Usage: scripts/build-ffmpeg.sh [prefix]
#   prefix            install dir (default: $RGRAM_FFMPEG_DIR or ~/.cache/rgram/ffmpeg-<version>)
#   FFMPEG_BUILD_JOBS parallel jobs (default: all cores)
#   FFMPEG_DOWNLOADS  optional dir that already holds the tarballs
#
# Needs: a C compiler, make, pkg-config, meson and ninja (dav1d), nasm on x86.
# Then build rgram with PKG_CONFIG_PATH=<prefix>/lib/pkgconfig and
# `--features media/static-ffmpeg`; leave FFMPEG_DIR unset, or ffmpeg-sys-next
# skips pkg-config and does not link dav1d.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
sources="$repo_root/ffmpeg/SOURCES"
field() { awk -v name="$1" -v column="$2" '$1 == name { print $column }' "$sources"; }
ffmpeg_version="$(field ffmpeg 2)"
prefix="${1:-${RGRAM_FFMPEG_DIR:-$HOME/.cache/rgram/ffmpeg-$ffmpeg_version}}"
stamp="$(shasum -a 256 "$sources" "${BASH_SOURCE[0]}" | shasum -a 256 | cut -c1-16)"
jobs="${FFMPEG_BUILD_JOBS:-$(getconf _NPROCESSORS_ONLN)}"

if [[ -f "$prefix/.rgram-ffmpeg" ]] && [[ "$(cat "$prefix/.rgram-ffmpeg")" == "$stamp" ]]; then
  echo "FFmpeg $ffmpeg_version already built in $prefix"
  exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Downloads (or finds) a tarball, checks its hash and unpacks it.
fetch() {
  local name="$1" url hash file
  url="$(field "$name" 3)"
  hash="$(field "$name" 4)"
  file="${FFMPEG_DOWNLOADS:-$work}/$(basename "$url")"
  [[ -f "$file" ]] || curl -sSfL "$url" -o "$file"
  echo "$hash  $file" | shasum -a 256 -c - > /dev/null
  mkdir -p "$work/$name"
  tar -xf "$file" -C "$work/$name" --strip-components 1
}

fetch dav1d
fetch ffmpeg

case "$(uname -s)" in
  Darwin)
    export MACOSX_DEPLOYMENT_TARGET=11.0
    hwaccel=(--enable-videotoolbox
      --enable-hwaccel=h264_videotoolbox,hevc_videotoolbox,vp9_videotoolbox,av1_videotoolbox)
    ;;
  Linux)
    hwaccel=(--enable-vaapi
      --enable-hwaccel=h264_vaapi,hevc_vaapi,vp9_vaapi,av1_vaapi)
    ;;
  MINGW* | MSYS* | CYGWIN*)
    # shortcut: written from tdesktop's build_ffmpeg_win.sh, not run yet; needs an MSYS2 shell inside the MSVC developer environment. Check on the first Windows CI run.
    hwaccel=(--toolchain=msvc --enable-d3d11va
      --enable-hwaccel=h264_d3d11va,hevc_d3d11va,vp9_d3d11va,av1_d3d11va,h264_d3d11va2,hevc_d3d11va2,vp9_d3d11va2,av1_d3d11va2)
    ;;
  *)
    echo "unsupported system $(uname -s)" >&2
    exit 1
    ;;
esac

meson setup "$work/dav1d-build" "$work/dav1d" \
  --prefix="$prefix" --libdir=lib --buildtype=release --default-library=static \
  -Denable_tools=false -Denable_tests=false -Denable_examples=false
ninja -C "$work/dav1d-build" install

# Everything off, then exactly what rgram plays: Telegram's videos, round
# videos, GIFs (H.264 MP4) and WebM stickers; voice (Opus) and music.
configure=(
  --prefix="$prefix"
  --enable-static --disable-shared --enable-pic
  --disable-programs --disable-doc --disable-network --disable-autodetect
  --disable-everything --disable-avdevice --disable-avfilter
  --enable-swscale --enable-swresample
  --enable-protocol=file
  --enable-libdav1d
  --enable-decoder=h264,hevc,vp9,av1,libdav1d,aac,opus,mp3,mp3float,flac,vorbis
  --enable-demuxer=mov,matroska,ogg,mp3,flac,aac
  --enable-parser=h264,hevc,vp9,av1,aac,opus,mpegaudio,flac,vorbis
  "${hwaccel[@]}"
)
(
  cd "$work/ffmpeg"
  PKG_CONFIG_PATH="$prefix/lib/pkgconfig" ./configure "${configure[@]}"
  make -j "$jobs"
  make install
)

echo "$stamp" > "$prefix/.rgram-ffmpeg"
echo "FFmpeg $ffmpeg_version installed in $prefix"
du -sh "$prefix/lib"/*.a
