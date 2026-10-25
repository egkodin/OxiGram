//! rgram: startup, the main window, and feature composition. No logic lives here.

mod credentials;

use anyhow::Context as _;
use futures::StreamExt as _;
use gpui::{
    App, AppContext as _, Bounds, Context, IntoElement, ParentElement, Render, Rgba, SharedString,
    Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder as _,
    px, size,
};
use style::{Color, Palette};
use td_api::enums::OptionValue;
use td_api::functions::{Close, GetOption, SetLogVerbosityLevel};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

// From Telegram/SourceFiles/window/window.style (windowDefault*, windowMin*);
// style-gen exports window.style with the first screens that need it.
const WINDOW_DEFAULT_WIDTH: f32 = 800.0;
const WINDOW_DEFAULT_HEIGHT: f32 = 600.0;
const WINDOW_MIN_WIDTH: f32 = 380.0;
const WINDOW_MIN_HEIGHT: f32 = 480.0;

/// TDLib log verbosity: errors only. Levels above 2 log message content.
const TDLIB_LOG_VERBOSITY: i32 = 1;

fn main() -> anyhow::Result<()> {
    init_logging()?;
    tg::execute(&SetLogVerbosityLevel {
        new_verbosity_level: TDLIB_LOG_VERBOSITY,
    })
    .context("setting TDLib's log verbosity")?;

    let notice = credentials::from_env()
        .err()
        .map(|error| SharedString::from(error.to_string()));
    if notice.is_some() {
        tracing::error!("Telegram API credentials are missing or invalid");
    }

    let runtime = tg::Runtime::start().context("starting TDLib")?;
    let (client, mut updates) = runtime.create_client();

    gpui_platform::application().run(move |cx: &mut App| {
        let version = client.request(&GetOption {
            name: "version".to_owned(),
        });
        cx.background_executor()
            .spawn(async move {
                match version.await {
                    Ok(OptionValue::String(version)) => {
                        tracing::info!(version = %version.value, "TDLib is ready");
                    }
                    Ok(_) => tracing::error!("TDLib returned a non-string version"),
                    Err(error) => tracing::error!(%error, "TDLib version request failed"),
                }
            })
            .detach();
        cx.background_executor()
            .spawn(async move {
                // The model consumes updates from M1 on; until then they are
                // drained. Never log them: they carry message text.
                while updates.next().await.is_some() {}
            })
            .detach();

        let close_client = client.clone();
        cx.on_app_quit(move |_| {
            // Best effort within gpui's shutdown timeout; M1 closes TDLib before
            // quitting, once it holds a database.
            let closed = close_client.request(&Close {});
            async move {
                if let Err(error) = closed.await {
                    tracing::warn!(%error, "closing TDLib failed");
                }
            }
        })
        .detach();

        if let Err(error) = open_main_window(cx, notice) {
            tracing::error!("{error:#}");
            cx.quit();
            return;
        }
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
    drop(runtime);
    Ok(())
}

fn init_logging() -> anyhow::Result<()> {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .with_env_var("RGRAM_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|error| anyhow::anyhow!(error))
        .context("initializing logging")
}

fn open_main_window(cx: &mut App, notice: Option<SharedString>) -> anyhow::Result<()> {
    let bounds = Bounds::centered(
        None,
        size(px(WINDOW_DEFAULT_WIDTH), px(WINDOW_DEFAULT_HEIGHT)),
        cx,
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(WINDOW_MIN_WIDTH), px(WINDOW_MIN_HEIGHT))),
        titlebar: Some(TitlebarOptions {
            title: Some("rgram".into()),
            ..Default::default()
        }),
        app_id: Some("rgram".to_owned()),
        ..Default::default()
    };
    cx.open_window(options, |_, cx| cx.new(|_| MainWindow { notice }))
        .context("opening the main window (rgram needs a working GPU driver)")?;
    Ok(())
}

/// The empty main window; shows a startup problem when there is one.
struct MainWindow {
    notice: Option<SharedString>,
}

impl Render for MainWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let palette = &Palette::DEFAULT;
        div()
            .size_full()
            .bg(rgba(palette.window_bg))
            .text_color(rgba(palette.window_fg))
            .when_some(self.notice.clone(), |root, notice| {
                root.flex().items_center().justify_center().child(notice)
            })
    }
}

fn rgba(color: Color) -> Rgba {
    gpui::rgba(u32::from_be_bytes([color.r, color.g, color.b, color.a]))
}
