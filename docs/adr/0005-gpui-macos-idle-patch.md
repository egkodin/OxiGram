# ADR 0005: Patched gpui macOS backend for idle CPU

- Status: accepted
- Date: 2026-10-26

## Context

The brief asks for ~0 % CPU and zero redraws while nothing animates. The M0 empty
window idled at 1.2 % CPU (docs/perf.md). A `sample` showed why: gpui's macOS
backend (`gpui_macos`) subscribes each visible window to the display's
`CVDisplayLink` and keeps it subscribed until the window is occluded, so the
main thread wakes on every refresh (160 Hz on the test display) only to find
nothing to draw.

gpui's core is already demand-driven. Every source of frame demand
(`Window::refresh`, `cx.notify`, `on_next_frame`, throttled frames) calls the
platform's `PlatformWindow::frame_waker`, and the core's own comments say it is
there for "platforms that stop requesting frames for idle windows". The macOS
backend does not implement `frame_waker`, so it cannot stop the link safely.

Options:

1. Accept 1.2 % and record it. It drains laptop batteries and misses the target.
2. Fork Zed. Too much to carry for a 70-line change.
3. Patch only `gpui_macos` with Cargo's `[patch]`, keeping the rest of gpui at
   the ADR 0002 pin.

## Decision

Option 3. `vendor/gpui_macos` is Zed's crate at the pinned rev plus
`vendor/patches/gpui_macos-0001-park-idle-display-link.patch`:

- After a display-link frame request that drew nothing, a window counts an idle
  frame; after 30 of them (a quarter second at 120 Hz) it unsubscribes from the
  link ("parked"). A link with no subscribers stops.
- `frame_waker` restarts a parked link. It runs on the main thread; if the window
  state lock is already held further up the stack, it retries from the
  foreground executor instead of blocking.
- Every existing path that restarts the link (occlusion, screen change, key
  window, `display_layer`) also clears the parked state, so it never conflicts
  with upstream's start and stop logic.

`[patch."https://github.com/zed-industries/zed"]` in the root `Cargo.toml`
points `gpui_macos` at the vendored copy, and `vendor/` is excluded from the
workspace, so our lints do not apply to Zed's code. The crate stays under
Apache-2.0, and its license file sits beside it (NOTICE.md).

`scripts/vendor-gpui-macos.sh` re-creates the vendored crate. It copies the
sources from the Zed checkout in `~/.cargo/git`, rebuilds the manifest from
`cargo metadata` on Zed's own workspace (the upstream manifest inherits from it),
and applies the patches in `vendor/patches/`.

## Consequences

- Idle CPU drops from 1.2 % to 0.0 % (docs/perf.md, M1).
- The first frame after an idle period waits for the next vsync of the restarted
  link: 9–10 ms in the `frame-wake` example, against at most one refresh interval
  before. Animations and continuous input never park, because every frame they
  cause draws.
- Moving the gpui pin (ADR 0002) adds one step: run the script. If the patch no
  longer applies, the script fails and the patch is updated by hand.
- Linux and Windows are not patched. Whether their backends idle at zero is
  measured on those systems in M8.
- `crates/app/examples/frame-wake.rs` checks the behavior. A counter changes
  every two seconds; with
  `RUST_LOG=gpui_macos=trace,frame_wake=info cargo run -p rgram --example frame-wake`
  each "rendered" line must follow "display link resumed", and "display link
  parked" must follow it.
- If upstream implements `frame_waker` for macOS, the patch is dropped and this
  ADR is superseded.

## Addendum: interface scale (2026-11-04)

tdesktop's "Default interface scale" (50–300 %) enlarges the whole UI. gpui has
no app-wide zoom, so `vendor/patches/gpui_macos-0002-interface-zoom.patch` adds
one to the same vendored crate: `gpui_macos::set_interface_zoom` is read before
the first window opens. The window reports `backing scale × zoom` as its scale
factor and divides its content size, mouse positions, screen points and pixel
scroll deltas by the zoom; the Metal layer keeps the real backing scale, and
the minimum size, `resize` and the IME rectangle go back to points by
multiplying. A new scale needs a restart, as in tdesktop.

- Linux and Windows ignore the setting until their backends get the same
  patch (M8).
- Snapshots run on the test platform, so they cannot show the zoom; it was
  checked live at 150 % (drawing, clicks, small targets).
