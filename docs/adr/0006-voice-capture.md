# ADR 0006: Voice capture with cpal and libopus; media decoding with FFmpeg

- Status: accepted
- Date: 2026-10-27

## Context

Voice messages must reach other clients as OGG Opus, mono (TDLib's
`inputVoiceNote`). tdesktop records with OpenAL and encodes with FFmpeg's
libopus at 48 kHz, 32 kbit/s. oxigram had no audio stack yet.

Options:

1. FFmpeg (through `ffmpeg-next`). It would also cover M5's video and GIFs,
   but it is a large system dependency with GPL/LGPL build choices of its own,
   and M5 may not need it if TDLib's thumbnails and gpui's decoders suffice.
2. A platform API per OS (AVFoundation, WASAPI, PipeWire). Three backends to
   write; none gives OGG Opus without another encoder.
3. cpal for capture on every desktop OS, libopus (the `opus` crate builds it
   from source with cmake) for encoding, and the `ogg` crate for the pages.

## Decision

Option 3, in a separate `media` crate so the UI crates do not link audio
libraries directly. The crate follows tdesktop's capture: 400 ms dropped, a
300 ms fade in and out, recordings under a second discarded, a peak per
10 ms reduced to the 100-value 5-bit waveform.

All three crates are GPL-compatible (Apache-2.0, MIT/Apache-2.0,
BSD-3-Clause; libopus BSD-3-Clause, see NOTICE.md). The decision is revisited
when M5 picks a media decoder: if that is FFmpeg, voice encoding can move to
it and these crates go.

## Consequences

- Building needs cmake (libopus); CI images must have it.
- macOS asks for microphone access on first use; a bundled app needs
  `NSMicrophoneUsageDescription` in its Info.plist (packaging, M14).
- `media::Source::Samples` lets tests and snapshots record without a
  microphone.

## Revisited in M5: media decoding with FFmpeg

- Status: accepted (the owner's choice, 2026-10-28)

Video, round videos, GIFs, WebM stickers and music need a decoder. The owner
picked FFmpeg through `ffmpeg-next` 9.x over a pure-Rust stack (H.264 only)
and per-OS media APIs. Voice capture stays on cpal and libopus for now; voice
playback and music may move to FFmpeg's Opus and MP3 decoders.

Conditions:

- Release builds do not depend on a system FFmpeg. `scripts/build-ffmpeg.sh`
  builds a minimal static FFmpeg from the tarballs pinned with their SHA-256
  in `ffmpeg/SOURCES` (FFmpeg 9.0.2, dav1d 1.5.4, libvpx 1.17.0), and the app links it
  (`FFMPEG_DIR=<prefix>`, feature `static-ffmpeg`). Development builds may
  link the Homebrew FFmpeg of the same major version.
- Hardware decoding where available (VideoToolbox, D3D11VA, VAAPI), with
  FFmpeg's software decoders as the fallback.
- Decoding runs off the main thread; only the frames on screen stay in
  memory, within the budget in `docs/perf.md`.

Configure flags (common):

```
--enable-static --disable-shared --enable-pic
--disable-programs --disable-doc --disable-network --disable-autodetect
--disable-everything --disable-avdevice --disable-avfilter
--enable-swscale --enable-swresample
--enable-protocol=file
--enable-libdav1d --enable-libvpx
--enable-decoder=h264,hevc,vp9,libvpx_vp9,av1,libdav1d,aac,opus,mp3,mp3float,flac,vorbis
--enable-demuxer=mov,matroska,ogg,mp3,flac,aac
--enable-parser=h264,hevc,vp9,av1,aac,opus,mpegaudio,flac,vorbis
```

Per platform:

- macOS: `--enable-videotoolbox --enable-hwaccel=h264_videotoolbox,hevc_videotoolbox,vp9_videotoolbox,av1_videotoolbox`
- Linux: `--enable-vaapi --enable-hwaccel=h264_vaapi,hevc_vaapi,vp9_vaapi,av1_vaapi`
- Windows: `--toolchain=msvc --enable-d3d11va` with the `*_d3d11va` and
  `*_d3d11va2` hwaccels for the same four codecs (not built yet).

dav1d is built with meson as a static library (`-Denable_tools=false
-Denable_tests=false`): FFmpeg's own `av1` decoder only drives hardware, so
software AV1 needs it. The `mp3`, `flac` and `aac` demuxers are added to the
three containers for music files; `vp9_superframe_split` comes in as the VP9
decoder's dependency.

libvpx is built for its VP9 decoder alone, as tdesktop builds it
(`--enable-static --disable-shared --enable-pic --disable-examples
--disable-tools --disable-docs --disable-unit-tests --disable-vp8
--enable-vp9 --disable-vp9-encoder --disable-webm-io
--size-limit=4096x4096`, with `--target=arm64-darwin20-gcc` or
`x86_64-darwin20-gcc` on macOS, since its configure does not know newer
Darwin versions, and `x86_64-win64-vs17` on Windows). Only VP9 streams that
WebM marks with `alpha_mode=1` open on `libvpx-vp9`; the rest keep the
hardware decoder and FFmpeg's `vp9` (tdesktop's `FindDecoder` forces libvpx
for every VP9 stream).

Size, macOS arm64 (Apple M4), FFmpeg 9.0.2 + dav1d 1.5.4, 2026-10-28:

- static archives: 34 MB (libavcodec 20 MB, libswscale 5.1 MB, libavformat
  4.1 MB, libavutil 3.2 MB, libdav1d 1.0 MB, libswresample 0.4 MB), debug
  info included;
- a probe that opens a file, finds and opens every stream's decoder with a
  VideoToolbox device, and creates a scaler and a resampler, linked with
  `-dead_strip` and stripped: 6.2 MB, depending only on system frameworks
  (VideoToolbox, CoreMedia, CoreVideo, CoreFoundation, CoreServices). It
  decodes an H.264 MP4.

Size with libvpx, macOS arm64, 2026-11-03: static archives 34 MB
(libvpx 0.5 MB, the others unchanged); the stripped probe was not
measured again.

Known gaps:

- Decided with WebM stickers (2026-11-03): FFmpeg's `vp9` decoder ignores
  the alpha plane WebM stickers carry in Matroska `BlockAdditional`, so
  libvpx (BSD-3-Clause) joins the build, as tdesktop links it. Checked: on
  a VP9 WebM with alpha, `vp9` gives opaque pixels where `libvpx-vp9`
  keeps them clear (test `a_webm_keeps_its_alpha`, passing on the static
  build with no FFmpeg or libvpx dylib in the binary).
- The FFmpeg tarball's hash is checked against Homebrew's formula, not its
  GPG signature (no gpg on this machine).
- The build is LGPL (no `--enable-gpl`), compatible with oxigram's GPLv3.

Linking, checked 2026-10-28: `media/static-ffmpeg` turns on ffmpeg-next's
`static`; with `PKG_CONFIG_PATH=<prefix>/lib/pkgconfig` and `FFMPEG_DIR`
unset, ffmpeg-sys-next links the archives through `pkg-config --static`,
which brings dav1d, libvpx and the system frameworks in order (with `FFMPEG_DIR` it
skips pkg-config and dav1d is missing). The `media` tests pass on that
build, and the test binary depends on no FFmpeg or Homebrew library. Its
fixed macOS framework list names QTKit, which the linker warns about and
ignores. CI builds the archives into `~/.cache/oxigram-ffmpeg`, cached by
`ffmpeg/SOURCES` and the script. Development builds without the feature
use the system FFmpeg (Homebrew 9.0.2 here); ffmpeg-next 9 needs FFmpeg 9.

Decoding (`media/src/video.rs`): a thread per video reads packets, decodes
on the platform's device when one opens (FFmpeg falls back to software for
streams the device cannot take; VideoToolbox decodes a 720p H.264 here, a
64x36 one goes to software), copies hardware frames down, scales them with
swscale to BGRA no larger than the size they show at, and hands them over
through a two-frame bounded channel, so a video holds the frame on screen
and two ahead. Seeks go to the keyframe before and skip to the target.

Sound (`media/src/audio.rs`, 2026-11-03): music and a video's track decode
through FFmpeg on a thread of their own and swresample to the default
output's rate and channels as packed f32, through cpal like the voice
player. Voice messages still decode whole with libopus.
