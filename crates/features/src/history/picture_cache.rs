//! The history's pictures, kept while recently drawn. gpui's own cache
//! keeps every image a window ever drew, decoded and on the GPU, so a chat
//! of photos grew by two copies of each one scrolled past.

use std::collections::HashMap;
use std::hash::{Hash as _, Hasher as _};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    App, AppContext as _, Entity, Global, ImageCache, ImageCacheError, ImageCacheItem, Img,
    RenderImage, Resource, Window,
};

/// Decoded bytes kept, a dozen 1280 px photos; the GPU holds each once more.
const BUDGET: usize = 64 << 20;
/// What was drawn this recently stays whatever the budget, or a screen of
/// pictures over it would decode again every frame.
const KEEP: Duration = Duration::from_secs(1);

struct Entry<T> {
    item: T,
    last_used: Instant,
}

/// The least-recently-drawn bookkeeping, apart from gpui for the tests.
struct Recent<T> {
    entries: HashMap<u64, Entry<T>>,
}

impl<T> Recent<T> {
    fn contains(&self, key: u64) -> bool {
        self.entries.contains_key(&key)
    }

    fn get_or_insert(&mut self, key: u64, now: Instant, make: impl FnOnce() -> T) -> &T {
        let entry = self.entries.entry(key).or_insert_with(|| Entry {
            item: make(),
            last_used: now,
        });
        entry.last_used = now;
        &entry.item
    }

    /// Removes the least recently drawn entries until the rest weigh
    /// `budget` or less; those drawn within `KEEP` stay.
    fn evict(&mut self, now: Instant, budget: usize, weigh: impl Fn(&T) -> usize) -> Vec<T> {
        let mut total: usize = self.entries.values().map(|entry| weigh(&entry.item)).sum();
        let mut old: Vec<(u64, Instant)> = self
            .entries
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.last_used) > KEEP)
            .map(|(key, entry)| (*key, entry.last_used))
            .collect();
        old.sort_by_key(|(_, last_used)| *last_used);
        let mut evicted = Vec::new();
        for (key, _) in old {
            if total <= budget {
                break;
            }
            if let Some(entry) = self.entries.remove(&key) {
                total = total.saturating_sub(weigh(&entry.item));
                evicted.push(entry.item);
            }
        }
        evicted
    }
}

/// A loaded picture's decoded bytes; nothing while it loads or after it failed.
fn decoded_bytes(item: &ImageCacheItem) -> usize {
    let Some(Ok(image)) = item.get() else {
        return 0;
    };
    let size = image.size(0);
    let pixels = i64::from(size.width.0) * i64::from(size.height.0);
    usize::try_from(pixels).unwrap_or(0) * 4
}

pub(crate) struct PictureCache(Recent<ImageCacheItem>);

impl ImageCache for PictureCache {
    fn load(
        &mut self,
        resource: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        resource.hash(&mut hasher);
        let key = hasher.finish();
        let now = Instant::now();
        if !self.0.contains(key) {
            for item in self.0.evict(now, BUDGET, decoded_bytes) {
                if let Some(Ok(image)) = item.get() {
                    cx.drop_image(image, Some(window));
                }
            }
        }
        self.0
            .get_or_insert(key, now, || ImageCacheItem::new(resource, cx))
            .use_image(window)
    }
}

struct Pictures(Entity<PictureCache>);

impl Global for Pictures {}

/// Starts the cache; without it pictures load through gpui's own.
pub(crate) fn init(cx: &mut App) {
    let cache = cx.new(|_| {
        PictureCache(Recent {
            entries: HashMap::new(),
        })
    });
    cx.set_global(Pictures(cache));
}

/// `picture` loading through the bounded cache.
pub(super) fn bounded(picture: Img, cx: &App) -> Img {
    match cx.try_global::<Pictures>() {
        Some(pictures) => picture.image_cache(&pictures.0),
        None => picture,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: usize = 1 << 20;

    fn weights(sizes: &[(u64, usize, u64)], start: Instant) -> Recent<usize> {
        let mut recent = Recent {
            entries: HashMap::new(),
        };
        for (key, weight, drawn_at) in sizes {
            recent.get_or_insert(*key, start + Duration::from_secs(*drawn_at), || *weight);
        }
        recent
    }

    #[test]
    fn evicts_the_least_recently_drawn_down_to_the_budget() {
        let start = Instant::now();
        let mut recent = weights(&[(1, 5 * MB, 0), (2, 5 * MB, 1), (3, 5 * MB, 2)], start);
        // Drawing the first again leaves the second the oldest.
        recent.get_or_insert(1, start + Duration::from_secs(3), || 0);
        let evicted = recent.evict(start + Duration::from_secs(10), 10 * MB, |weight| *weight);
        assert_eq!(evicted, [5 * MB]);
        assert!(recent.contains(1) && !recent.contains(2) && recent.contains(3));
    }

    #[test]
    fn keeps_everything_under_the_budget() {
        let start = Instant::now();
        let mut recent = weights(&[(1, 5 * MB, 0), (2, 5 * MB, 1)], start);
        let evicted = recent.evict(start + Duration::from_secs(10), 10 * MB, |weight| *weight);
        assert!(evicted.is_empty());
    }

    #[test]
    fn keeps_what_was_just_drawn_even_over_the_budget() {
        let start = Instant::now();
        let mut recent = weights(&[(1, 5 * MB, 0), (2, 5 * MB, 10), (3, 5 * MB, 10)], start);
        let now = start + Duration::from_secs(10) + KEEP / 2;
        let evicted = recent.evict(now, MB, |weight| *weight);
        assert_eq!(evicted.len(), 1, "only the one drawn long ago goes");
        assert!(recent.contains(2) && recent.contains(3));
    }
}
