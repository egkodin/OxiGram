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
| Idle with no animations | ~0 % CPU, zero redraws | 1.2 % CPU (empty window) | **over** |
| Release binary size without TDLib | < 30 MB | 9.7 MB | ok |

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
(160 Hz here). The gpui spike idled at the same 1.2–1.35 %. M1, which brings the
first real screen and the baseline, has to stop the display link while no window
is dirty, or record why the target cannot be met.

## How to measure

- Frame timing and memory under scrolling: the spike method in
  [ADR 0002](adr/0002-gui.md). From M3 the history view gets the same automatic
  scroll run against real chats.
- Idle: start the release binary, wait 5 s, then take the CPU time from
  `ps -o time= -p <pid>` at the start and end of 10 s, and the footprint from
  `vmmap --summary <pid>`.
