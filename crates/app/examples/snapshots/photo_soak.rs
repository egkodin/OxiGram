//! The photo soak: `SNAPSHOTS_PHOTOS=<seconds>` opens a chat of 200 photos,
//! each its own 1280×960 JPEG as TDLib downloads them, scrolls it up and
//! down for that long and prints the resident set and the physical
//! footprint every 10 seconds, for docs/perf.md. A decoded photo is 4.9 MB,
//! so what the history keeps of the photos scrolled past shows at once.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use gpui::{AppContext as _, AsyncApp, MouseButton, point, px};
use model::file::{Dimensions, File};
use model::message::{Content, Media, Message, Sender};
use model::{ChatId, ConnectionState, Environment, Event, FileId, MessageId, Session, UserId};

use features::Store;

use super::lottie_soak::{sample, scroll};
use super::scenarios::MARIA_ROW;

const MARIA: i64 = 2;
const PHOTOS: i32 = 200;
const SIZE: (u32, u32) = (1280, 960);
const STEP: f32 = 120.0;
const FRAME: Duration = Duration::from_millis(16);
const SAMPLE_EVERY: Duration = Duration::from_secs(10);

/// A JPEG per message, each with its own pattern.
fn photos() -> anyhow::Result<Vec<PathBuf>> {
    let folder = std::env::temp_dir().join("rgram-photo-soak");
    std::fs::create_dir_all(&folder).context("creating the photo soak folder")?;
    (0..PHOTOS)
        .map(|index| {
            let path = folder.join(format!("{index}.jpg"));
            if !path.exists() {
                let shade = u8::try_from(index % 200).unwrap_or(0);
                image::RgbImage::from_fn(SIZE.0, SIZE.1, |x, y| {
                    let stripe = u8::try_from((x + y) / 64 % 2 * 40).unwrap_or(0);
                    image::Rgb([shade + stripe, 120, 200 - shade / 2])
                })
                .save(&path)
                .with_context(|| format!("writing {}", path.display()))?;
            }
            Ok(path)
        })
        .collect()
}

fn messages(paths: &[PathBuf]) -> Vec<Message> {
    (1..)
        .zip(paths)
        .map(|(id, path)| {
            let file = File {
                id: FileId(60_000 + id),
                path: Some(path.to_string_lossy().into_owned()),
                is_downloading: false,
                downloaded: 0,
                size: 100_000,
                uploaded: 100_000,
                is_uploading: false,
                prefix: model::file::Prefix::default(),
            };
            let media = Media {
                dimensions: Dimensions::new(1280, 960),
                file: Some(file),
                ..Media::default()
            };
            Message::new(
                MessageId(i64::from(id) << 20),
                ChatId(MARIA),
                Sender::User(UserId(MARIA)),
                Content::Photo(media),
            )
        })
        .collect()
}

pub(crate) async fn run(session: Session, seconds: u64, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let paths = photos()?;
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
        store.apply(Event::Connection(ConnectionState::Ready), cx);
        let chat = ChatId(MARIA);
        store.open_chat(chat.into(), cx);
        store.add_history_page(
            chat.into(),
            model::history::Page::Newest,
            messages(&paths),
            cx,
        );
        store
    });
    let window = cx.update(|cx| {
        // The chat list's rows where the snapshots click them.
        features::settings::set_folders_placement(features::settings::FoldersPlacement::Top, cx);
        let accounts = cx.new(|cx| features::Accounts::single(store, cx));
        super::open(accounts, cx)
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
    let start = Instant::now();
    let end = start + Duration::from_secs(seconds);
    let mut next_sample = start;
    let mut direction = 1.0;
    let mut travelled = 0.0;
    // About 330 px a photo; turn at either end.
    #[expect(clippy::cast_precision_loss, reason = "a small photo count")]
    let span = PHOTOS as f32 * 330.0;
    println!("seconds,rss_mb,footprint_mb");
    while Instant::now() < end {
        scroll(window, direction * STEP, cx)?;
        window.update(cx, |_, window, cx| window.draw(cx).clear(cx))?;
        travelled += STEP;
        if travelled >= span {
            travelled = 0.0;
            direction = -direction;
        }
        if Instant::now() >= next_sample {
            let now = sample()?;
            println!(
                "{},{:.1},{:.1}",
                start.elapsed().as_secs(),
                now.rss_mb,
                now.footprint_mb
            );
            next_sample += SAMPLE_EVERY;
        }
        cx.background_executor().timer(FRAME).await;
    }
    // The last frame, to check that the history scrolled with its photos.
    let image = window.update(cx, |_, window, _| window.render_to_image())??;
    image
        .save("target/photo-soak-last-frame.png")
        .context("saving the photo soak's last frame")?;
    window.update(cx, |_, window, _| window.remove_window())?;
    Ok(())
}
