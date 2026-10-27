//! The userpic soak: `SNAPSHOTS_SOAK=<seconds>` scrolls a chat list of
//! 2000 chats, each with its own downloaded userpic, up and down for that
//! long and prints the process's resident memory every 15 seconds. Four
//! times the userpic cache's capacity, so it evicts all the time; the
//! memory must stay flat once the first pass is done.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use gpui::{AppContext as _, AsyncApp, PlatformInput, ScrollDelta, ScrollEvent, point, px};
use model::chat::ChatKind;
use model::chats::Change;
use model::file::File;
use model::user::Photo;
use model::{BasicGroupId, Environment, Event, FileId, Session};

use features::Store;

use super::chats;

const CHATS: i64 = 2000;
const FIRST_ID: i64 = 10_000;
/// The chat list's middle, where the wheel turns.
const LIST: (f32, f32) = (150.0, 300.0);
/// About six rows a step, a brisk scroll.
const STEP: f32 = 400.0;
const FRAME: Duration = Duration::from_millis(16);
const SAMPLE_EVERY: Duration = Duration::from_secs(15);

/// A 160 px JPEG per chat, as TDLib downloads small userpics.
fn userpics() -> anyhow::Result<Vec<PathBuf>> {
    let folder = std::env::temp_dir().join("rgram-soak");
    std::fs::create_dir_all(&folder).context("creating the soak folder")?;
    (0..CHATS)
        .map(|index| {
            let path = folder.join(format!("{index}.jpg"));
            if !path.exists() {
                let shade = u8::try_from(index % 200).unwrap_or(0);
                image::RgbImage::from_fn(160, 160, |x, y| {
                    let stripe = u8::try_from((x + y) / 16 % 2 * 40).unwrap_or(0);
                    image::Rgb([shade + stripe, 120, 200 - shade / 2])
                })
                .save(&path)
                .with_context(|| format!("writing {}", path.display()))?;
            }
            Ok(path)
        })
        .collect()
}

fn events(paths: &[PathBuf]) -> Vec<Event> {
    let mut events = chats::events();
    for (index, path) in (0..).zip(paths) {
        let id = FIRST_ID + index;
        let file = FileId(i32::try_from(id).unwrap_or(i32::MAX));
        events.push(Event::Chats(Change::File(File {
            id: file,
            path: Some(path.to_string_lossy().into_owned()),
            is_downloading: false,
            downloaded: 0,
            size: 4000,
            uploaded: 4000,
            is_uploading: false,
        })));
        let mut chat = chats::chat(
            -id,
            ChatKind::Group {
                group: BasicGroupId(id),
            },
            &format!("Soak chat {index}"),
            CHATS - index,
            None,
        );
        chat.photo = Some(Photo {
            small: file,
            minithumbnail: None,
        });
        events.push(Event::Chats(Change::Chat(Box::new(chat))));
    }
    events
}

/// The resident set size in KB, as `ps` reports it.
fn rss_kb() -> anyhow::Result<u64> {
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .context("running ps")?;
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .context("reading ps's RSS")
}

pub(crate) async fn run(session: Session, seconds: u64, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let paths = userpics()?;
    let store = cx.new(|cx| {
        let mut store = Store::new(session, Environment::Production);
        for event in events(&paths) {
            store.apply(event, cx);
        }
        store.apply(super::scenarios::auth(model::auth::State::Ready), cx);
        store
    });
    let window = cx.update(|cx| super::open(store, cx))?;
    let start = Instant::now();
    let end = start + Duration::from_secs(seconds);
    let mut next_sample = start;
    let mut direction = -1.0;
    let mut travelled = 0.0;
    // Rows are 62 px high; turn at either end of the list.
    #[expect(clippy::cast_precision_loss, reason = "a small row count")]
    let length = CHATS as f32 * 62.0;
    println!("seconds,rss_mb");
    while Instant::now() < end {
        window.update(cx, |_, window, cx| {
            window.dispatch_event(
                PlatformInput::Scroll(ScrollEvent {
                    position: point(px(LIST.0), px(LIST.1)),
                    delta: ScrollDelta::Pixels(point(px(0.0), px(direction * STEP))),
                    ..ScrollEvent::default()
                }),
                cx,
            );
            window.draw(cx).clear(cx);
        })?;
        travelled += STEP;
        if travelled >= length {
            travelled = 0.0;
            direction = -direction;
        }
        if Instant::now() >= next_sample {
            #[expect(clippy::cast_precision_loss, reason = "megabytes for a table")]
            let megabytes = rss_kb()? as f64 / 1024.0;
            println!("{},{megabytes:.1}", start.elapsed().as_secs());
            next_sample += SAMPLE_EVERY;
        }
        cx.background_executor().timer(FRAME).await;
    }
    // The last frame, to check that the list scrolled with its userpics.
    let image = window.update(cx, |_, window, _| window.render_to_image())??;
    image
        .save("target/soak-last-frame.png")
        .context("saving the soak's last frame")?;
    window.update(cx, |_, window, _| window.remove_window())?;
    Ok(())
}
