#!/usr/bin/env bash
# Builds the minimal static FFmpeg rgram links in release builds: only the
# decoders, demuxers and parsers it plays, with the platform's hardware
# decoders, dav1d for software AV1 and libvpx for VP9 with alpha (WebM
# stickers). Sources and hashes: ffmpeg/SOURCES.
#
# Usage: scripts/build-ffmpeg.sh [prefix]
#   prefix            install dir (default: $RGRAM_FFMPEG_DIR or ~/.cache/rgram/ffmpeg-<version>)
#   FFMPEG_BUILD_JOBS parallel jobs (default: all cores)
#   FFMPEG_DOWNLOADS  optional dir that already holds the tarballs
#
# Needs: a C compiler, make, pkg-config, meson and ninja (dav1d), nasm on x86
# (dav1d, libvpx and FFmpeg).
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
fetch libvpx
fetch ffmpeg

vpx_target=()
case "$(uname -s)" in
  Darwin)
    export MACOSX_DEPLOYMENT_TARGET=11.0
    hwaccel=(--enable-videotoolbox
      --enable-hwaccel=h264_videotoolbox,hevc_videotoolbox,vp9_videotoolbox,av1_videotoolbox)
    # libvpx does not guess newer Darwin versions; tdesktop names its target too.
    case "$(uname -m)" in
      arm64) vpx_target=(--target=arm64-darwin20-gcc) ;;
      *) vpx_target=(--target=x86_64-darwin20-gcc) ;;
    esac
    ;;
  Linux)
    hwaccel=(--enable-vaapi
      --enable-hwaccel=h264_vaapi,hevc_vaapi,vp9_vaapi,av1_vaapi)
    ;;
  MINGW* | MSYS* | CYGWIN*)
    vpx_target=(--target=x86_64-win64-vs17)
    # shortcut: written from tdesktop's build_ffmpeg_win.sh and build_libvpx_win.sh, not run yet; needs an MSYS2 shell inside the MSVC developer environment. Check on the first Windows CI run.
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

# The VP9 decoder alone, which FFmpeg's libvpx_vp9 wraps for WebM's alpha.
(
  cd "$work/libvpx"
  ./configure --prefix="$prefix" "${vpx_target[@]}" \
    --enable-static --disable-shared --enable-pic \
    --disable-examples --disable-tools --disable-docs --disable-unit-tests \
    --disable-vp8 --enable-vp9 --disable-vp9-encoder --disable-webm-io \
    --size-limit=4096x4096
  make -j "$jobs"
  make install
)

# Everything off, then exactly what rgram plays: Telegram's videos, round
# videos, animations (H.264 MP4 and GIF) and WebM stickers; voice and music.
configure=(
  --prefix="$prefix"
  --enable-static --disable-shared --enable-pic
  --disable-programs --disable-doc --disable-network --disable-autodetect
  --disable-everything --disable-avdevice --disable-avfilter
  --enable-swscale --enable-swresample
  --enable-protocol=file
  --enable-libdav1d --enable-libvpx
  --enable-decoder=h264,hevc,vp9,libvpx_vp9,av1,libdav1d,aac,opus,mp3,mp3float,flac,vorbis,gif
  --enable-demuxer=mov,matroska,ogg,mp3,flac,aac,gif
  --enable-parser=h264,hevc,vp9,av1,aac,opus,mpegaudio,flac,vorbis,gif
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
