//! Renders the sign-in screens to PNG files for comparison with Telegram
//! Desktop, without signing in: each window gets preset model events, and
//! TDLib never receives parameters, so nothing goes online.
//!
//! cargo run -p rgram --example snapshots -- [output directory]

mod chats;
mod history;
mod media;
mod scenarios;
mod soak;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context as _;
use features::settings::FoldersPlacement;
use features::{Root, Store};
use gpui::{
    AnyWindowHandle, App, AppContext as _, AsyncApp, Bounds, Entity, ExternalPaths, FileDropEvent,
    InputEvent as _, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, PlatformInput, Point, ScrollDelta, ScrollEvent, Window, WindowBounds,
    WindowOptions, point, px, size,
};
use model::{Environment, Event, Session};

/// Long enough for the intro's slide and cover animations to finish.
const SETTLE: Duration = Duration::from_millis(1500);
/// Past the floating date's fade-in, before it hides.
const SCROLL_SETTLE: Duration = Duration::from_millis(500);
const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 600.0;

/// A point in the window, in logical pixels.
type WindowPoint = (f32, f32);

/// One window: the events it receives, each step saved under its name.
struct Scenario {
    /// Applied before the first step.
    setup: Vec<Event>,
    steps: Vec<(&'static str, Event)>,
    /// Clicks at window points after the steps, each saved.
    clicks: Vec<(&'static str, MouseButton, (f32, f32))>,
    /// Then drags from one point to another, saved before the button is let
    /// go; the release drops on the target.
    drags: Vec<(&'static str, WindowPoint, WindowPoint)>,
    /// Then files dropped on a point, saved while they hover and again
    /// once dropped.
    drops: Vec<(&'static str, WindowPoint, Vec<PathBuf>)>,
    /// Then wheel scrolls at a point by pixels, up when positive; saved
    /// before the floating date hides.
    scrolls: Vec<(&'static str, WindowPoint, f32)>,
    /// Then keys pressed and text typed into what has focus, each run saved.
    keys: Vec<(&'static str, Vec<Input>)>,
    /// Set before the window opens; it stays for later windows.
    folders: FoldersPlacement,
    /// Loaded into open chats before the window opens, as TDLib's pages.
    histories: Vec<(model::ChatId, Vec<model::message::Message>)>,
}

/// Keyboard input for a step.
enum Input {
    /// Each character as a key press, as a keyboard types it.
    Type(&'static str),
    /// One key press, such as `shift-enter`.
    Key(&'static str),
}

fn main() -> anyhow::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/snapshots"), PathBuf::from);
    std::fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
    tg::execute(&td_api::functions::SetLogVerbosityLevel {
        new_verbosity_level: 1,
    })
    .context("setting TDLib's log verbosity")?;
    let runtime = tg::Runtime::start().context("starting TDLib")?;
    let (client, updates) = runtime.create_client();
    gpui_platform::application().run(move |cx: &mut App| {
        // Kept alive so the idle client's updates have somewhere to go.
        let _updates = updates;
        if let Err(error) = ui::init(cx) {
            eprintln!("{error:#}");
            cx.quit();
            return;
        }
        features::init(cx);
        features::preset_voice_samples(VOICE_RATE, voice_samples(), cx);
        let session = Session::new(client);
        cx.spawn(async move |cx| {
            if let Err(error) = run(session, &out, cx).await {
                eprintln!("{error:#}");
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
    drop(runtime);
    Ok(())
}

async fn run(session: Session, out: &Path, cx: &mut AsyncApp) -> anyhow::Result<()> {
    if let Some(seconds) = std::env::var("SNAPSHOTS_SOAK")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        return soak::run(session, seconds, cx).await;
    }
    // SNAPSHOTS_ONLY=41,77 runs the scenarios whose first step starts so.
    let only = std::env::var("SNAPSHOTS_ONLY").unwrap_or_default();
    let only: Vec<&str> = only.split(',').filter(|part| !part.is_empty()).collect();
    // The night scenario goes last of all: its night mode and narrow sidebar stay.
    let mut all = scenarios::scenarios();
    let night = all.pop();
    all.extend(media::scenarios());
    all.extend(night);
    for scenario in all {
        let first = scenario.steps.first().map_or("", |(name, _)| *name);
        if !only.is_empty() && !only.iter().any(|prefix| first.starts_with(prefix)) {
            continue;
        }
        let mut steps = scenario.steps.into_iter();
        let setup = scenario.setup;
        let histories = scenario.histories;
        let Some((first_name, first)) = steps.next() else {
            continue;
        };
        let folders = scenario.folders;
        cx.update(|cx| {
            features::settings::set_folders_placement(folders, cx);
            // A scenario that switches it must not change the next one's Enter.
            features::settings::set_send_by(features::settings::SendBy::Enter, cx);
        });
        let store = cx.new(|cx| {
            let mut store = Store::new(session.clone(), Environment::Production);
            for event in setup {
                store.apply(event, cx);
            }
            store.apply(first, cx);
            for (chat, messages) in histories {
                store.open_chat(chat.into(), cx);
                store.add_history_page(chat.into(), model::history::Page::Newest, messages, cx);
            }
            for event in history::pins() {
                store.apply(event, cx);
            }
            store
        });
        let window = cx.update(|cx| open(store.clone(), cx))?;
        settle_and_save(window, first_name, out, SETTLE, cx).await?;
        for (name, event) in steps {
            store.update(cx, |store, cx| store.apply(event, cx));
            settle_and_save(window, name, out, SETTLE, cx).await?;
        }
        for (name, button, (x, y)) in scenario.clicks {
            window.update(cx, |_, window, cx| {
                click(button, point(px(x), px(y)), window, cx);
            })?;
            settle_and_save(window, name, out, SETTLE, cx).await?;
        }
        for (name, (from_x, from_y), (to_x, to_y)) in scenario.drags {
            let (from, to) = (point(px(from_x), px(from_y)), point(px(to_x), px(to_y)));
            window.update(cx, |_, window, cx| drag(from, to, window, cx))?;
            settle_and_save(window, name, out, SETTLE, cx).await?;
            window.update(cx, |_, window, cx| release(to, window, cx))?;
        }
        for (name, (x, y), paths) in scenario.drops {
            let position = point(px(x), px(y));
            window.update(cx, |_, window, cx| hover_files(position, paths, window, cx))?;
            settle_and_save(window, &format!("{name}-over"), out, SETTLE, cx).await?;
            window.update(cx, |_, window, cx| {
                window.dispatch_event(FileDropEvent::Submit { position }.to_platform_input(), cx);
            })?;
            settle_and_save(window, name, out, SETTLE, cx).await?;
        }
        for (name, (x, y), by) in scenario.scrolls {
            window.update(cx, |_, window, cx| {
                window.dispatch_event(
                    PlatformInput::Scroll(ScrollEvent {
                        position: point(px(x), px(y)),
                        delta: ScrollDelta::Pixels(point(px(0.0), px(by))),
                        ..ScrollEvent::default()
                    }),
                    cx,
                );
            })?;
            settle_and_save(window, name, out, SCROLL_SETTLE, cx).await?;
        }
        for (name, inputs) in scenario.keys {
            window.update(cx, |_, window, cx| {
                inputs.iter().try_for_each(|input| press(input, window, cx))
            })??;
            settle_and_save(window, name, out, SETTLE, cx).await?;
        }
        window.update(cx, |_, window, _| window.remove_window())?;
    }
    Ok(())
}

fn press(input: &Input, window: &mut Window, cx: &mut App) -> anyhow::Result<()> {
    match input {
        Input::Type(text) => {
            for c in text.chars() {
                let key = if c == ' ' {
                    "space".to_owned()
                } else {
                    c.to_lowercase().collect()
                };
                window.dispatch_keystroke(
                    Keystroke {
                        modifiers: Modifiers::default(),
                        key,
                        key_char: Some(c.to_string()),
                    },
                    cx,
                );
                window.draw(cx).clear(cx);
            }
        }
        Input::Key(spec) => {
            let keystroke = Keystroke::parse(spec).with_context(|| format!("key {spec}"))?;
            window.dispatch_keystroke(keystroke, cx);
            window.draw(cx).clear(cx);
        }
    }
    Ok(())
}

fn click(button: MouseButton, position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    let modifiers = Modifiers::default();
    window.dispatch_event(
        PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers,
        }),
        cx,
    );
    window.draw(cx).clear(cx);
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            button,
            position,
            modifiers,
            click_count: 1,
            first_mouse: false,
        }),
        cx,
    );
    // Text links listen for the release only from the frame after the press.
    window.draw(cx).clear(cx);
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            button,
            position,
            modifiers,
            click_count: 1,
        }),
        cx,
    );
}

/// Presses at `from` and moves to `to` in steps, past gpui's drag threshold.
fn drag(from: Point<Pixels>, to: Point<Pixels>, window: &mut Window, cx: &mut App) {
    const STEPS: u16 = 8;
    let modifiers = Modifiers::default();
    window.dispatch_event(
        PlatformInput::MouseMove(MouseMoveEvent {
            position: from,
            pressed_button: None,
            modifiers,
        }),
        cx,
    );
    window.draw(cx).clear(cx);
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: from,
            modifiers,
            click_count: 1,
            first_mouse: false,
        }),
        cx,
    );
    for step in 1..=STEPS {
        let part = f32::from(step) / f32::from(STEPS);
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: from + (to - from) * part,
                pressed_button: Some(MouseButton::Left),
                modifiers,
            }),
            cx,
        );
        window.draw(cx).clear(cx);
    }
}

/// Files from the system entering the window and moving to `position`.
fn hover_files(position: Point<Pixels>, paths: Vec<PathBuf>, window: &mut Window, cx: &mut App) {
    let paths = ExternalPaths(paths.into());
    window.dispatch_event(
        FileDropEvent::Entered { position, paths }.to_platform_input(),
        cx,
    );
    window.draw(cx).clear(cx);
    window.dispatch_event(FileDropEvent::Pending { position }.to_platform_input(), cx);
}

fn release(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
        }),
        cx,
    );
}

const VOICE_RATE: u32 = 48_000;

/// Three seconds of a tone that swells and fades, recorded in place of the
/// microphone.
fn voice_samples() -> std::sync::Arc<[f32]> {
    (0..VOICE_RATE * 3)
        .map(|index| {
            #[expect(clippy::cast_precision_loss, reason = "a test tone")]
            let time = index as f32 / VOICE_RATE as f32;
            let swell = (time * std::f32::consts::PI / 3.0).sin();
            (time * 220.0 * std::f32::consts::TAU).sin() * swell * 0.6
        })
        .collect()
}

pub(crate) fn open(store: Entity<Store>, cx: &mut App) -> anyhow::Result<AnyWindowHandle> {
    let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };
    let window = cx.open_window(options, |window, cx| {
        cx.new(|cx| Root::new(store, window, cx))
    })?;
    Ok(window.into())
}

async fn settle_and_save(
    window: AnyWindowHandle,
    name: &str,
    out: &Path,
    settle: Duration,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    // A window that is not on screen gets no display-link frames, so draw by
    // hand: once to start the transitions, once after they settled.
    window.update(cx, |_, window, cx| window.draw(cx).clear(cx))?;
    cx.background_executor().timer(settle).await;
    window.update(cx, |_, window, cx| window.draw(cx).clear(cx))?;
    let image = window.update(cx, |_, window, _| window.render_to_image())??;
    let path = out.join(format!("{name}.png"));
    image
        .save(&path)
        .with_context(|| format!("saving {}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}
