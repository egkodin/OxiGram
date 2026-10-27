# ADR 0006: Voice capture with cpal and libopus

- Status: accepted
- Date: 2026-10-27

## Context

Voice messages must reach other clients as OGG Opus, mono (TDLib's
`inputVoiceNote`). tdesktop records with OpenAL and encodes with FFmpeg's
libopus at 48 kHz, 32 kbit/s. rgram had no audio stack yet.

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
