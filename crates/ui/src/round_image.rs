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
/// Pixel buffers only. The atlas has its own storage for the same images.
const PIXEL_BUDGET: usize = 24_000_000;
/// Keep the visible working set even when it temporarily exceeds the budget.
const KEEP: Duration = Duration::from_secs(1);
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
    touched_at: Cell<Instant>,
}

impl Slot {
    fn pixel_bytes(&self) -> usize {
        match self {
            Self::Ready(image) => image.as_bytes(0).expect("userpics have one frame").len(),
            _ => 0,
        }
    }
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
    pixel_bytes: usize,
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
            pixel_bytes: 0,
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
        entry.touched_at.set(now);
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

    fn start(&mut self, key: Key, wait: Duration, now: Instant) {
        let entry = Entry {
            slot: Slot::Loading { wait },
            last_used: Cell::new(self.tick()),
            touched_at: Cell::new(now),
        };
        if let Some(previous) = self.entries.insert(key, entry) {
            self.pixel_bytes -= previous.slot.pixel_bytes();
        }
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
        let Slot::Loading { wait } = entry.slot else {
            return;
        };
        entry.slot = match loaded.image {
            Some(image) => Slot::Ready(image),
            None => Slot::Failed {
                retry_at: now + wait,
                next_wait: (wait * 2).min(MAX_RETRY),
                stamp: loaded.stamp,
            },
        };
        entry.touched_at.set(now);
        self.pixel_bytes += entry.slot.pixel_bytes();
        self.evict(now, PIXEL_BUDGET, &mut free);
    }

    fn evict(&mut self, now: Instant, budget: usize, mut free: impl FnMut(Arc<RenderImage>)) {
        while self.entries.len() > self.capacity || self.pixel_bytes > budget {
            let over_capacity = self.entries.len() > self.capacity;
            // Loading entries stay, so a decode in flight is never repeated.
            let oldest = self
                .entries
                .iter()
                .filter(|(_, entry)| match entry.slot {
                    Slot::Loading { .. } => false,
                    Slot::Failed { .. } => over_capacity,
                    Slot::Ready(_) => {
                        over_capacity || now.duration_since(entry.touched_at.get()) >= KEEP
                    }
                })
                .min_by_key(|(_, entry)| entry.last_used.get())
                .map(|(key, _)| key.clone());
            let Some(oldest) = oldest else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.pixel_bytes -= entry.slot.pixel_bytes();
                if let Slot::Ready(image) = entry.slot {
                    free(image);
                }
            }
        }
    }
}

/// The image at `path` filled into a `side` px square and cut to a
/// circle; `None` while it decodes in the background or after it failed.
pub(crate) fn round_file(path: &Path, side: u32, cx: &mut App) -> Option<Arc<RenderImage>> {
    let key = (path.to_path_buf(), side);
    let now = Instant::now();
    let found = cx
        .try_global::<RoundImages>()
        .map_or(Lookup::Start { wait: FIRST_RETRY }, |cache| {
            cache.lookup(&key, now)
        });
    let wait = match found {
        Lookup::Ready(image) => return Some(image),
        Lookup::Wait => return None,
        Lookup::Start { wait } => wait,
    };
    cx.default_global::<RoundImages>()
        .start(key.clone(), wait, now);
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
    let image = match load_square(path, side) {
        Ok(filled) => Some(Arc::new(RenderImage::new([Frame::new(circled(filled))]))),
        Err(_) => {
            tracing::warn!("cannot decode a userpic within the image limits");
            None
        }
    };
    Loaded { image, stamp }
}

fn load_square(path: &Path, side: u32) -> image::ImageResult<RgbaImage> {
    const MAX_SIDE: u32 = 4096;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(64 << 20);
    limits.check_dimensions(side, side)?;
    let mut reader = image::ImageReader::open(path)?;
    reader.limits(limits);
    let mut decoded = reader.decode()?;
    let (width, height) = (decoded.width(), decoded.height());
    let short = width.min(height);
    // resize_to_fill scales the long edge before cropping. A thin strip can
    // exceed the decode budget at this step even when both input sides fit.
    if u64::from(width.max(height)) * u64::from(side) > u64::from(short) * u64::from(MAX_SIDE) {
        decoded = decoded.crop_imm((width - short) / 2, (height - short) / 2, short, short);
    }
    Ok(decoded
        .resize_to_fill(side, side, FilterType::Lanczos3)
        .into_rgba8())
}

/// BGRA with straight alpha, as gpui's sprites expect, and an antialiased
/// circle: each pixel keeps the share of it inside the circle.
fn circled(mut square: RgbaImage) -> RgbaImage {
    #[expect(clippy::cast_precision_loss, reason = "userpics are small")]
    let radius = square.width().min(square.height()) as f32 / 2.0;
    for (x, y, pixel) in square.enumerate_pixels_mut() {
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
        *pixel = image::Rgba([b, g, r, alpha]);
    }
    square
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

    #[test]
    #[ignore = "offline memory measurement"]
    fn offline_userpic_cache_workload() {
        let mut cache = RoundImages::default();
        let start = Instant::now();
        let mut peak = 0;
        for id in 0..2000 {
            let side = 160;
            let key = (PathBuf::from(format!("avatar-{id}")), side);
            let now = start + Duration::from_millis(id * 30);
            let pixels = RgbaImage::from_pixel(side, side, image::Rgba([30, 20, 10, 255]));
            cache.start(key.clone(), FIRST_RETRY, now);
            cache.finish(
                key,
                Loaded {
                    image: Some(Arc::new(RenderImage::new([Frame::new(pixels)]))),
                    stamp: None,
                },
                now,
                |_| {},
            );
            peak = peak.max(decoded_bytes(&cache));
        }
        eprintln!(
            "userpics: {} entries, {} retained pixel bytes, {peak} peak retained pixel bytes",
            cache.entries.len(),
            decoded_bytes(&cache)
        );
        assert!(peak <= PIXEL_BUDGET);
        assert_eq!(cache.pixel_bytes, decoded_bytes(&cache));
    }

    fn decoded_bytes(cache: &RoundImages) -> usize {
        cache
            .entries
            .values()
            .map(|entry| match &entry.slot {
                Slot::Ready(image) => image.as_bytes(0).expect("userpics have one frame").len(),
                _ => 0,
            })
            .sum()
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
            cache.start(key(name), FIRST_RETRY, now);
            cache.finish(key(name), ready(image), now, |_| panic!("under capacity"));
        }
        // Drawing `a` again leaves `b` the oldest.
        assert!(matches!(cache.lookup(&key("a"), now), Lookup::Ready(_)));
        let mut freed = Vec::new();
        cache.start(key("c"), FIRST_RETRY, now);
        cache.finish(key("c"), ready(&c), now, |image| freed.push(image));
        assert_eq!(freed.len(), 1);
        assert!(Arc::ptr_eq(&freed[0], &b), "the freed image is b's");
        assert!(matches!(cache.lookup(&key("b"), now), Lookup::Start { .. }));
        drop(freed);
        // Nothing else holds the evicted image, so its memory goes too.
        assert_eq!(Arc::strong_count(&b), 1);
    }

    #[test]
    fn pixel_pressure_preserves_an_active_userpic_and_reloads_an_evicted_one() {
        let mut png = std::io::Cursor::new(Vec::new());
        RgbaImage::from_pixel(2, 2, image::Rgba([200, 100, 50, 255]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let path = scratch_file("eviction.png", png.get_ref());
        let cold = (path.clone(), 2);
        let hot = key("hot");
        let mut cache = RoundImages::default();
        let start = Instant::now();
        cache.start(cold.clone(), FIRST_RETRY, start);
        cache.finish(cold.clone(), load(&path, 2), start, |_| {});
        let Lookup::Ready(original) = cache.lookup(&cold, start) else {
            panic!("the fixture must decode");
        };
        cache.start(hot.clone(), FIRST_RETRY, start);
        let hot_image = image();
        cache.finish(hot.clone(), ready(&hot_image), start, |_| {});

        let now = start + KEEP;
        assert!(matches!(cache.lookup(&hot, now), Lookup::Ready(_)));
        let mut freed = Vec::new();
        cache.evict(now, 4, |image| freed.push(image));
        assert_eq!(freed.len(), 1);
        assert!(Arc::ptr_eq(&freed[0], &original));
        assert_eq!(cache.pixel_bytes, 4);
        assert!(matches!(cache.lookup(&hot, now), Lookup::Ready(_)));
        let Lookup::Start { wait } = cache.lookup(&cold, now) else {
            panic!("eviction must leave the image reloadable");
        };
        cache.start(cold.clone(), wait, now);
        cache.finish(cold.clone(), load(&path, 2), now, |_| {});
        std::fs::remove_file(path).unwrap();
        let Lookup::Ready(reloaded) = cache.lookup(&cold, now) else {
            panic!("the evicted userpic must decode again");
        };
        assert_ne!(reloaded.id, original.id);
        assert_eq!(reloaded.as_bytes(0), original.as_bytes(0));
    }

    #[test]
    fn keeps_decodes_in_flight_over_capacity() {
        let mut cache = RoundImages::with_capacity(1);
        let now = Instant::now();
        cache.start(key("loading"), FIRST_RETRY, now);
        cache.start(key("a"), FIRST_RETRY, now);
        cache.finish(key("a"), ready(&image()), now, |_| {});
        assert!(matches!(cache.lookup(&key("loading"), now), Lookup::Wait));
    }

    #[test]
    fn retries_a_failed_decode_after_the_back_off() {
        let path = scratch_file("backoff", b"not a jpeg");
        let key = (path.clone(), 46);
        let mut cache = RoundImages::with_capacity(4);
        let now = Instant::now();
        cache.start(key.clone(), FIRST_RETRY, now);
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
        cache.start(key.clone(), FIRST_RETRY, now);
        cache.finish(key.clone(), failed(file_stamp(&path)), now, |_| {});
        std::fs::write(&path, b"the whole file now").expect("the scratch file is writable");
        assert!(matches!(cache.lookup(&key, now), Lookup::Start { .. }));
        std::fs::remove_file(path).expect("the scratch file exists");
    }

    #[test]
    fn rejects_userpics_exceeding_the_dimension_budget() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbImage::new(4097, 1)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let path = scratch_file("over-budget.png", png.get_ref());
        let loaded = load(&path, 1);
        std::fs::remove_file(path).unwrap();
        assert!(loaded.image.is_none());
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
        let buffer = square.as_raw().as_ptr();
        let round = circled(square);
        assert_eq!(round.as_raw().as_ptr(), buffer);
        assert_eq!(round.get_pixel(20, 20).0, [30, 20, 10, 255]);
        assert_eq!(round.get_pixel(0, 0).0[3], 0);
        // On the edge the pixel is partly covered.
        let edge = round.get_pixel(0, 20).0[3];
        assert!(edge > 0 && edge < 255, "edge alpha {edge}");
    }
}
