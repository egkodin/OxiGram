//! rgram: startup, the main window, and feature composition. No logic lives here.

mod client;
mod credentials;
mod device;
mod paths;
mod shutdown;

use anyhow::Context as _;
use features::{Accounts, Root};
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement, Render,
    SharedString, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, div, px, size,
};
use model::{Environment, Parameters, Secret};
use style::{ColorId, ColorRef};
use td_api::enums::{LogStream, OptionValue};
use td_api::functions::{GetOption, SetLogStream};
use td_api::types::LogStreamEmpty;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

// From Telegram/SourceFiles/window/window.style (windowDefault*, windowMin*);
// style-gen exports window.style with the first screens that need it.
const WINDOW_DEFAULT_WIDTH: f32 = 800.0;
const WINDOW_DEFAULT_HEIGHT: f32 = 600.0;
const WINDOW_MIN_WIDTH: f32 = 380.0;
const WINDOW_MIN_HEIGHT: f32 = 480.0;

const TEST_DC_VAR: &str = "RGRAM_TEST_DC";
/// `RGRAM_WITHHOLD_VIEWS=1` browses without reading messages, for checks on
/// a real account.
const WITHHOLD_VIEWS_VAR: &str = "RGRAM_WITHHOLD_VIEWS";
/// `RGRAM_SILENT=1` plays nothing aloud, for live checks of the players.
const SILENT_VAR: &str = "RGRAM_SILENT";
const SETTINGS_FILE: &str = "settings.json";

fn main() -> anyhow::Result<()> {
    install_panic_hook();
    init_logging()?;
    tg::execute(&SetLogStream {
        log_stream: Box::new(LogStream::Empty(LogStreamEmpty {})),
    })
    .context("disabling TDLib's native log")?;
    match std::env::args().nth(1).as_deref() {
        Some("--version") => return print_version(),
        Some("--check-release-configuration") => {
            return credentials::check_release_configuration()
                .map_err(|_| anyhow::anyhow!("Missing or invalid bundled Telegram configuration"));
        }
        _ => {}
    }

    if matches!(std::env::var(SILENT_VAR).as_deref(), Ok("1" | "true")) {
        media::silence_output();
    }
    let environment = environment();
    let data =
        paths::data_dir(environment).map_err(|error| SharedString::from(format!("{error:#}")));
    let settings = data.as_ref().ok().map(|dir| dir.join(SETTINGS_FILE));
    let connection = credentials::from_env()
        .map_err(|error| SharedString::from(error.to_string()))
        .and_then(|credentials| {
            let data = data?;
            let parameters = parameters(credentials, environment, &data)
                .map_err(|error| SharedString::from(format!("{error:#}")))?;
            let runtime = tg::Runtime::start()
                .context("starting TDLib")
                .map_err(|error| SharedString::from(format!("{error:#}")))?;
            Ok((runtime, parameters, data))
        });
    if connection.is_err() {
        tracing::error!("cannot start a Telegram connection");
    }
    gpui_platform::application().run(move |cx: &mut App| {
        if let Err(error) = ui::init(cx) {
            tracing::error!("{error:#}");
            cx.quit();
            return;
        }
        features::init(cx);
        match settings {
            Some(path) => features::settings::load(path, cx),
            None => tracing::warn!("settings unavailable because the data directory is unusable"),
        }
        // Before the window opens: windows read the zoom as they are made.
        #[cfg(target_os = "macos")]
        gpui_macos::set_interface_zoom(features::settings::interface_zoom(cx));
        let content = match connection {
            Ok((runtime, mut parameters, data)) => {
                if let Some(device_model) = features::settings::custom_device_model(cx) {
                    parameters.device_model = device_model;
                }
                Content::Signed(client::start(runtime, parameters, data, cx))
            }
            Err(notice) => {
                shutdown::init(cx, None);
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
    data: &std::path::Path,
) -> anyhow::Result<Parameters> {
    std::fs::create_dir_all(data)
        .with_context(|| format!("creating the data directory {}", data.display()))?;
    let path = |name: &str| data.join(name).to_string_lossy().into_owned();
    let (device_model, system_version) = device::describe();
    Ok(Parameters {
        environment,
        database_directory: path(paths::DATABASE_DIR),
        files_directory: path(paths::FILES_DIR),
        language_pack_database: path(paths::LANGUAGE_PACKS),
        api_id: credentials.id,
        api_hash: Secret::new(credentials.hash),
        system_language: paths::system_language(),
        device_model,
        system_version,
        application_version: env!("CARGO_PKG_VERSION").to_owned(),
        views: match std::env::var(WITHHOLD_VIEWS_VAR).as_deref() {
            Ok("1" | "true") => model::Views::Withhold,
            _ => model::Views::Report,
        },
    })
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
        "OxiGram {} (TDLib {})",
        env!("CARGO_PKG_VERSION"),
        version.value
    );
    Ok(())
}

fn install_panic_hook() {
    // A panic payload or thread name can contain message text or a local path.
    std::panic::set_hook(Box::new(|info| {
        use std::io::Write as _;
        let mut stderr = std::io::stderr().lock();
        if let Some(location) = info.location() {
            let name = location
                .file()
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or("unknown");
            let _ = writeln!(stderr, "OxiGram panicked at {name}:{}", location.line());
        } else {
            let _ = writeln!(stderr, "OxiGram panicked");
        }
    }));
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
        // The bar is drawn in the theme's colours (`features::title_bar`).
        titlebar: Some(TitlebarOptions {
            title: Some(features::APP_NAME.into()),
            appears_transparent: true,
            ..Default::default()
        }),
        app_id: Some("oxigram".to_owned()),
        ..Default::default()
    };
    features::title_bar::enable(cx);
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
    .context("opening the main window (OxiGram needs a working GPU driver)")
}

/// Why OxiGram cannot start, such as missing API credentials.
struct Notice {
    notice: SharedString,
}

impl Render for Notice {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notice = div()
            .flex_1()
            .p_8()
            .flex()
            .items_center()
            .justify_center()
            .bg(ui::color(cx, ColorRef::Palette(ColorId::WindowBg)))
            .text_color(ui::color(cx, ColorRef::Palette(ColorId::WindowFg)))
            .child(self.notice.clone());
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(features::title_bar::title_bar(
                features::APP_NAME,
                window,
                cx,
            ))
            .child(notice)
    }
}

#[cfg(test)]
mod panic_tests {
    #[test]
    fn diagnostic_omits_panic_payload() {
        const CHILD: &str = "OXIGRAM_PANIC_HOOK_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            super::install_panic_hook();
            panic!("private fixture payload");
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "panic_tests::diagnostic_omits_panic_payload",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("OxiGram panicked at main.rs:"));
        assert!(!stderr.contains("private fixture payload"));
        assert!(!stderr.contains(env!("CARGO_MANIFEST_DIR")));
    }
}
