#!/bin/bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
[[ "$(uname -s)" == Darwin ]] || { echo "macOS is required" >&2; exit 1; }
ffmpeg_version="$(awk '$1 == "ffmpeg" { print $2 }' ffmpeg/SOURCES)"
ffmpeg_dir="${RGRAM_FFMPEG_DIR:-$HOME/.cache/rgram/ffmpeg-$ffmpeg_version}"
if [[ ! -f "$ffmpeg_dir/lib/libvpx.a" ]] || ! grep -Eq '(^|[[:space:]])-lvpx([[:space:]]|$)' "$ffmpeg_dir/lib/pkgconfig/libavcodec.pc"; then
    echo "Rebuild static FFmpeg with scripts/build-ffmpeg.sh for VP9 alpha support" >&2
    exit 1
fi
if ! ar -t "$ffmpeg_dir/lib/libavcodec.a" | grep -Fx gifdec.o >/dev/null ||
   ! ar -t "$ffmpeg_dir/lib/libavcodec.a" | grep -Fx gif_parser.o >/dev/null ||
   ! ar -t "$ffmpeg_dir/lib/libavformat.a" | grep -Fx gifdec.o >/dev/null; then
    echo "Rebuild static FFmpeg with scripts/build-ffmpeg.sh for GIF playback" >&2
    exit 1
fi
unset FFMPEG_DIR
export PKG_CONFIG_PATH="$ffmpeg_dir/lib/pkgconfig"
"$root/scripts/check-metal.sh"
cargo build --release -p oxigram -p media --features media/static-ffmpeg

app="$root/target/OxiGram.app"
[[ ! -e "$app" ]] || { echo "$app already exists; move it before rebuilding" >&2; exit 1; }
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/oxigram "$app/Contents/MacOS/oxigram-bin"
# The icon is the owner's local file (git-ignored), used when present.
icon="$root/resources/OxiGram.icns"
[[ ! -f "$icon" ]] || cp "$icon" "$app/Contents/Resources/OxiGram.icns"
cp "target/release/libtdjson.$(cat tdlib/VERSION).dylib" "$app/Contents/MacOS/"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>OxiGram</string>
<key>CFBundleDisplayName</key><string>OxiGram</string>
<key>CFBundleIdentifier</key><string>org.oxigram.client</string>
<key>CFBundleExecutable</key><string>OxiGram</string>
<key>CFBundleIconFile</key><string>OxiGram</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSMicrophoneUsageDescription</key><string>OxiGram uses the microphone to record voice messages.</string>
</dict></plist>
PLIST
launcher="$app/Contents/MacOS/OxiGram"
printf '#!/bin/bash\nproject_env=%q\n' "$root/.env" > "$launcher"
cat >> "$launcher" <<'LAUNCHER'
if [[ -f "$project_env" ]]; then
    set -a
    source "$project_env"
    set +a
fi
if [[ -z "${RGRAM_API_ID:-}" ]]; then
    RGRAM_API_ID="$(security find-generic-password -a rgram -s rgram.api_id -w 2>/dev/null || true)"
fi
if [[ -z "${RGRAM_API_HASH:-}" ]]; then
    RGRAM_API_HASH="$(security find-generic-password -a rgram -s rgram.api_hash -w 2>/dev/null || true)"
fi
export RGRAM_API_ID RGRAM_API_HASH
exec "$(dirname "$0")/oxigram-bin" "$@"
LAUNCHER
chmod +x "$launcher"
codesign --force --sign - "$app/Contents/MacOS/libtdjson.$(cat tdlib/VERSION).dylib"
codesign --force --sign - "$app/Contents/MacOS/oxigram-bin"
codesign --force --sign - "$app"
echo "$app"
