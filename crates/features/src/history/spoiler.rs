//! Media under a spoiler (`MediaSpoiler`): the blurry preview with dots
//! over it until a click reveals the photo or video.

use std::rc::Rc;

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement as _, ParentElement as _, Point,
    StatefulInteractiveElement as _, Styled as _, canvas, div, fill, hsla, point, px, size,
};
use model::message::{Content, Message};

use super::HistoryView;
use super::bubble::OnClick;
use super::photo::Photo;

/// Dots per 1000 square points, about tdesktop's particle density.
const DENSITY: f32 = 6.0;

/// Where the dots go: the same for a given size, so frames do not flicker.
fn dots(width: f32, height: f32) -> Vec<(Point<f32>, f32)> {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a dot count"
    )]
    let count = (width * height / 1000.0 * DENSITY) as usize;
    let mut seed: u32 = 0x9E37_79B9;
    let mut next = move || {
        // xorshift32
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        #[expect(clippy::cast_precision_loss, reason = "a share")]
        let share = seed as f32 / u32::MAX as f32;
        share
    };
    (0..count)
        .map(|_| (point(next() * width, next() * height), 0.3 + next() * 0.7))
        .collect()
}

impl HistoryView {
    /// `photo` as the spoiler hides it, until it is revealed.
    pub(super) fn spoil(&self, message: &Message, photo: Photo, cx: &Context<Self>) -> Photo {
        let has_spoiler = match &message.content {
            Content::Photo(media) | Content::Video(media) | Content::Animation(media) => {
                media.has_spoiler
            }
            _ => false,
        };
        if !has_spoiler || self.revealed_spoilers.contains(&message.id) {
            return photo;
        }
        let weak = cx.weak_entity();
        let id = message.id;
        // shortcut: the preview is stretched, not blurred, and the dots stand still; the particle animation comes with M8 polish.
        Photo {
            source: photo.thumbnail.clone(),
            video: None,
            frame: None,
            spoiler: Some(Rc::new(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.revealed_spoilers.insert(id);
                    cx.notify();
                });
            })),
            ..photo
        }
    }
}

/// The dots over the preview, revealing it on a click.
pub(super) fn cover(on_click: OnClick, id: i64) -> AnyElement {
    let paint = canvas(
        |_, _, _| {},
        |bounds, (), window, _| {
            let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            for (at, alpha) in dots(width, height) {
                let origin = bounds.origin + point(px(at.x), px(at.y));
                window.paint_quad(fill(
                    gpui::Bounds::new(origin, size(px(1.0), px(1.0))),
                    hsla(0.0, 0.0, 1.0, alpha),
                ));
            }
        },
    )
    .size_full();
    div()
        .id(("spoiler", id.unsigned_abs()))
        .absolute()
        .inset_0()
        .cursor_pointer()
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            on_click(event, window, cx);
        })
        .child(paint)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dots_fill_the_box_the_same_way_each_time() {
        let first = dots(200.0, 100.0);
        assert_eq!(first.len(), 120);
        assert_eq!(first, dots(200.0, 100.0));
        assert!(first.iter().all(|(at, alpha)| {
            (0.0..=200.0).contains(&at.x) && (0.0..=100.0).contains(&at.y) && *alpha <= 1.0
        }));
    }
}
