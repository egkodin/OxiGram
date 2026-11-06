# ADR 0007: Lottie with tlottie, on a renderer thread

- Status: accepted
- Date: 2026-11-06

## Context

Animated stickers (TGS), animated and big emoji, custom emoji, reaction
animations and Premium sticker effects are Lottie JSON, gzipped. The owner
asked for the renderer tdesktop uses, built from source and linked
statically as FFmpeg is.

tdesktop used rlottie (Samsung's C++ library, forked by Telegram) until
commit 6190755 "Move rlottie -> tlottie" (2026-09-07). Since then
`lib_lottie` draws with tlottie, a Rust crate, pinned
in `Telegram/build/prepare/prepare.py` at
758c7cb74444f1c3c9923065c40fdb3aad8b7d60.

Options:

1. rlottie from Telegram's fork, built with cmake and linked through FFI. It
   is what tdesktop shipped for years, but no longer what it ships; it needs
   a C++ toolchain step, `unsafe` bindings and a fork to track.
2. tlottie, the crate tdesktop now builds. Pure Rust, MIT, no dependencies,
   treats Lottie JSON as untrusted input, supports the skin-tone (`fitz`)
   modifier Telegram's emoji need.
3. A general Rust Lottie crate (rlottie bindings on crates.io, velato).
   Neither is what tdesktop draws with, so frames would differ.

## Decision

Option 2, the owner's choice when asked (rlottie's premise was out of date).

- tlottie is a cargo git dependency pinned to tdesktop's revision; cargo
  builds it from source and links it statically. `deny.toml` allows that
  one git source. Moving the pin follows tdesktop's `prepare.py`.
- `[profile.dev.package.tlottie]` builds it at `opt-level = 3`: unoptimized
  it renders a frame about ten times slower than its budget.
- `media::lottie` wraps it: gunzip with an 8 MiB cap, a 2048 px side cap,
  BGRA with straight alpha for gpui. tlottie's renderer is not `Send`, so
  one thread named `lottie` owns every open animation and answers open,
  render and close over a channel (tdesktop's `FrameRenderer`).
- `features::lottie` keeps one player per file, size in device pixels,
  skin tone and clock: it renders at display size, moves only what was
  drawn in the last 250 ms, pauses while the window is not visible, closes
  what was not drawn for 3 s, and caches every frame of an animation only
  when the whole of it fits in 4 MiB, 32 MiB for all. Power Usage's switches
  pick loop, once or the first frame.

## Consequences

- No C++ step and no `unsafe` for Lottie; the crate shows up in NOTICE.md.
- A git dependency: builds need network access the first time (or a vendored
  copy), and the pin is moved by hand.
- Big animations (stickers at 512 px) are rendered frame by frame on the
  thread instead of tdesktop's compressed frame cache on disk; see
  docs/perf.md for what that costs.
