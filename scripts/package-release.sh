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
bundle_version="${version%%[-+]*}"
arch="$(uname -m)"
if [[ "$arch" == aarch64 ]]; then arch=arm64; fi

smoke_test() {
    env -u OXIGRAM_API_ID -u OXIGRAM_API_HASH -u DYLD_LIBRARY_PATH -u LD_LIBRARY_PATH \
        "$1" --check-release-configuration
}

write_update_marker() {
    local directory="$1" path="$2"
    (
        cd "$directory"
        printf '%s\n%s\n' 'OxiGram package 1' "$version" > "$path"
        if [[ "$path" == OxiGram.package ]]; then
            LC_ALL=C find . -mindepth 1 -print | LC_ALL=C sort >> "$path"
        fi
    )
}

case "$(uname -s)" in
    Darwin)
        app="$work/OxiGram.app"
        mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
        cp "$build_dir/oxigram" "$app/Contents/MacOS/OxiGram"
        cp resources/OxiGram.icns "$app/Contents/Resources/OxiGram.icns"
        mv "$stage/LICENSE" "$stage/NOTICE.md" "$app/Contents/Resources/"
        dylib="libtdjson.$tdlib_version.dylib"
        cp "$build_dir/$dylib" "$app/Contents/MacOS/"
        cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>OxiGram</string>
<key>CFBundleIdentifier</key><string>org.oxigram.client</string>
<key>CFBundleIconFile</key><string>OxiGram.icns</string>
<key>CFBundleExecutable</key><string>OxiGram</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$bundle_version</string>
<key>CFBundleVersion</key><string>$bundle_version</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSMicrophoneUsageDescription</key><string>OxiGram uses the microphone to record voice messages.</string>
</dict></plist>
PLIST
        write_update_marker "$app" Contents/Resources/OxiGram.package
        plutil -lint "$app/Contents/Info.plist"
        codesign --force --sign - "$app/Contents/MacOS/$dylib"
        codesign --force --sign - "$app"
        codesign --verify --deep --strict "$app"
        smoke_test "$app/Contents/MacOS/OxiGram"
        ditto -c -k --norsrc --keepParent "$app" "$out/OxiGram-$version-macos-$arch.zip"
        ;;
    Linux)
        cp "$build_dir/oxigram" "$build_dir/libtdjson.so.$tdlib_version" "$stage/"
        write_update_marker "$stage" OxiGram.package
        smoke_test "$stage/oxigram"
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
        write_update_marker "$stage" OxiGram.package
        PATH="/usr/bin:/c/Windows/System32:/c/Windows" smoke_test "$stage/oxigram.exe"
        STAGE="$(cygpath -w "$stage")" ARCHIVE="$(cygpath -w "$out/OxiGram-$version-windows-x86_64.zip")" \
            pwsh -NoProfile -Command 'Compress-Archive -LiteralPath $env:STAGE -DestinationPath $env:ARCHIVE'
        ;;
    *)
        echo "Unsupported packaging platform: $(uname -s)" >&2
        exit 1
        ;;
esac
