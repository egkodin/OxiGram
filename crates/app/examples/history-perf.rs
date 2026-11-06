//! Scrolls a 10,000-message history and reports frame intervals (the M3
//! budget), then checks that loading older messages does not move what is
//! on screen. Nothing goes online: TDLib never receives parameters.
//!
//! The window must stay visible and on top: macOS stops frame callbacks for
//! occluded windows. Run the release build, three times (docs/perf.md):
//!
//! cargo run --release -p oxigram --example history-perf -- [output directory]

#[path = "snapshots/chats.rs"]
mod chats;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use features::settings::FoldersPlacement;
use features::{Root, Store};
use gpui::{
    AnyWindowHandle, App, AppContext as _, AsyncApp, Bounds, Entity, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, Point, ScrollDelta, ScrollEvent,
    Window, WindowBounds, WindowKind, WindowOptions, point, px, size,
};
use model::chats::Change;
use model::history::Page;
use model::message::{Content, Message, ReplyTo, Sender};
use model::text::{Entity as TextEntity, EntityKind, FormattedText};
use model::{ChatId, Environment, Event, MessageId, Session, UserId};

/// The window and timing of the ADR 0002 spike.
const WIDTH: f32 = 1000.0;
const HEIGHT: f32 = 800.0;
const WARM_UP: Duration = Duration::from_secs(2);
const SCROLL: Duration = Duration::from_secs(60);
const SPEED: f32 = 3000.0;
/// Hiking Club in the chat list, and a point inside its history.
const GROUP: ChatId = ChatId(-100);
/// The chat list's right edge: 5/14 of the window.
const COLUMN_EDGE: f32 = WIDTH * 5.0 / 14.0;
const GROUP_ROW: (f32, f32) = (150.0, 366.0);
const HISTORY: (f32, f32) = (680.0, 400.0);
const MESSAGES: i32 = 10_000;
const OLDER_PAGE: i32 = 50;
/// The first id of the long history; the older page goes below it.
const FIRST_ID: i64 = 1_000;
const MINUTE: i32 = 60;
/// The time between two messages.
const STEP: i32 = 5 * MINUTE;
/// Long enough for opening the chat and loading pictures to settle.
const SETTLE: Duration = Duration::from_millis(1500);

const WORDS: &[&str] = &[
    "the",
    "trail",
    "map",
    "station",
    "weather",
    "tomorrow",
    "snacks",
    "photos",
    "meeting",
    "north",
    "loop",
    "great",
    "привет",
    "погода",
    "встреча",
    "завтра",
    "маршрут",
    "😀",
    "🏔️",
    "👍",
    "everyone",
    "saturday",
    "ride",
    "seats",
    "row",
    "gate",
    "doors",
    "open",
    "show",
];
const AUTHORS: &[i64] = &[chats::ME, 2, 3, 4];

/// A fixed-seed generator, so every run scrolls the same history.
struct Rng(u64);

impl Rng {
    fn below(&mut self, bound: usize) -> usize {
        // xorshift64
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % u64::try_from(bound).unwrap_or(1)).unwrap_or(0)
    }
}

/// 1 to 160 words with bold, italic, monospace and link spans.
fn text(rng: &mut Rng) -> FormattedText {
    let count = 1 + rng.below(160);
    let mut text = String::new();
    let mut entities = Vec::new();
    for index in 0..count {
        if index > 0 {
            text.push(' ');
        }
        let start = text.len();
        let kind = match rng.below(24) {
            0 => Some(EntityKind::Bold),
            1 => Some(EntityKind::Italic),
            2 => Some(EntityKind::Code),
            3 => {
                text.push_str("https://example.org/");
                Some(EntityKind::Url)
            }
            _ => None,
        };
        text.push_str(WORDS[rng.below(WORDS.len())]);
        if let Some(kind) = kind {
            entities.push(TextEntity {
                range: start..text.len(),
                kind,
            });
        }
    }
    FormattedText { text, entities }
}

/// Messages `ids`, five minutes apart and ending `ago` seconds back; runs by
/// one author, and every 15th replies to a recent one.
fn messages(ids: std::ops::Range<i64>, ago: i32, rng: &mut Rng) -> Vec<Message> {
    let count = i32::try_from(ids.end - ids.start).unwrap_or(i32::MAX);
    let start = chats::now() - ago - count * STEP;
    let mut from = AUTHORS[0];
    ids.zip(0..)
        .map(|(id, index): (i64, i32)| {
            if rng.below(3) == 0 {
                from = AUTHORS[rng.below(AUTHORS.len())];
            }
            let reply_to = (index % 15 == 14).then(|| ReplyTo {
                chat: GROUP,
                message: MessageId(id - 1 - i64::try_from(rng.below(10)).unwrap_or(0)),
                quote: None,
                external: None,
            });
            Message {
                is_outgoing: from == chats::ME,
                date: start + index * STEP,
                reply_to,
                ..Message::new(
                    MessageId(id),
                    GROUP,
                    Sender::User(UserId(from)),
                    Content::text(text(rng)),
                )
            }
        })
        .collect()
}

/// Draw-to-draw intervals while the history scrolls.
#[derive(Default)]
struct Meter {
    started: Option<Instant>,
    last: Option<Instant>,
    intervals: Vec<Duration>,
    scrolled: f32,
    is_done: bool,
}

/// One frame: scrolls by the time since the last one, towards older
/// messages, on the wheel path, and asks for the next frame.
fn tick(meter: Rc<RefCell<Meter>>, window: &mut Window, cx: &mut App) {
    let now = Instant::now();
    let delta = {
        let mut meter = meter.borrow_mut();
        let started = *meter.started.get_or_insert(now);
        if now - started >= SCROLL {
            meter.is_done = true;
            return;
        }
        let delta = meter.last.map_or(0.0, |last| {
            meter.intervals.push(now - last);
            SPEED * (now - last).as_secs_f32()
        });
        meter.last = Some(now);
        meter.scrolled += delta;
        delta
    };
    scroll(delta, window, cx);
    window.on_next_frame(move |window, cx| tick(meter, window, cx));
}

fn scroll(delta: f32, window: &mut Window, cx: &mut App) {
    window.dispatch_event(
        PlatformInput::Scroll(ScrollEvent {
            position: point(px(HISTORY.0), px(HISTORY.1)),
            delta: ScrollDelta::Pixels(point(px(0.0), px(delta))),
            ..Default::default()
        }),
        cx,
    );
}

fn percentile(sorted: &[Duration], part: f64) -> Duration {
    let last = sorted.len().saturating_sub(1);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "an index into at most a few thousand samples"
    )]
    let index = ((last as f64) * part).round() as usize;
    sorted.get(index).copied().unwrap_or_default()
}

fn report(meter: &Meter) {
    let mut sorted = meter.intervals.clone();
    sorted.sort_unstable();
    let p50 = percentile(&sorted, 0.5);
    // Late: longer than one and a half refresh periods, as in ADR 0002.
    let late = sorted
        .iter()
        .filter(|&&interval| interval * 2 > p50 * 3)
        .count();
    let ms = |duration: Duration| duration.as_secs_f64() * 1000.0;
    println!(
        "frames {} p50 {:.2} ms p99 {:.2} ms max {:.2} ms late {late} scrolled {:.0} px",
        sorted.len(),
        ms(p50),
        ms(percentile(&sorted, 0.99)),
        ms(sorted.last().copied().unwrap_or_default()),
        meter.scrolled,
    );
}

fn main() -> anyhow::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/history-perf"), PathBuf::from);
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
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let history = messages(FIRST_ID..FIRST_ID + i64::from(MESSAGES), 0, &mut rng);
    cx.update(|cx| features::settings::set_folders_placement(FoldersPlacement::Top, cx));
    let store = cx.new(|cx| {
        let mut store = Store::new(session.clone(), Environment::Production);
        for event in chats::events() {
            store.apply(event, cx);
        }
        // Read, so the chat opens at the newest message.
        store.apply(
            Event::Chats(Change::ReadInbox {
                chat: GROUP,
                last_read: MessageId(FIRST_ID + i64::from(MESSAGES)),
                unread_count: 0,
            }),
            cx,
        );
        store.apply(Event::Auth(model::auth::State::Ready), cx);
        store.open_chat(GROUP.into(), cx);
        store.add_history_page(GROUP.into(), Page::Newest, history, cx);
        store
    });
    let window = cx.update(|cx| open(store.clone(), cx))?;
    window.update(cx, |_, window, cx| {
        click(point(px(GROUP_ROW.0), px(GROUP_ROW.1)), window, cx);
    })?;
    cx.background_executor().timer(SETTLE + WARM_UP).await;

    let meter = Rc::new(RefCell::new(Meter::default()));
    let ticking = meter.clone();
    // Marks where to read `ZED_MEASUREMENTS=1`'s frame durations.
    eprintln!("scroll: start");
    window.update(cx, |_, window, cx| tick(ticking, window, cx))?;
    while !meter.borrow().is_done {
        cx.background_executor().timer(Duration::from_secs(1)).await;
    }
    eprintln!("scroll: end");
    report(&meter.borrow());

    let differing = check_older_page(&store, window, out, &mut rng, cx).await?;
    println!("history pixels changed by loading older messages: {differing}");
    window.update(cx, |_, window, _| window.remove_window())?;
    Ok(())
}

/// Scrolls near the top, adds an older page as a preload would, and counts
/// the pixels that changed in the history (the chat list has a spinner).
async fn check_older_page(
    store: &Entity<Store>,
    window: AnyWindowHandle,
    out: &Path,
    rng: &mut Rng,
    cx: &mut AsyncApp,
) -> anyhow::Result<usize> {
    // To the top, then partway down, so the anchor is inside an item.
    window.update(cx, |_, window, cx| scroll(1.0e9, window, cx))?;
    settle(window, cx).await?;
    window.update(cx, |_, window, cx| scroll(-300.0, window, cx))?;
    let before = save(window, &out.join("before.png"), cx).await?;
    // Two days before the history starts, so its first message keeps its
    // date badge and its group.
    let ago = MESSAGES * STEP + 2 * 24 * 60 * MINUTE;
    let older = messages(FIRST_ID - i64::from(OLDER_PAGE)..FIRST_ID, ago, rng);
    store.update(cx, |store, cx| {
        store.add_history_page(GROUP.into(), Page::Older(MessageId(FIRST_ID)), older, cx);
    });
    let after = save(window, &out.join("after.png"), cx).await?;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a pixel column of a window a few thousand pixels wide"
    )]
    let edge = (COLUMN_EDGE * before.width() as f32 / WIDTH).ceil() as u32;
    Ok(before
        .enumerate_pixels()
        .zip(after.pixels())
        .filter(|((x, _, a), b)| *x >= edge && a != b)
        .count())
}

async fn settle(window: AnyWindowHandle, cx: &mut AsyncApp) -> anyhow::Result<()> {
    window.update(cx, |_, window, cx| window.draw(cx).clear(cx))?;
    cx.background_executor().timer(SETTLE).await;
    window.update(cx, |_, window, cx| window.draw(cx).clear(cx))?;
    Ok(())
}

async fn save(
    window: AnyWindowHandle,
    path: &Path,
    cx: &mut AsyncApp,
) -> anyhow::Result<image::RgbaImage> {
    settle(window, cx).await?;
    let image = window.update(cx, |_, window, _| window.render_to_image())??;
    image
        .save(path)
        .with_context(|| format!("saving {}", path.display()))?;
    println!("{}", path.display());
    Ok(image)
}

fn click(position: Point<gpui::Pixels>, window: &mut Window, cx: &mut App) {
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
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: 1,
            first_mouse: false,
        }),
        cx,
    );
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: 1,
        }),
        cx,
    );
    // Leaves the pointer over the history, where the wheel events go.
    window.dispatch_event(
        PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(HISTORY.0), px(HISTORY.1)),
            pressed_button: None,
            modifiers,
        }),
        cx,
    );
}

fn open(store: Entity<Store>, cx: &mut App) -> anyhow::Result<AnyWindowHandle> {
    let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        // Above other windows, which would stop its frames.
        kind: WindowKind::PopUp,
        ..Default::default()
    };
    let window = cx.open_window(options, |window, cx| {
        let accounts = cx.new(|cx| features::Accounts::single(store, cx));
        cx.new(|cx| Root::new(accounts, window, cx))
    })?;
    cx.activate(true);
    Ok(window.into())
}
