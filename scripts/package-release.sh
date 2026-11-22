#!/usr/bin/env bash
# Package a release built on the current platform.
set -euo pipefail

build_dir="${1:?release build directory required}"
out="${2:?output directory required}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
stage="$work/OxiGram"
mkdir -p "$stage"
cp LICENSE NOTICE.md "$stage/"
tdlib_version="$(tr -d '[:space:]' < tdlib/VERSION)"
version="$(awk -F'"' '/^version = "/ {print $2; exit}' Cargo.toml)"
arch="$(uname -m)"
if [[ "$arch" == aarch64 ]]; then arch=arm64; fi

cat > "$stage/START.txt" <<'TEXT'
Get a Telegram API ID and hash at https://my.telegram.org/.
Set RGRAM_API_ID and RGRAM_API_HASH in the environment before launching.
Linux and Windows: run oxigram or oxigram.exe from the extracted directory.
macOS: run OxiGram.app/Contents/MacOS/OxiGram from Terminal.
Linux builds target Ubuntu 24.04. See the repository README for system libraries.

Source and dependency build recipes:
https://github.com/egkodin/OxiGram
TEXT

case "$(uname -s)" in
    Darwin)
        app="$stage/OxiGram.app"
        mkdir -p "$app/Contents/MacOS"
        cp "$build_dir/oxigram" "$app/Contents/MacOS/OxiGram"
        dylib="libtdjson.$tdlib_version.dylib"
        cp "$build_dir/$dylib" "$app/Contents/MacOS/"
        cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>OxiGram</string>
<key>CFBundleIdentifier</key><string>org.oxigram.client</string>
<key>CFBundleExecutable</key><string>OxiGram</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSMicrophoneUsageDescription</key><string>OxiGram uses the microphone to record voice messages.</string>
</dict></plist>
PLIST
        plutil -lint "$app/Contents/Info.plist"
        codesign --force --sign - "$app/Contents/MacOS/$dylib"
        codesign --force --sign - "$app"
        codesign --verify --deep --strict "$app"
        "$app/Contents/MacOS/OxiGram" --version
        ditto -c -k --keepParent "$stage" "$out/OxiGram-$version-macos-$arch.zip"
        ;;
    Linux)
        cp "$build_dir/oxigram" "$build_dir/libtdjson.so.$tdlib_version" "$stage/"
        "$stage/oxigram" --version
        tar -czf "$out/OxiGram-$version-linux-$arch.tar.gz" -C "$work" OxiGram
        ;;
    MINGW*|MSYS*|CYGWIN*)
        cp "$build_dir/oxigram.exe" "$build_dir/"*.dll "$stage/"
        triplet="${VCPKG_ROOT:?VCPKG_ROOT must point to vcpkg}/installed/x64-windows"
        cp "$triplet/bin/"*.dll "$stage/"
        mkdir -p "$stage/licenses"
        for copyright in "$triplet/share/"*/copyright; do
            library="$(basename "$(dirname "$copyright")")"
            cp "$copyright" "$stage/licenses/$library.txt"
        done
        PATH="/c/Windows/System32:/c/Windows" "$stage/oxigram.exe" --version
        STAGE="$(cygpath -w "$stage")" ARCHIVE="$(cygpath -w "$out/OxiGram-$version-windows-x86_64.zip")" \
            pwsh -NoProfile -Command 'Compress-Archive -LiteralPath $env:STAGE -DestinationPath $env:ARCHIVE'
        ;;
    *)
        echo "Unsupported packaging platform: $(uname -s)" >&2
        exit 1
        ;;
esac
