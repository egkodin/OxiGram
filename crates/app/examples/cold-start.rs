//! Times a window's start: from `main` to the window opening (the Metal
//! renderer, and with `runtime-shaders` the shader compile) and to its first
//! frame, then quits. Compare the shader modes (docs/perf.md):
//! `cargo run -q -p oxigram --example cold-start` and the same with
//! `--features oxigram/runtime-shaders`.

use std::time::{Duration, Instant};

use gpui::{
    App, AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, WindowBounds, WindowOptions, div, px, size,
};

struct Probe;

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(gpui::white()).child("cold start")
    }
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn main() {
    let start = Instant::now();
    gpui_platform::application().run(move |cx: &mut App| {
        let launched = start.elapsed();
        let bounds = Bounds::centered(None, size(px(320.0), px(160.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        let before = Instant::now();
        let opened = cx.open_window(options, |_, cx| cx.new(|_| Probe));
        let open = before.elapsed();
        match opened {
            Ok(window) => {
                let drawn = window.update(cx, move |_, window, _| {
                    window.on_next_frame(move |_, cx| {
                        println!(
                            "launch {:.1} ms, open_window {:.1} ms, first frame {:.1} ms",
                            millis(launched),
                            millis(open),
                            millis(start.elapsed()),
                        );
                        cx.quit();
                    });
                });
                if let Err(error) = drawn {
                    eprintln!("{error:#}");
                    cx.quit();
                }
            }
            Err(error) => {
                eprintln!("{error:#}");
                cx.quit();
            }
        }
        cx.activate(true);
    });
}
