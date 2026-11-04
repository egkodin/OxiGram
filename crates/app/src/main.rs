//! rgram: startup, the main window, and feature composition. No logic lives here.

mod client;
mod credentials;
mod device;
mod paths;
mod shutdown;

use anyhow::Context as _;
use features::{Accounts, Root};
use futures::StreamExt as _;
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement, Render,
    SharedString, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, div, px, size,
};
use model::{Environment, Parameters, Secret};
use style::{ColorId, ColorRef};
use td_api::enums::OptionValue;
use td_api::functions::{GetOption, SetLogVerbosityLevel};
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

const TEST_DC_VAR: &str = "RGRAM_TEST_DC";
/// `RGRAM_WITHHOLD_VIEWS=1` browses without reading messages, for checks on
/// a real account.
const WITHHOLD_VIEWS_VAR: &str = "RGRAM_WITHHOLD_VIEWS";
const SETTINGS_FILE: &str = "settings.json";

fn main() -> anyhow::Result<()> {
    init_logging()?;
    tg::execute(&SetLogVerbosityLevel {
        new_verbosity_level: TDLIB_LOG_VERBOSITY,
    })
    .context("setting TDLib's log verbosity")?;
    if std::env::args().nth(1).as_deref() == Some("--version") {
        return print_version();
    }

    let environment = environment();
    let parameters = credentials::from_env()
        .map_err(|error| SharedString::from(error.to_string()))
        .and_then(|credentials| {
            parameters(credentials, environment)
                .map_err(|error| SharedString::from(format!("{error:#}")))
        });
    if parameters.is_err() {
        tracing::error!(
            "cannot start: Telegram API credentials or the data directory are unusable"
        );
    }

    let data =
        paths::data_dir(environment).map_err(|error| SharedString::from(format!("{error:#}")));
    let settings = data.clone().map(|dir| dir.join(SETTINGS_FILE));
    let runtime = tg::Runtime::start().context("starting TDLib")?;
    gpui_platform::application().run(move |cx: &mut App| {
        if let Err(error) = ui::init(cx) {
            tracing::error!("{error:#}");
            cx.quit();
            return;
        }
        features::init(cx);
        match settings {
            Ok(path) => features::settings::load(path, cx),
            Err(error) => tracing::warn!("{error}"),
        }
        // Before the window opens: windows read the zoom as they are made.
        #[cfg(target_os = "macos")]
        gpui_macos::set_interface_zoom(features::settings::interface_zoom(cx));
        let parameters = parameters.map(
            |parameters| match features::settings::custom_device_model(cx) {
                Some(device_model) => Parameters {
                    device_model,
                    ..parameters
                },
                None => parameters,
            },
        );
        let content = match (parameters, data) {
            (Ok(parameters), Ok(data)) => {
                Content::Signed(client::start(runtime, parameters, data, cx))
            }
            (Err(notice), _) | (_, Err(notice)) => {
                start_idle_client(&runtime, cx);
                // The idle client only has to close; TDLib's threads stop with the process.
                std::mem::forget(runtime);
                Content::Notice(notice)
            }
        };
        if let Err(error) = open_main_window(cx, content) {
            tracing::error!("{error:#}");
            shutdown::quit(cx);
            return;
        }
        cx.activate(true);
    });
    Ok(())
}

/// `RGRAM_TEST_DC=1` uses Telegram's test servers (separate accounts).
fn environment() -> Environment {
    match std::env::var(TEST_DC_VAR).as_deref() {
        Ok("1" | "true") => Environment::Test,
        _ => Environment::Production,
    }
}

fn parameters(
    credentials: credentials::ApiCredentials,
    environment: Environment,
) -> anyhow::Result<Parameters> {
    let data = paths::data_dir(environment)?;
    std::fs::create_dir_all(&data)
        .with_context(|| format!("creating the data directory {}", data.display()))?;
    let path = |name: &str| data.join(name).to_string_lossy().into_owned();
    Ok(Parameters {
        environment,
        database_directory: path(paths::DATABASE_DIR),
        files_directory: path(paths::FILES_DIR),
        language_pack_database: path(paths::LANGUAGE_PACKS),
        api_id: credentials.id,
        api_hash: Secret::new(credentials.hash),
        system_language: paths::system_language(),
        device_model: device::model(),
        system_version: device::system_version(),
        application_version: env!("CARGO_PKG_VERSION").to_owned(),
        views: match std::env::var(WITHHOLD_VIEWS_VAR).as_deref() {
            Ok("1" | "true") => model::Views::Withhold,
            _ => model::Views::Report,
        },
    })
}

/// Without credentials the client never signs in; it runs only so quitting
/// follows the same path.
fn start_idle_client(runtime: &tg::Runtime, cx: &mut App) {
    let (client, mut updates) = runtime.create_client();
    let (closed_sender, closed) = shutdown::closed_signal();
    cx.background_executor()
        .spawn(async move {
            while updates.next().await.is_some() {}
            closed_sender.send(()).ok();
        })
        .detach();
    shutdown::init(cx, runtime.stopper());
    shutdown::set_client(cx, 0, client, closed);
}

/// Prints the rgram and TDLib versions; launching the binary directly checks
/// that it finds `libtdjson` on its own.
fn print_version() -> anyhow::Result<()> {
    let version = tg::execute(&GetOption {
        name: "version".to_owned(),
    })
    .context("reading TDLib's version")?;
    let OptionValue::String(version) = version else {
        anyhow::bail!("TDLib returned a non-string version");
    };
    println!(
        "rgram {} (TDLib {})",
        env!("CARGO_PKG_VERSION"),
        version.value
    );
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

enum Content {
    Signed(Entity<Accounts>),
    Notice(SharedString),
}

fn open_main_window(cx: &mut App, content: Content) -> anyhow::Result<()> {
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
    match content {
        Content::Signed(accounts) => cx
            .open_window(options, |window, cx| {
                cx.new(|cx| Root::new(accounts, window, cx))
            })
            .map(drop),
        Content::Notice(notice) => cx
            .open_window(options, |_, cx| cx.new(|_| Notice { notice }))
            .map(drop),
    }
    .context("opening the main window (rgram needs a working GPU driver)")
}

/// Why rgram cannot start, such as missing API credentials.
struct Notice {
    notice: SharedString,
}

impl Render for Notice {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_8()
            .flex()
            .items_center()
            .justify_center()
            .bg(ui::color(cx, ColorRef::Palette(ColorId::WindowBg)))
            .text_color(ui::color(cx, ColorRef::Palette(ColorId::WindowFg)))
            .child(self.notice.clone())
    }
}
