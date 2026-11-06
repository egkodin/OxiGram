//! Checks that a window redraws after its display link has parked (ADR 0005).
//!
//! A counter changes every two seconds, long after the link parks. Run it with
//! `RUST_LOG=gpui_macos=trace,frame_wake=info cargo run -p oxigram --example frame-wake`:
//! each "rendered" line must follow a "display link resumed" line on macOS.

use std::time::Duration;

use gpui::{
    App, AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, WindowBounds, WindowOptions, div, px, size,
};
use tracing_subscriber::EnvFilter;

const TICK: Duration = Duration::from_secs(2);
const TICKS: u32 = 3;

struct Counter {
    ticks: u32,
}

impl Render for Counter {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        tracing::info!(ticks = self.ticks, "rendered");
        div()
            .size_full()
            .bg(gpui::white())
            .text_color(gpui::black())
            .child(format!("tick {}", self.ticks))
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    gpui_platform::application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(320.0), px(160.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        let opened = cx.open_window(options, |_, cx| {
            cx.new(|cx| {
                cx.spawn(async move |this, cx| {
                    for _ in 0..TICKS {
                        cx.background_executor().timer(TICK).await;
                        this.update(cx, |this: &mut Counter, cx| {
                            this.ticks += 1;
                            cx.notify();
                        })?;
                    }
                    cx.background_executor().timer(TICK).await;
                    cx.update(|cx| cx.quit());
                    anyhow::Ok(())
                })
                .detach();
                Counter { ticks: 0 }
            })
        });
        if let Err(error) = opened {
            tracing::error!("{error:#}");
            cx.quit();
        }
        cx.activate(true);
    });
}
