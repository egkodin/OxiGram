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
| History scroll frame time, p99 | < 8 ms (120 Hz) | 7.68 ms (spike scene, 160 Hz) | on track |
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

## How to measure

- Frame timing and memory under scrolling: the spike method in
  [ADR 0002](adr/0002-gui.md). From M3 the history view gets the same automatic
  scroll run against real chats.
- Idle: start the release binary, wait 5 s, then take the CPU time from
  `ps -o time= -p <pid>` at the start and end of 10 s, the footprint from
  `vmmap --summary <pid>`, and idle wakeups from
  `top -l 3 -s 5 -pid <pid> -stats pid,cpu,idlew`.
- Redraws after idle: the `frame-wake` example (ADR 0005).
