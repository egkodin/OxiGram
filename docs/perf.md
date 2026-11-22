# Performance and memory

## Offline startup and shutdown measurements (2026-11-22)

On the arm64 macOS development machine, the two subprocesses used to read
device details took a median 16.08 ms (p90 29.95 ms). One `sysctl` invocation
took a median 3.80 ms (p90 6.14 ms). Each path ran 32 times in alternating
order using a monotonic clock and captured subprocess output. This measures
the device lookup during startup. Paths with unusable credentials also skip
creating a TDLib runtime and client.

`cargo test --locked --offline -p tg close_last_client_then_stop_receiver -- --ignored --nocapture`
creates and closes two successive TDLib clients without credentials or a
database. After the final update stream ends, the probe waits 50 ms and
times the receiver join. One run took 950.90 ms before and 0.16 ms after
the receiver began waiting for new clients between sessions. The test also
checks that a second client can open and close after the first.

Signed-in window startup, database close time and forced OS shutdown were
outside these measurements.

## Release samples, 2026-11-20

The installed macOS release bundle was sampled on a signed-in account
with read receipts withheld. CPU is process CPU time divided by elapsed
wall time, with one fully occupied core counting as 100%. RSS is resident
memory as reported by the OS.

| Activity | Sampling interval | RSS | Mean CPU |
|---|---|---:|---:|
| Idle chat list | 50 s | 206.8–209.2 MB | 0.32% |
| Video playing | 11 s | 300.3–310.7 MB | 11.6% |

The video average excludes the later samples taken with the profiler
attached. Live work ended when the audit moved to code only. The
5,000-message scroll, animated-emoji screen and 30-minute growth check
have no measurements for this run. No allocation ranking or before/after
performance claim follows from these two samples.

## Offline follow-up, 2026-11-20

All sizes below use decimal MB. The signed-in idle baseline was 206.8–209.2 MB
RSS before the audit fixes, or 106.8–109.2 MB over the new goal. The historical
empty window already used 101.4 MB RSS. Today's cache workloads isolate two
retention costs. They cannot apportion the remaining idle RSS among TDLib,
model state, GPU resource mappings, decoded images, and resident code.

### Cache retention

| Offline workload | Before entries | Before MB | After entries | After MB |
|---|---:|---:|---:|---:|
| One account, 10,000 distinct loaded emoji with local paths | 10,000 | 6.72 | 3,850 | 2.28 |
| One account, 128 reaction results with three paths per result | 128 | 0.09 | 128 | 0.09 |
| One account, 50,000 userpic download tracking records | 50,000 | 0.46 | 50,000 | 0.46 |
| Shared decoded userpic cache, 2,000 distinct 160 × 160 BGRA images | 512 | 52.43 | 234 | 23.96 |

The account test counts usable hash-table slots, boxed metadata and owned
string capacity. It excludes hash-table control bytes, allocator overhead
and decoded pixels. The decoded userpic test counts actual pixel bytes in
`RenderImage` frames and spaces insertions 30 ms apart on a synthetic clock.
Peak retained pixels were 52,428,800 bytes before and 23,961,600 bytes after.
The after test process peaked at 36.08 MB RSS without a GPU atlas. These
workloads use separate processes. Their savings cannot be added to estimate
the signed-in result.

The userpic cache keeps its 512-entry ceiling and starts evicting cold
images above 24 MB of decoded pixels. Recent draws and decodes can exceed
the byte budget until later completions trigger eviction. Evictions still
release the corresponding atlas image. A fixture test kept an active image
and decoded an evicted image back to identical pixels. The emoji cache trims
old entries in batches above 4,096, retaining recent lookups and pending
requests. The cache test verified that returning to an evicted key starts
a fresh fetch and accepts its completion. Reaction and userpic tracking
metadata keep their existing policies. Both were small in this workload.

Follow-up mask test: the RGBA-to-BGRA circular mask now edits the resized
buffer in place. An optimized standalone harness ran the old and new
`round_image::circled` on the same 2048 × 2048 synthetic RGBA square
(`[10, 20, 30, 200]`). In three fresh child processes per version,
macOS `getrusage` reported 35.54 MB peak RSS before and 18.73 MB after.
The 16,777,216 output bytes matched byte for byte. The unit regression
also verifies buffer reuse and the center, corner and antialiased edge
pixels. This measures masking alone. At 160 × 160, the eliminated
second buffer is 0.1024 MB per concurrent mask.

Run the cache workloads with:

```sh
cargo test --locked --offline --workspace --all-features --lib offline_account_cache_workload -- --ignored --nocapture
cargo test --locked --offline --workspace --all-features --lib offline_userpic_cache_workload -- --ignored --nocapture
```

### TDLib update delivery

The sole `td_receive` caller sees all clients. It sends replies by `@extra`
to waiting one-shots and sends updates by `@client_id` to per-client FIFOs.
`authorizationStateClosed` enters that account's FIFO after its earlier
updates, removes its sender, and cancels any outstanding replies for that
client. The GPUI pump applies events in order and signals shutdown when the
stream ends. The runtime stop path drops all senders and pending replies.

`stalled_account_keeps_order_without_delaying_another_reply_or_close`
injected 8,192 sequenced updates before polling the first account. It
confirmed the second account could receive its reply, cancel a pending
request and close while the first remained stalled. The first account
then received all 8,192 updates and its close in order. A blocking bounded
send would stop the shared receiver before it sees the second account's
messages. A nonblocking bounded send would reject updates, and a secondary
in-memory backlog would move the same growth elsewhere. The queue remains
unbounded. The test did not establish a lossless policy for an indefinitely
stalled account within a fixed RAM budget.

### Video decoder peak RSS

An offline fixture exercises the real video decoder without the viewer,
audio, TDLib or a GPU atlas. The static FFmpeg dev build decodes 240 H.264
frames at 1920 × 1080 into 960 × 540 BGRA at the clip's 30 fps cadence.
Each run uses a fresh test process. Peak RSS is the child process's
`getrusage(RUSAGE_CHILDREN).ru_maxrss` in bytes on macOS, read by a Python
parent running only that process. The media test binary used 8.54 MB peak
RSS when it ran no tests.

| Decoder build | Peak RSS, median of 3 | Range |
|---|---:|---:|
| Existing and retained implementation | 111.33 MB | 110.95–111.39 MB |
| Experiment with an exactly preallocated BGRA buffer and row copies | 122.90 MB | 120.90–125.49 MB |

The experiment reduced time to drain frames in the unpaced dev workload
from 4.67 s to 0.88 s. The paced RSS regression outweighed that result,
so frame packing remains as before. The RSS includes FFmpeg's decoder
surfaces and buffers, native framework mappings and Rust frame buffers.
The fixed output queue itself is two frames deep, or 4.15 MB of 960 × 540
BGRA payload at capacity. The remaining peak was not apportioned by allocator
or subsystem. Release playback, startup and scrolling were outside this
offline comparison.

```sh
ffmpeg -nostdin -f lavfi -i testsrc2=size=1920x1080:rate=30 -frames:v 240 \
  -an -c:v libx264 -pix_fmt yuv420p -preset ultrafast /tmp/oxigram-1080p.mp4
cargo test --locked --offline --workspace --all-features --lib --no-run
OXIGRAM_OFFLINE_CLIP=/tmp/oxigram-1080p.mp4 python3 -c '
import resource, subprocess, sys
subprocess.run(sys.argv[1:], check=True)
print(resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss)
' /path/to/media-test-binary --ignored --exact video::tests::offline_video_rss_workload
```

Use the media test executable printed by Cargo and the pinned static
FFmpeg recipe from `scripts/build-ffmpeg.sh`.

### macOS bundle size

Measured on macOS 27.2.1 arm64 by summing the logical byte length of
every regular file in each signed bundle. `otool -L` confirmed that TDLib
is the only bundled dylib. FFmpeg, libvpx, dav1d, Opus, embedded icons and
fonts are included in the executable. macOS supplies the system
frameworks and libraries.

| Bundle component | Preserved release | Offline release candidate |
|---|---:|---:|
| Complete .app | 65.46 MB (65,460,117 bytes) | 65.96 MB (65,960,725 bytes) |
| Signed executable, including static libraries | 35.37 MB | 35.87 MB |
| Signed TDLib dylib | 28.98 MB | 28.98 MB |
| Icon resource | 1.10 MB | 1.10 MB |
| Launcher, plist and bundle signature files | 0.0044 MB | 0.0044 MB |

For the candidate, built the current code in the normal stripped, fat-LTO
release profile with the rebuilt pinned static FFmpeg. Staged the resulting
executable and TDLib beside a copy of the preserved bundle's resources and
launcher. Both bundles passed `codesign --verify --deep --strict`.
The candidate used `runtime-shaders` because the host lacks the Metal
Toolchain. The regular packaging recipe uses precompiled shaders.

The executable and TDLib account for 98.3% of the candidate. The link map
attributed 6.50 MB of file-backed symbol bytes inside the executable to
FFmpeg and its bundled codecs, and another 0.40 MB to the separate Opus
encoder. These counts exclude alignment and zero-fill, and are included
in the executable row. The pre-signing candidate executable was 36.08 MB.

Budgets come from the project brief. They are starting targets: M1 records a
baseline on a real account and refines them here. A target is loosened only with
a written justification.

## Offline media follow-up, 2026-11-21

The same paced 1080p decode workload ran in three fresh processes with
the GIF-enabled static FFmpeg. Peak RSS was 115.93, 115.39 and 110.40 MB,
with a median of 115.39 MB against the prior 111.33 MB median. The samples
overlapped a release build. The comparison does not isolate a cause for
the difference and does not measure the signed-in app, its GPU atlas or
GIF-picker memory. The picker starts one muted decoder on hover and
caps its output at 320 × 240 pixels.

The arm64 release candidate at `target/current-6ece55c/OxiGram.app`
uses the stripped fat-LTO profile, static FFmpeg and runtime Metal
shaders. The preserved 65.46 MB bundle and the new candidate passed
`codesign --verify --deep --strict`. Summing the logical file sizes in
the complete signed candidate gave the following breakdown.

| Component | Bytes | Decimal MB |
|---|---:|---:|
| Complete .app | 66,273,781 | 66.27 |
| Native executable with FFmpeg and static libraries | 36,186,944 | 36.19 |
| TDLib dylib | 28,978,752 | 28.98 |
| Resources, launcher, plist and signing files | 1,108,085 | 1.11 |

`otool -L` found TDLib and macOS system libraries. The earlier
59.52 MB developer bundle at `target/current-b9ea15c/OxiGram.app`
loads five Homebrew FFmpeg dylibs from outside the app. It is excluded
from the complete-bundle baseline. The new candidate is 33.73 MB under
the 100 MB goal, and it uses runtime shaders because the precompiled
Metal toolchain was unavailable.

The checkout's `target/` used 4.24 GB after staging. The external
verification directory reached 5.08 GB of allocated disk space,
including 5.00 GB of Cargo build output, the FFmpeg build and the
fixtures. Its Cargo output was removed before final packaging and the
remaining scratch files after verification.

The last signed-in idle sample still stands at 206.8–209.2 MB RSS,
106.8–109.2 MB above the 100 MB goal. A new live sample and a breakdown
of resident allocations are still needed.

## Budgets

Release build, macOS Apple Silicon, account with ~500 chats.

| Metric | Target | Latest | Status |
|---|---|---|---|
| Cold start to interactive chat list (database present) | < 800 ms | — | M1 |
| Idle RSS on the chat list | < 100 MB | 206.8–209.2 MB before the audit fixes | Historical gap of 106.8–109.2 MB. Current result requires a signed-in run. |
| RSS after scrolling 10 media-heavy chats and returning | < 250 MB, no growth in a soak test | — | M5 |
| History scroll frame time, p99 | < 8 ms (120 Hz) | 5.44 ms frame build, no dropped refreshes (10,000 synthetic messages, 120 Hz) | ok; real chats not measured |
| Opening a chat to first messages (TDLib cache) | < 100 ms | — | M3 |
| Idle with no animations | ~0 % CPU, zero redraws | 0.0 % CPU, no redraws (empty window) | ok |
| Release executable, with static FFmpeg and excluding TDLib | < 30 MB | 35.37 MB in preserved bundle | Above the historical executable budget |
| Complete macOS release bundle | < 100 MB | 65.46 MB preserved release, 65.96 MB runtime-shader candidate | Within goal for both measured bundles |

## Measuring memory on macOS

`ps -o rss= -p <pid>` reports resident memory in units of 1024 bytes, including
resident shared mappings. Multiply by 1024 and divide by 1,000,000 for decimal
MB. Use RSS for the 100 MB goal. The older physical footprint samples
(`vmmap --summary <pid>`) describe a different metric. The empty M0 window had
101.4 MB RSS and 46.6 MB physical footprint.

## M0: empty window

Mac mini, Apple M4, 24 GB, macOS 27.0.1, external 4K display at 160 Hz with 2×
scaling. `target/release/oxigram` (`lto = "fat"`, `codegen-units = 1`,
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

## Metal shaders: precompiled or compiled at start

Mac mini, Apple M4, macOS 27.2, Xcode 27.2 (2026-11-03). gpui either embeds
a metallib built by `xcrun metal` (the default) or, with
`--features oxigram/runtime-shaders`, embeds the 66 KB stitched source and
compiles it in `MetalRenderer::new` when the first window opens.

| Case | Shader library ready in |
|---|---|
| Runtime compile, not in macOS's Metal cache (first launch, after an OS or driver update) | 201–273 ms (6 runs) |
| Runtime compile, cached by macOS | 0.1 ms |
| Precompiled metallib | below what `open_window` resolves (same 60–71 ms as the cached case) |

`cargo run -p oxigram --example cold-start` (both modes, 6 runs each, dev
build): `open_window` 60–71 ms and first frame 149–158 ms in both modes
once warm, because macOS caches the compiled source by its hash; the first
run of each new binary took about 200 ms in `open_window` either way.
The uncached compile was timed directly: the stitched source from gpui's
build, prefixed with a unique comment so the cache cannot match, through
`MTLDevice.makeLibrary(source:)` in a small Swift program.

Decision: runtime compilation costs about 200 ms on a cold start, so release
builds (`scripts/build-macos-app.sh`) keep precompiled shaders.
`runtime-shaders` stays for builds without the Metal Toolchain (CI);
`scripts/check-metal.sh` names the install command when it is missing.

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

## Lottie: a chat of 300 animated emoji

Mac mini, Apple M4, macOS 27.2, 2× scaling (2026-11-06). `SNAPSHOTS_ON_TOP=1
SNAPSHOTS_LOTTIE=60` with the snapshots example built in release (LTO off,
16 codegen units, so not the shipped profile): Maria's chat with 300 animated
emoji in eight TGS files (60 frames at 60 fps, 112 px, so 224 px frames at
2×), each its own player as tdesktop's per-message emoji. The window floats
over the others so the display link draws it as in the app. One run each;
`SNAPSHOTS_LOTTIE_EMPTY=1` is the same window without the emoji.

| Phase | CPU | Footprint | RSS | Players | Lottie frames/s |
|---|---|---|---|---|---|
| Empty chat, idle (baseline) | 1.9 % | 71 MB | 106 MB | 0 | 0 |
| Opened, the emoji on screen played once, then idle | 2.9 % | 137 MB | 120 MB | 5 | 9 (over 31 s) |
| Scrolling at 15,000 px/s for 60 s | 22–33 % | 187–220 MB | 188–218 MB | 63–68 | 700–1,560 |
| Left alone after scrolling, 30 s | 1.8 % | 167 MB | 219 MB | 5 | 8 |
| Power Usage all off, scrolling | 25 % | 170 MB | 222 MB | 70 | 18 |
| Power Usage all off, idle | 1.3 % | 149 MB | 222 MB | 6 | 0 |

- Idle CPU with the chat open equals the empty baseline: once each emoji has
  played, nothing redraws. The baseline's 1–2 % is the harness: TDLib gets
  no parameters, so the chat list asks for a page every 2 s and fails.
- Memory is bounded by the players: those off screen close after 3 s, and
  beyond 32 the oldest close at once. Before that cap a fast scroll kept all
  300 open and the footprint peaked at 412 MB. Frames are not cached (a
  224 px emoji is 12 MB of frames, over the 4.19 MB per-animation limit), so
  each is rendered on the `lottie` thread when due.
- Scrolling with Power Usage all off costs about as much as with it on: the
  scroll itself, not Lottie (18 frames/s are the first frames).
- RSS stays near its peak after scrolling (the allocator keeps the pages);
  the footprint is what comes back.
- Found on the way: the chat list asked for the next page every frame after
  a failed one, redrawing the window forever (3,000 frames in 20 s); it now
  waits 2 s.
- Not measured: the shipped release profile, a real chat on the account,
  big stickers (512 px frames) looping.

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
- gpui decodes the downloaded file off the main thread at full size
  (TDLib's largest size, up to 2560 px). Its own image cache never evicts,
  so every photo scrolled past stayed twice, decoded and as a texture.
  `history/picture_cache.rs` holds them instead: 67.11 MB of decoded pictures,
  the least recently drawn dropped (with `App::drop_image`) when a new one
  loads, and nothing drawn within the last second. A photo scrolled back to
  decodes again behind its thumbnail, as on first view.
- Photo soak, 2026-11-14, Mac mini M4, release harness (fat LTO, static
  FFmpeg): `SNAPSHOTS_PHOTOS=60` scrolls a chat of 200 photos, each its own
  1280×960 JPEG (4.9 MB decoded), up and down at 7,500 px/s. One run each.

  | Seconds | RSS before | Footprint before | RSS after | Footprint after |
  |---|---|---|---|---|
  | 0 | 154 MB | 82 MB | 153 MB | 82 MB |
  | 10 | 1,864 MB | 1,946 MB | 355 MB | 385 MB |
  | 20 | 2,020 MB | 2,150 MB | 350 MB | 380 MB |
  | 30–50 | 2,020 MB | 2,150 MB | 350–360 MB | 380–390 MB |

  Before, all 200 photos stayed (200 × 4.9 MB × 2). After, the level is the
  budget plus what one second of this scroll passes (about 23 photos, twice
  each); a slower scroll keeps less.
- Not measured: a photo-heavy channel on the account (M5 live test 1), and
  photos larger than the window are still decoded at full size.

## M5: userpic cache soak

- Downloaded userpics are decoded on the background executor, filled into
  their device-pixel square (Lanczos3) and cut to a circle (`ui::round_image`).
  The historical soak below used a cache of the 512 most recently drawn.
  Each eviction calls
  `App::drop_image`, which frees the texture in every window's atlas. A
  failed decode waits 2 s, doubling to 60 s, or until the file's time or
  length changes.
- Soak, 2026-10-28, macOS arm64, dev-profile example build:
  `SNAPSHOTS_SOAK=300 cargo run -q -p oxigram --example snapshots` scrolls a
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
  so opening a downloaded photo decodes nothing new; the viewer's picture
  sits in the same bounded cache as the history's photos.
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
- Sound (`media/src/audio.rs`): a decoder thread keeps half a second of
  f32 samples ahead of the output (192 KB at 48 kHz stereo), waiting on a
  condition variable while full; the output callback only `try_lock`s the
  buffer and plays silence for that callback when the decoder holds it,
  so it never waits on the decoder.
- Player bar (`player/`): while a voice or a song plays, the player
  notifies every 50 ms, which redraws the window (the bar, the time and
  the playing row), as the history's own ticker did before; paused or
  stopped, it notifies nothing. Not measured yet: CPU while a song plays
  in the background of an idle window (live test 23).

## M5: video streaming

Click to first frame on a video not in the cache, measured from the
viewer's player opening to the "first video frame" info log
(`history/media_video.rs`), with `OXIGRAM_SILENT=1` on the production DC,
2026-11-06, release app bundle on this Mac. The click itself goes through
computer use, about 0.2 s before the viewer opens (not counted).

| Video | Cache | First frame |
|---|---|---|
| 113.8 MB, 1:04, H.264 MP4 in a channel | none | 0.64 s |
| the same, after `deleteFile` (local copy only) | none | 1.76 s |
| the same, reopened | first half and the tail | 0.11 s |
| round video, 5.4 MB, 0:42 | none | 0.65 s |
| GIF, 2.4 MB | none | 0.30 s |

tdesktop's time on the same video: not measured.

Memory: FFmpeg reads through a 64 KB buffer and TDLib writes to its own
cache file (`files/temp`, sparse, 94 MB of 119 MB on disk after the seeks
above); nothing holds the file in RAM. The app's RSS while the video
played: 352 MB, the whole app with two accounts loaded. Seeking to 1:02
of 1:04 asked TDLib for that range only; the file was not downloaded in
full.

