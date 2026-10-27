//! Photos cut to a circle at the exact device size, as tdesktop prepares
//! userpics (`Image::pixCircled`): gpui scales images on the GPU without
//! mipmaps, which aliases a 160 px photo drawn at 46, and clips children
//! only to rectangles.
//!
//! Decoding runs on the background executor. The cache keeps the most
//! recently drawn images and frees the GPU textures of those it evicts; a
//! file that fails to decode is tried again when it changes on disk or
//! after a back-off.

use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use gpui::{App, Global, RenderImage};
use image::{Frame, RgbaImage, imageops::FilterType};

/// Prepared images kept: a few screens of chat list and history userpics.
const CAPACITY: usize = 512;
/// The wait before decoding a failed file again, doubling up to the cap.
const FIRST_RETRY: Duration = Duration::from_secs(2);
const MAX_RETRY: Duration = Duration::from_secs(60);

type Key = (PathBuf, u32);

/// A file's modification time and length, which change when it is
/// rewritten.
type Stamp = Option<(SystemTime, u64)>;

enum Slot {
    /// `wait` is the back-off should this decode fail.
    Loading {
        wait: Duration,
    },
    Ready(Arc<RenderImage>),
    Failed {
        retry_at: Instant,
        /// The back-off after the next failure.
        next_wait: Duration,
        stamp: Stamp,
    },
}

struct Entry {
    slot: Slot,
    /// Interior, so a cache hit while drawing needs no `&mut` global.
    last_used: Cell<u64>,
}

/// What a lookup found.
enum Lookup {
    Ready(Arc<RenderImage>),
    /// Loading, or failed and not due again.
    Wait,
    /// Nothing usable yet: start a decode with this back-off.
    Start {
        wait: Duration,
    },
}

/// A decode's result, with the file's stamp from before it was read.
struct Loaded {
    image: Option<Arc<RenderImage>>,
    stamp: Stamp,
}

struct RoundImages {
    entries: HashMap<Key, Entry>,
    clock: Cell<u64>,
    capacity: usize,
}

impl Default for RoundImages {
    fn default() -> Self {
        Self::with_capacity(CAPACITY)
    }
}

impl Global for RoundImages {}

impl RoundImages {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            clock: Cell::new(0),
            capacity,
        }
    }

    fn tick(&self) -> u64 {
        let now = self.clock.get() + 1;
        self.clock.set(now);
        now
    }

    fn lookup(&self, key: &Key, now: Instant) -> Lookup {
        let Some(entry) = self.entries.get(key) else {
            return Lookup::Start { wait: FIRST_RETRY };
        };
        entry.last_used.set(self.tick());
        match &entry.slot {
            Slot::Loading { .. } => Lookup::Wait,
            Slot::Ready(image) => Lookup::Ready(image.clone()),
            Slot::Failed {
                retry_at,
                next_wait,
                stamp,
            } => {
                // Only failed files are checked on disk, so drawing does no I/O.
                if now >= *retry_at || file_stamp(&key.0) != *stamp {
                    Lookup::Start { wait: *next_wait }
                } else {
                    Lookup::Wait
                }
            }
        }
    }

    fn start(&mut self, key: Key, wait: Duration) {
        let entry = Entry {
            slot: Slot::Loading { wait },
            last_used: Cell::new(self.tick()),
        };
        self.entries.insert(key, entry);
    }

    /// Stores a decode's result and evicts the least recently drawn images
    /// over capacity, handing each to `free` for its GPU texture.
    fn finish(
        &mut self,
        key: Key,
        loaded: Loaded,
        now: Instant,
        mut free: impl FnMut(Arc<RenderImage>),
    ) {
        let Some(entry) = self.entries.get_mut(&key) else {
            // Evicted while it loaded; the image was never drawn.
            return;
        };
        let wait = match entry.slot {
            Slot::Loading { wait } => wait,
            _ => FIRST_RETRY,
        };
        entry.slot = match loaded.image {
            Some(image) => Slot::Ready(image),
            None => Slot::Failed {
                retry_at: now + wait,
                next_wait: (wait * 2).min(MAX_RETRY),
                stamp: loaded.stamp,
            },
        };
        while self.entries.len() > self.capacity {
            // Loading entries stay, so a decode in flight is never repeated.
            let oldest = self
                .entries
                .iter()
                .filter(|(_, entry)| !matches!(entry.slot, Slot::Loading { .. }))
                .min_by_key(|(_, entry)| entry.last_used.get())
                .map(|(key, _)| key.clone());
            let Some(oldest) = oldest else {
                break;
            };
            if let Some(Entry {
                slot: Slot::Ready(image),
                ..
            }) = self.entries.remove(&oldest)
            {
                free(image);
            }
        }
    }
}

/// The image at `path` filled into a `side` px square and cut to a
/// circle; `None` while it decodes in the background or after it failed.
pub(crate) fn round_file(path: &Path, side: u32, cx: &mut App) -> Option<Arc<RenderImage>> {
    let key = (path.to_path_buf(), side);
    let found = cx
        .try_global::<RoundImages>()
        .map_or(Lookup::Start { wait: FIRST_RETRY }, |cache| {
            cache.lookup(&key, Instant::now())
        });
    let wait = match found {
        Lookup::Ready(image) => return Some(image),
        Lookup::Wait => return None,
        Lookup::Start { wait } => wait,
    };
    cx.default_global::<RoundImages>().start(key.clone(), wait);
    let file = key.0.clone();
    let decode = cx
        .background_executor()
        .spawn(async move { load(&file, side) });
    cx.spawn(async move |cx| {
        let loaded = decode.await;
        cx.update(|cx| {
            let mut freed = Vec::new();
            cx.default_global::<RoundImages>()
                .finish(key, loaded, Instant::now(), |image| freed.push(image));
            for image in freed {
                cx.drop_image(image, None);
            }
            cx.refresh_windows();
        });
    })
    .detach();
    None
}

fn file_stamp(path: &Path) -> Stamp {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

fn load(path: &Path, side: u32) -> Loaded {
    let stamp = file_stamp(path);
    let image = match image::open(path) {
        Ok(decoded) => {
            let filled = decoded
                .resize_to_fill(side, side, FilterType::Lanczos3)
                .to_rgba8();
            Some(Arc::new(RenderImage::new([Frame::new(circled(&filled))])))
        }
        Err(error) => {
            tracing::warn!(%error, "cannot decode a userpic");
            None
        }
    };
    Loaded { image, stamp }
}

/// BGRA with straight alpha, as gpui's sprites expect, and an antialiased
/// circle: each pixel keeps the share of it inside the circle.
fn circled(square: &RgbaImage) -> RgbaImage {
    #[expect(clippy::cast_precision_loss, reason = "userpics are small")]
    let radius = square.width().min(square.height()) as f32 / 2.0;
    let mut out = RgbaImage::new(square.width(), square.height());
    for (x, y, pixel) in square.enumerate_pixels() {
        #[expect(clippy::cast_precision_loss, reason = "userpics are small")]
        let (dx, dy) = (x as f32 + 0.5 - radius, y as f32 + 0.5 - radius);
        let coverage = (radius - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
        let [r, g, b, a] = pixel.0;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "coverage is within 0..=1"
        )]
        let alpha = (f32::from(a) * coverage).round() as u8;
        out.put_pixel(x, y, image::Rgba([b, g, r, alpha]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> Arc<RenderImage> {
        Arc::new(RenderImage::new([Frame::new(RgbaImage::new(1, 1))]))
    }

    fn key(name: &str) -> Key {
        (PathBuf::from(name), 46)
    }

    fn ready(image: &Arc<RenderImage>) -> Loaded {
        Loaded {
            image: Some(image.clone()),
            stamp: None,
        }
    }

    fn failed(stamp: Stamp) -> Loaded {
        Loaded { image: None, stamp }
    }

    /// A file of its own in the temp folder with `bytes`.
    fn scratch_file(name: &str, bytes: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("rgram-round-{}-{name}", std::process::id()));
        std::fs::write(&path, bytes).expect("the temp folder is writable");
        path
    }

    #[test]
    fn evicts_the_least_recently_drawn_and_frees_its_texture() {
        let mut cache = RoundImages::with_capacity(2);
        let now = Instant::now();
        let (a, b, c) = (image(), image(), image());
        for (name, image) in [("a", &a), ("b", &b)] {
            cache.start(key(name), FIRST_RETRY);
            cache.finish(key(name), ready(image), now, |_| panic!("under capacity"));
        }
        // Drawing `a` again leaves `b` the oldest.
        assert!(matches!(cache.lookup(&key("a"), now), Lookup::Ready(_)));
        let mut freed = Vec::new();
        cache.start(key("c"), FIRST_RETRY);
        cache.finish(key("c"), ready(&c), now, |image| freed.push(image));
        assert_eq!(freed.len(), 1);
        assert!(Arc::ptr_eq(&freed[0], &b), "the freed image is b's");
        assert!(matches!(cache.lookup(&key("b"), now), Lookup::Start { .. }));
        drop(freed);
        // Nothing else holds the evicted image, so its memory goes too.
        assert_eq!(Arc::strong_count(&b), 1);
    }

    #[test]
    fn keeps_decodes_in_flight_over_capacity() {
        let mut cache = RoundImages::with_capacity(1);
        let now = Instant::now();
        cache.start(key("loading"), FIRST_RETRY);
        cache.start(key("a"), FIRST_RETRY);
        cache.finish(key("a"), ready(&image()), now, |_| {});
        assert!(matches!(cache.lookup(&key("loading"), now), Lookup::Wait));
    }

    #[test]
    fn retries_a_failed_decode_after_the_back_off() {
        let path = scratch_file("backoff", b"not a jpeg");
        let key = (path.clone(), 46);
        let mut cache = RoundImages::with_capacity(4);
        let now = Instant::now();
        cache.start(key.clone(), FIRST_RETRY);
        cache.finish(key.clone(), failed(file_stamp(&path)), now, |_| {});
        assert!(matches!(cache.lookup(&key, now), Lookup::Wait));
        let due = now + FIRST_RETRY;
        let Lookup::Start { wait } = cache.lookup(&key, due) else {
            panic!("a failed decode is tried again after its wait");
        };
        assert_eq!(wait, FIRST_RETRY * 2, "the next wait doubles");
        std::fs::remove_file(path).expect("the scratch file exists");
    }

    #[test]
    fn retries_a_failed_decode_when_the_file_changes() {
        let path = scratch_file("changed", b"partial");
        let key = (path.clone(), 46);
        let mut cache = RoundImages::with_capacity(4);
        let now = Instant::now();
        cache.start(key.clone(), FIRST_RETRY);
        cache.finish(key.clone(), failed(file_stamp(&path)), now, |_| {});
        std::fs::write(&path, b"the whole file now").expect("the scratch file is writable");
        assert!(matches!(cache.lookup(&key, now), Lookup::Start { .. }));
        std::fs::remove_file(path).expect("the scratch file exists");
    }

    #[test]
    fn decodes_a_jpeg_and_reports_garbage() {
        let mut jpeg = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(160, 160, image::Rgb([200, 100, 50]))
            .write_to(&mut jpeg, image::ImageFormat::Jpeg)
            .expect("encoding a JPEG in memory");
        let good = scratch_file("good.jpg", jpeg.get_ref());
        let bad = scratch_file("bad.jpg", b"garbage");
        let loaded = load(&good, 92);
        let image = loaded.image.expect("a valid JPEG decodes");
        assert_eq!(image.size(0).width.0, 92);
        assert!(loaded.stamp.is_some());
        assert!(load(&bad, 92).image.is_none());
        for path in [good, bad] {
            std::fs::remove_file(path).expect("the scratch file exists");
        }
    }

    #[test]
    fn keeps_the_middle_and_clears_the_corners() {
        let square = RgbaImage::from_pixel(40, 40, image::Rgba([10, 20, 30, 255]));
        let round = circled(&square);
        assert_eq!(round.get_pixel(20, 20).0, [30, 20, 10, 255]);
        assert_eq!(round.get_pixel(0, 0).0[3], 0);
        // On the edge the pixel is partly covered.
        let edge = round.get_pixel(0, 20).0[3];
        assert!(edge > 0 && edge < 255, "edge alpha {edge}");
    }
}
