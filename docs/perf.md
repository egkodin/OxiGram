# Performance and memory

Budgets come from the project brief. They are starting targets: M1 records a
baseline on a real account and refines them here. A target is loosened only with
a written justification.

## Budgets

Release build, macOS Apple Silicon, account with ~500 chats.

| Metric | Target | Latest | Status |
|---|---|---|---|
| Cold start to interactive chat list (database present) | < 800 ms | — | M1 |
| Idle RSS on the chat list | < 150 MB | — | M1; re-base on footprint (below) |
| RSS after scrolling 10 media-heavy chats and returning | < 250 MB, no growth in a soak test | — | M5 |
| History scroll frame time, p99 | < 8 ms (120 Hz) | 5.44 ms frame build, no dropped refreshes (10,000 synthetic messages, 120 Hz) | ok; real chats not measured |
| Opening a chat to first messages (TDLib cache) | < 100 ms | — | M3 |
| Idle with no animations | ~0 % CPU, zero redraws | 0.0 % CPU, no redraws (empty window) | ok |
| Release binary size without TDLib | < 30 MB | 17.0 MB | ok |

## Measuring memory on macOS

`ps` RSS counts resident pages of shared system frameworks and is not what the
user sees. rgram measures the physical footprint (`vmmap --summary <pid>`, the
figure Activity Monitor shows). The empty M0 window has 101 MB RSS and a 47 MB
footprint. At M1 the 150 MB idle target is restated as footprint, with the
justification recorded here.

## M0: empty window

Mac mini, Apple M4, 24 GB, macOS 27.0.1, external 4K display at 160 Hz with 2×
scaling. `target/release/rgram` (`lto = "fat"`, `codegen-units = 1`,
`panic = "abort"`, `strip = true`, precompiled Metal shaders), started without
API credentials so it shows the startup notice; TDLib is loaded and answers
`getOption("version")`. Three runs; measured 5 s after start for 10 s.

| Metric | Value |
|---|---|
| Release binary, without `libtdjson` | 9.7 MB (9,713,104 bytes) |
| `libtdjson.1.8.69.dylib` (shipped beside it) | 25.3 MB |
| Idle CPU | 1.2 % (1.2–1.3) |
| Physical footprint, idle | 46.6 MB (46.5–46.7) |
| Physical footprint, peak (during startup) | 99.7 MB (99.5–99.7) |
| RSS, idle | 101.4 MB |

The binary is 4.2 MB larger than the gpui spike. The generated TDLib API and its
serde code are the likely cause; that has not been measured yet.

### Idle CPU

A `sample` of the idle process shows the `CVDisplayLink` thread running and
posting to the main queue: gpui appears to keep the display link going for a
visible window even when nothing redraws, so the app wakes on every refresh
(160 Hz here). The gpui spike idled at the same 1.2–1.35 %. Fixed in M1
([ADR 0005](adr/0005-gpui-macos-idle-patch.md)).

## M1: idle display link

The same machine and method as M0, after the gpui_macos patch from ADR 0005.
Three runs.

| Metric | M0 | M1 |
|---|---|---|
| Idle CPU (10 s of `ps` CPU time) | 1.2 % | 0.0 % (below the 0.1 % `ps` resolves, all runs) |
| Idle wakeups (`top -stats idlew`) | not measured | 22–26 per 5 s |
| Redraws while idle | every refresh | none: the link parks 0.2 s after the last frame |
| Frame after an idle period | next refresh | 9–10 ms after the change (`frame-wake` example) |
| Physical footprint, idle | 46.6 MB | 47.1 MB (46.9–47.3) |
| Physical footprint, peak | 99.7 MB | 100.1 MB (100.0–100.3) |
| Release binary | 9,713,104 bytes | 9,713,248 bytes |

The remaining wakeups are not attributed yet. TDLib's own threads and the
receive thread, which waits in `td_receive` with a timeout, are the likely
sources; none of them reaches the window.

## M1: sign-in code, notice screen

The same machine and method, after the sign-in code landed, started without
API credentials (so the window shows the startup notice and TDLib gets no
parameters). Three runs. Cold start to the chat list and idle on a signed-in
account need credentials and are not measured yet.

| Metric | Value |
|---|---|
| Release binary, without `libtdjson` | 17.0 MB (17,011,552 bytes) |
| Idle CPU (10 s of `ps` CPU time) | 0.0–0.1 % (0.00–0.01 s per 10 s) |
| Physical footprint, idle | 48.0 MB (47.9–48.0) |
| Physical footprint, peak | 101.0 MB |
| RSS, idle | 100.8 MB |

The binary grew by 7.3 MB: the bundled fonts (0.9 MB), the icon masks
(4.6 MB, embedded with `include_bytes!`) and the sign-in code. Packaging (M8)
can move the icons next to the binary if size becomes a concern; the target is
30 MB.

## M3: history scroll

Mac mini, Apple M4, macOS 27.0.1, external 4K display at 120 Hz with 2×
scaling (the display ran at 160 Hz in the spike). `scripts/history-perf.sh`
runs the `history-perf` example (release build): Hiking Club from the snapshot
fixtures with 10,000 generated messages (fixed seed; 4 authors in runs, 1 to
160 words with bold, italic, monospace and link spans, Cyrillic and emoji,
every 15th a reply), 1000×800 window kept on top. After opening the chat and
2 s of warm-up it scrolls towards older messages at 3,000 px/s for 60 s,
driven per frame on the wheel path. TDLib gets no parameters, so nothing goes
online and no page loads during the run. Three runs; medians, ranges in
parentheses.

| Metric | Value |
|---|---|
| Frame build, p50 (gpui draw and present, `ZED_MEASUREMENTS=1`) | 3.53 ms (3.47–3.59) |
| Frame build, p99 | 5.44 ms (5.39–5.44) |
| Frame build, max | 9.31 ms (8.57–9.95); one frame over 8 ms per run |
| Draw-to-draw interval, p50 / p99 / max | 8.33 / 10.31 / 10.50 ms (10.37–10.60) |
| Late frames (interval over 1.5 refresh periods) | 0 of 7,200 in every run |
| Scrolled | 180,000 px per run |
| History pixels changed by adding 50 older messages near the top | 0 in every run |

The p99 interval above one refresh period is callback jitter: no interval
reaches two periods, so no refresh was dropped. The run found a crash on the
first wheel event: the list calls its scroll handler with its state borrowed,
and the handler read the list again; the handler now defers its work. Not
measured: CPU, memory under scrolling (M5), real chats and their page loads.

After pull to next channel added a scroll handler to the history, one more
run (the display at 160 Hz this time): frame build p50 1.50 ms, p99 2.93 ms,
max 7.93 ms, no frame over 8 ms, no late frames, 0 history pixels changed.

## M4: emoji panel

Not measured yet. The panel's 1,900 emoji sit in a gpui `list`, so a frame
lays out only the visible rows (about 11 of 9 cells); the rows are built
once when the panel opens and the panel draws nothing while hidden. To
check on the live run: frame times while scrolling the panel quickly.

## How to measure

- Frame timing and memory under scrolling: the spike method in
  [ADR 0002](adr/0002-gui.md). The history view: `scripts/history-perf.sh`
  (below); the same run against real chats waits for API credentials.
- Idle: start the release binary, wait 5 s, then take the CPU time from
  `ps -o time= -p <pid>` at the start and end of 10 s, the footprint from
  `vmmap --summary <pid>`, and idle wakeups from
  `top -l 3 -s 5 -pid <pid> -stats pid,cpu,idlew`.
- Redraws after idle: the `frame-wake` example (ADR 0005).

## M4: attachments

- Picked, pasted and dropped files are read (metadata, image dimensions
  from the header only via `image::image_dimensions`) on the background
  executor; the send box opens when that finishes.
- Box thumbnails load through gpui's image cache from the path, scaled
  with `ObjectFit::Cover`; nothing is decoded on the main thread by us.
- The upload row reads the file's state from `Chats::file` per frame of
  the visible bubble only; `updateFile` notifies the history once per
  update. Not measured yet with a large live upload (live test 10).

## M4: voice recording

- The audio callback only mixes to mono and sends one `Vec` per buffer to
  the encoder thread; resampling, Opus and the OGG writes run there.
  Progress comes back through atomics, so the UI never waits on audio.
- While recording, the history view asks for a frame every refresh (the
  time, the dot and the level); nothing animates once it stops. Not
  measured yet: CPU while recording on battery (live test 11).

## M5: photos in the history

- A photo's box comes from its dimensions, so the history does not jump
  when the picture arrives. The file downloads once per history view at
  priority 16 (userpics 1).
- gpui decodes the downloaded file off the main thread and keeps it in its
  global image cache with no eviction, at full size (TDLib's largest size,
  up to 2560 px). Not measured yet: memory after scrolling a photo-heavy
  channel (M5 live test 1); a per-history cache with eviction is the
  upgrade if it grows.

## M5: userpic cache soak

- Downloaded userpics are decoded on the background executor, filled into
  their device-pixel square (Lanczos3) and cut to a circle (`ui::round_image`).
  The cache keeps the 512 most recently drawn; each eviction calls
  `App::drop_image`, which frees the texture in every window's atlas. A
  failed decode waits 2 s, doubling to 60 s, or until the file's time or
  length changes.
- Soak, 2026-10-28, macOS arm64, dev-profile example build:
  `SNAPSHOTS_SOAK=300 cargo run -q -p rgram --example snapshots` scrolls a
  list of 2000 chats with 2000 distinct 160 px userpics (four times the
  capacity) at about 33 rows a second, up and down, for 5 minutes. RSS in
  MB every 15 s: 133, 189, 185, 196, 238, 236, 236, 223, 223, 223, 224,
  236, 233, 233, 238, 233, 233, 239, 233, 233. Flat at 223-239 MB once the
  first pass (about 60 s) filled the cache and the atlas.
- The same soak with eviction that drops the image without `drop_image`
  (the textures stay in the atlas) grows without bound: 133, 208, 249, 284,
  334, 369, 388, 400, 415, 428, 441, 455 MB in 165 s, about 2 MB a second.
- Media viewer (`history/media_view.rs`): the full photo loads through
  gpui's `img` by path, the same path the history bubble already decoded,
  so opening a downloaded photo decodes nothing new; a photo seen only in
  the viewer stays in gpui's image cache like the history's photos do.
  Not measured: paging through hundreds of large photos in one session;
  if RSS climbs, drop the image on page as the userpic cache does.
- Video decoding (`media/src/video.rs`): one thread per playing video,
  frames scaled to BGRA no larger than the size they show at and queued two
  deep, so a 1280x720 video shown at 640x360 holds three 0.9 MB frames
  (2.7 MB) beyond FFmpeg's own buffers; a paused video blocks its thread on
  the full queue and decodes nothing. Hardware decoding (VideoToolbox here)
  takes the decode off the CPU; the copy down and the swscale pass stay on
  the decoder thread. Not measured yet: CPU and RSS while a long 1080p
  video plays in the viewer (with the player).
- Video in the viewer (`history/media_video.rs`): frames become one
  `RenderImage` each, only for the latest frame due at a draw; the one it
  replaces goes back to the atlas through `drop_image` on the next draw
  (the last frame of a closed video on the next video's first draw), so
  the atlas holds one or two frames per playing video. The view asks for
  animation frames only while playing or waiting for a frame after a
  seek; paused, it draws nothing. Not measured yet: CPU while a 1080p
  video plays (live test 19).
- Videos in bubbles (`history/inline_video.rs`): a decoder only for the
  rounds and GIFs the last layout drew; the rest stop at the next draw, so
  a long chat of GIFs runs only the few on screen (a 240 px round at 2x:
  three 0.9 MB frames). The history asks for animation frames only while
  one plays; a GIF that fails to decode is remembered and not reopened
  each frame. Not measured yet: CPU with six GIFs on screen (live test 20).
  WebM stickers take the same path on libvpx, in software: a 512 px VP9
  sticker shown at 224 pt decodes at 448 px on a 2x screen. Not measured
  yet: CPU with a screen of video stickers (live test 21).
