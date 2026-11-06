//! The Lottie soak: `SNAPSHOTS_LOTTIE=<seconds>` opens a chat of 300
//! animated emoji in eight TGS files, scrolls it up and down for that long,
//! leaves it alone, then turns Power Usage's animations off and scrolls
//! again. Each phase prints the resident set, the physical footprint, the
//! CPU it used, the players alive and the frames shown, for docs/perf.md.
//! Hidden windows get no display-link frames, so it is drawn when a frame
//! changed; `SNAPSHOTS_ON_TOP=1` floats it over every window (it cannot be
//! hidden then) so the display link draws it as in the app.

use std::time::{Duration, Instant};

use anyhow::Context as _;
use gpui::{
    AnyWindowHandle, AppContext as _, AsyncApp, MouseButton, PlatformInput, ScrollDelta,
    ScrollEvent, point, px,
};
use model::file::{Dimensions, File};
use model::message::{Content, Message, Sender};
use model::sticker::{StickerKind, StickerPicture};
use model::{ChatId, ConnectionState, Environment, Event, FileId, MessageId, Session, UserId};

use features::Store;

use super::scenarios::MARIA_ROW;

const MARIA: i64 = 2;
const MESSAGES: i64 = 300;
/// The history's middle, where the wheel turns.
const HISTORY: (f32, f32) = (550.0, 300.0);
const STEP: f32 = 240.0;
const FRAME: Duration = Duration::from_millis(16);
const SAMPLE_EVERY: Duration = Duration::from_secs(10);
/// How long each quiet phase is watched.
const QUIET: Duration = Duration::from_secs(30);
/// tdesktop's `kAll`: every Power Usage switch off.
const ALL_SAVING: u32 = (1 << 11) - 1;

fn tgs(id: i32, variant: usize) -> File {
    File {
        id: FileId(id),
        path: super::lottie::tgs(variant).map(|path| path.to_string_lossy().into_owned()),
        is_downloading: false,
        downloaded: 0,
        size: 2_000,
        uploaded: 2_000,
        is_uploading: false,
        prefix: model::file::Prefix::default(),
    }
}

fn messages() -> Vec<Message> {
    (1..=MESSAGES)
        .map(|id| {
            let variant = usize::try_from(id).unwrap_or(0) % 8;
            let picture = |file| StickerPicture {
                kind: StickerKind::Animated,
                dimensions: Dimensions::new(512, 512),
                file,
                still: None,
                premium_effect: None,
                skin_tone: None,
            };
            let file_id = i32::try_from(50_000 + variant).unwrap_or(50_000);
            let content = Content::Emoji {
                emoji: "😀".to_owned(),
                sticker: Some(Box::new(picture(tgs(file_id, variant)))),
            };
            let from = if id % 3 == 0 { 1 } else { MARIA };
            Message {
                is_outgoing: from == 1,
                ..Message::new(
                    MessageId(id << 20),
                    ChatId(MARIA),
                    Sender::User(UserId(from)),
                    content,
                )
            }
        })
        .collect()
}

/// What the process holds and has used so far.
struct Sample {
    rss_mb: f64,
    footprint_mb: f64,
    cpu_seconds: f64,
}

fn sample() -> anyhow::Result<Sample> {
    let pid = std::process::id().to_string();
    let ps = std::process::Command::new("ps")
        .args(["-o", "rss=,time=", "-p", &pid])
        .output()
        .context("running ps")?;
    let ps = String::from_utf8_lossy(&ps.stdout);
    let mut fields = ps.split_whitespace();
    let rss_kb: f64 = fields
        .next()
        .context("ps printed no RSS")?
        .parse()
        .context("reading ps's RSS")?;
    let cpu_seconds = cpu_time(fields.next().context("ps printed no CPU time")?)?;
    let vmmap = std::process::Command::new("vmmap")
        .args(["--summary", &pid])
        .output()
        .context("running vmmap")?;
    let footprint_mb = String::from_utf8_lossy(&vmmap.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("Physical footprint:"))
        .map(str::trim)
        .and_then(megabytes)
        .context("vmmap printed no footprint")?;
    Ok(Sample {
        rss_mb: rss_kb / 1024.0,
        footprint_mb,
        cpu_seconds,
    })
}

/// `ps`'s `[hh:]mm:ss.cc` as seconds.
fn cpu_time(text: &str) -> anyhow::Result<f64> {
    text.split(':').try_fold(0.0, |total: f64, part| {
        Ok(total * 60.0 + part.parse::<f64>().context("reading ps's CPU time")?)
    })
}

/// vmmap's "85.3M" or "1.2G" as megabytes.
fn megabytes(text: &str) -> Option<f64> {
    let (number, unit) = text.split_at(text.len().checked_sub(1)?);
    let number: f64 = number.parse().ok()?;
    match unit {
        "K" => Some(number / 1024.0),
        "M" => Some(number),
        "G" => Some(number * 1024.0),
        _ => None,
    }
}

fn scroll(window: AnyWindowHandle, by: f32, cx: &mut AsyncApp) -> anyhow::Result<()> {
    window.update(cx, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::Scroll(ScrollEvent {
                position: point(px(HISTORY.0), px(HISTORY.1)),
                delta: ScrollDelta::Pixels(point(px(0.0), px(by))),
                ..ScrollEvent::default()
            }),
            cx,
        );
    })
}

/// Draws when a Lottie frame changed since the last draw, as the display
/// link draws a window only when it was notified.
struct Drawer {
    window: AnyWindowHandle,
    shown: u64,
    draws: u64,
}

impl Drawer {
    fn draw_if_changed(&mut self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        let shown = cx.update(|cx| features::lottie_stats(cx).frames_shown);
        if shown != self.shown && !self.is_visible(cx)? {
            self.shown = shown;
            self.draw(cx)?;
        }
        Ok(())
    }

    fn is_visible(&self, cx: &mut AsyncApp) -> anyhow::Result<bool> {
        self.window.update(cx, |_, window, _| window.is_visible())
    }

    fn draw(&mut self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        self.draws += 1;
        self.window
            .update(cx, |_, window, cx| window.draw(cx).clear(cx))
    }
}

/// One phase's figures, from its start.
struct Phase {
    name: &'static str,
    start: Instant,
    before: Sample,
    shown: u64,
    draws: u64,
}

impl Phase {
    fn start(name: &'static str, drawer: &Drawer, cx: &mut AsyncApp) -> anyhow::Result<Self> {
        Ok(Self {
            name,
            start: Instant::now(),
            before: sample()?,
            shown: cx.update(|cx| features::lottie_stats(cx).frames_shown),
            draws: drawer.draws,
        })
    }

    fn report(&self, drawer: &Drawer, cx: &mut AsyncApp) -> anyhow::Result<()> {
        let after = sample()?;
        let seconds = self.start.elapsed().as_secs_f64();
        let cpu = (after.cpu_seconds - self.before.cpu_seconds) / seconds * 100.0;
        let stats = cx.update(|cx| features::lottie_stats(cx));
        #[expect(clippy::cast_precision_loss, reason = "rates for a table")]
        let (frames, draws, cached) = (
            (stats.frames_shown - self.shown) as f64 / seconds,
            (drawer.draws - self.draws) as f64 / seconds,
            stats.cached_bytes as f64 / 1_048_576.0,
        );
        let drawn = if drawer.is_visible(cx)? {
            "display link"
        } else {
            "by hand"
        };
        println!(
            "{},{seconds:.0},{:.1},{:.1},{cpu:.1},{},{cached:.1},{frames:.0},{draws:.0},{drawn}",
            self.name, after.rss_mb, after.footprint_mb, stats.players,
        );
        Ok(())
    }
}

/// Leaves the chat alone for `QUIET`, drawing what changes.
async fn quiet(name: &'static str, drawer: &mut Drawer, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let phase = Phase::start(name, drawer, cx)?;
    while phase.start.elapsed() < QUIET {
        drawer.draw_if_changed(cx)?;
        cx.background_executor().timer(FRAME).await;
    }
    phase.report(drawer, cx)
}

/// Scrolls up and down for `length`, a step a frame.
async fn scrolling(
    name: &'static str,
    length: Duration,
    drawer: &mut Drawer,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let mut phase = Phase::start(name, drawer, cx)?;
    let end = Instant::now() + length;
    let mut direction = 1.0;
    let mut travelled = 0.0;
    // About 130 px a message; turn at either end.
    #[expect(clippy::cast_precision_loss, reason = "a small message count")]
    let span = MESSAGES as f32 * 130.0;
    while Instant::now() < end {
        scroll(drawer.window, direction * STEP, cx)?;
        if !drawer.is_visible(cx)? {
            drawer.draw(cx)?;
        }
        travelled += STEP;
        if travelled >= span {
            travelled = 0.0;
            direction = -direction;
        }
        if phase.start.elapsed() >= SAMPLE_EVERY {
            phase.report(drawer, cx)?;
            phase = Phase::start(name, drawer, cx)?;
        }
        cx.background_executor().timer(FRAME).await;
    }
    phase.report(drawer, cx)
}

pub(crate) async fn run(session: Session, seconds: u64, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let store = cx.new(|cx| {
        let mut store = Store::new(session, Environment::Production);
        // Without the typing one: its dots would redraw every frame.
        let events = super::chats::events()
            .into_iter()
            .filter(|event| !matches!(event, Event::Chats(model::chats::Change::Action { .. })));
        for event in events {
            store.apply(event, cx);
        }
        store.apply(super::scenarios::auth(model::auth::State::Ready), cx);
        // Online, so the connecting spinner does not redraw the window.
        store.apply(Event::Connection(ConnectionState::Ready), cx);
        let chat = ChatId(MARIA);
        store.open_chat(chat.into(), cx);
        // SNAPSHOTS_LOTTIE_EMPTY=1: the same window without emoji, the baseline.
        let messages = if std::env::var_os("SNAPSHOTS_LOTTIE_EMPTY").is_some() {
            Vec::new()
        } else {
            messages()
        };
        store.add_history_page(chat.into(), model::history::Page::Newest, messages, cx);
        store
    });
    let window = cx.update(|cx| {
        // The chat list's rows where the snapshots click them.
        features::settings::set_folders_placement(features::settings::FoldersPlacement::Top, cx);
        let accounts = cx.new(|cx| features::Accounts::single(store, cx));
        // Floating over every window only when asked: it cannot be hidden.
        if std::env::var_os("SNAPSHOTS_ON_TOP").is_some() {
            super::open_on_top(accounts, cx)
        } else {
            super::open(accounts, cx)
        }
    })?;
    window.update(cx, |_, window, cx| {
        super::click(
            MouseButton::Left,
            point(px(MARIA_ROW.0), px(MARIA_ROW.1)),
            window,
            cx,
        );
        window.draw(cx).clear(cx);
    })?;
    println!(
        "phase,seconds,rss_mb,footprint_mb,cpu_percent,players,cached_mb,frames_per_s,draws_per_s"
    );
    let mut drawer = Drawer {
        window,
        shown: 0,
        draws: 0,
    };
    drawer.draw(cx)?;
    quiet("opened", &mut drawer, cx).await?;
    scrolling("scrolling", Duration::from_secs(seconds), &mut drawer, cx).await?;
    quiet("quiet", &mut drawer, cx).await?;
    cx.update(|cx| features::settings::preset_power_saving(ALL_SAVING, cx));
    scrolling("saving-scrolling", SAMPLE_EVERY, &mut drawer, cx).await?;
    quiet("saving-quiet", &mut drawer, cx).await?;
    let image = window.update(cx, |_, window, _| window.render_to_image())??;
    image
        .save("target/lottie-soak-last-frame.png")
        .context("saving the soak's last frame")?;
    window.update(cx, |_, window, _| window.remove_window())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn ps_and_vmmap_figures_parse() {
        assert!((super::cpu_time("1:02.50").expect("mm:ss") - 62.5).abs() < 1e-9);
        assert!((super::cpu_time("1:00:01.00").expect("hh:mm:ss") - 3601.0).abs() < 1e-9);
        assert_eq!(super::megabytes("85.5M"), Some(85.5));
        assert_eq!(super::megabytes("1.5G"), Some(1536.0));
        assert_eq!(super::megabytes("nope"), None);
    }
}
