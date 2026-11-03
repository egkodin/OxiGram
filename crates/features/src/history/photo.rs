//! A photo in its bubble (`HistoryView::Photo`): sized from its dimensions
//! before it loads, the minithumbnail until the download completes, and the
//! time on a dark pill over it when no caption follows.

use std::path::PathBuf;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, Corners, Div, ImageSource, IntoElement as _, ObjectFit,
    ParentElement as _, Pixels, RenderImage, SharedString, Styled, StyledImage as _, canvas, div,
    img, px,
};
use model::file::{Dimensions, File};
use model::message::{Content, Media, Message};
use style::chat::{
    HISTORY_PHOTO_BUBBLE_MIN_WIDTH, MAX_ANIMATED_EMOJI_SIZE, MAX_MEDIA_SIZE, MEDIA_CAPTION_SKIP,
    MEDIA_IN_BUBBLE_SKIP, MIN_PHOTO_SIZE, MSG_DATE_IMG_DELTA, MSG_DATE_IMG_PADDING, MSG_PADDING,
    WEB_PAGE_DESCRIPTION_FONT, WEB_PAGE_TITLE_FONT,
};
use style::{ColorId, Px};
use ui::color;

use super::HistoryView;
use super::album::{self, Pictures};
use super::bubble::{self, Foot, InfoOn, Item};
use super::documents::current;
use super::video::Video;
use crate::px_of;

/// Photos in view load before userpics (TDLib's 1..=32, higher first).
pub(super) const PHOTO_PRIORITY: i32 = 16;

pub(super) struct Photo {
    pub width: i32,
    pub height: i32,
    /// The downloaded photo, else its minithumbnail.
    pub source: Option<ImageSource>,
    pub thumbnail: Option<ImageSource>,
    /// A video's or GIF's button and status over the still.
    pub video: Option<Video>,
    /// The frame of the video playing in place of the still.
    pub frame: Option<Arc<RenderImage>>,
    /// A map's link to the point in Maps; the pin stands on it.
    pub map: Option<SharedString>,
    /// What shows until the picture loads; nothing for a sticker.
    pub placeholder: Option<ColorId>,
    /// Set while a spoiler hides it: the click that reveals it.
    pub spoiler: Option<bubble::OnClick>,
    /// A photo's click, which opens it in the media viewer.
    pub open: Option<bubble::OnClick>,
    /// Set on view-once and timed media: the fire over the cover.
    pub once: Option<model::message::SelfDestruct>,
}

/// `QSize::scaled` with `Qt::KeepAspectRatio` (`expanding` false) or
/// `KeepAspectRatioByExpanding`, truncating as Qt does.
fn scaled(
    (width, height): (i64, i64),
    (to_width, to_height): (i64, i64),
    expanding: bool,
) -> (i64, i64) {
    let scaled_width = to_height * width / height;
    let use_height = if expanding {
        scaled_width >= to_width
    } else {
        scaled_width <= to_width
    };
    if use_height {
        (scaled_width, to_height)
    } else {
        (to_width, to_width * height / width)
    }
}

/// `Photo::adjustHeightForLessCrop`: a bubble wider than the photo grows
/// taller to show all of it, unless that crops less than a quarter anyway
/// (`FrameResizeMayExpand`).
fn less_crop_height(original: (i64, i64), outer: (i64, i64)) -> i64 {
    let min = outer.0.min(outer.1).min(original.0).min(original.1);
    if 6 * min < 8 + 4 * min {
        return outer.1;
    }
    let big = scaled(original, outer, true);
    let is_expanding = big.0 <= outer.0 && big.1 * (3 * min - 4) <= outer.1 * 4 * min;
    if !is_expanding {
        return outer.1;
    }
    outer.1.max(outer.0 * original.1 / original.0)
}

/// The photo's size in a bubble wide enough for it (`Photo::countCurrentSize`
/// at `maxMediaSize`): fit into 430x430, at least 200 wide and 100 high.
// shortcut: a caption wider than the photo does not widen it, and a history narrower than 430 does not shrink it; with the album layout.
fn size(dimensions: Option<Dimensions>) -> (i32, i32) {
    let max = i64::from(MAX_MEDIA_SIZE.0);
    let original = dimensions.map_or((max, max), |d| (i64::from(d.width), i64::from(d.height)));
    let desired = if original.0 > max || original.1 > max {
        scaled(original, (max, max), false)
    } else {
        original
    };
    let width = desired
        .0
        .max(1)
        .max(i64::from(HISTORY_PHOTO_BUBBLE_MIN_WIDTH.0));
    let height = desired.1.max(1).max(i64::from(MIN_PHOTO_SIZE.0));
    let height = less_crop_height(original, (width, height));
    let narrow = |value: i64| i32::try_from(value).unwrap_or(i32::MAX);
    (narrow(width), narrow(height))
}

impl HistoryView {
    /// A photo, video or GIF message's picture.
    pub(super) fn photo(&mut self, message: &Message, cx: &mut Context<Self>) -> Option<Photo> {
        let photo = match &message.content {
            Content::Photo(media) => Some(Photo {
                open: Some(self.on_open_photo(message.id, cx)),
                ..self.still(media.file.as_ref(), media, cx)
            }),
            Content::Video(_) | Content::Animation(_) | Content::VideoNote { .. } => {
                self.video(message, cx)
            }
            Content::Location(_) | Content::LiveLocation(_) | Content::Venue { .. } => {
                self.map(message, cx)
            }
            Content::Sticker {
                picture: Some(picture),
                ..
            } => Some(self.sticker(message.id, picture, cx)),
            // `Sticker::EmojiSize`, for animated emoji and dice alike.
            Content::Emoji {
                sticker: Some(picture),
                ..
            } if super::large_emoji::is_drawable(picture) => Some(Photo {
                width: MAX_ANIMATED_EMOJI_SIZE.0,
                height: MAX_ANIMATED_EMOJI_SIZE.0,
                ..self.sticker(message.id, picture, cx)
            }),
            _ => None,
        };
        photo.map(|photo| match message.self_destruct {
            Some(kind) => self.once(message, kind, photo, cx),
            None => self.spoil(message, photo, cx),
        })
    }

    /// `file` as `media`'s picture, downloading it the first time it shows.
    pub(super) fn still(
        &mut self,
        file: Option<&File>,
        media: &Media,
        cx: &mut Context<Self>,
    ) -> Photo {
        let file = file.map(|sent| current(self, sent, cx).clone());
        let path = file.as_ref().and_then(|file| file.path.clone());
        if let Some(file) = file.filter(|file| file.path.is_none()) {
            self.fetch(file.id, PHOTO_PRIORITY, cx);
        }
        let (width, height) = size(media.dimensions);
        let thumbnail = media
            .thumbnail
            .as_ref()
            .map(|thumbnail| ImageSource::from(ui::jpeg_image(thumbnail.jpeg.clone())));
        Photo {
            width,
            height,
            source: path
                .map(|path| ImageSource::from(PathBuf::from(path)))
                .or(thumbnail.clone()),
            thumbnail,
            video: None,
            frame: None,
            map: None,
            placeholder: Some(ColorId::ImageBg),
            spoiler: None,
            open: None,
            once: None,
        }
    }
}

/// The picture with the bubble's corners where it meets the bubble's edge.
pub(super) fn picture(photo: Photo, corners: Corners<Pixels>, cx: &App) -> AnyElement {
    let (width, height) = (px_of(Px(photo.width)), px_of(Px(photo.height)));
    if let Some(frame) = photo.frame {
        return playing(frame, corners)
            .w(width)
            .h(height)
            .into_any_element();
    }
    let Some(source) = photo.source else {
        let bg = photo.placeholder.map(|bg| color(cx, bubble::palette(bg)));
        return div()
            .when_some(bg, |this, bg| this.bg(bg))
            .map(|this| rounded(this, corners))
            .w(width)
            .h(height)
            .into_any_element();
    };
    let thumbnail = photo.thumbnail;
    let background = color(cx, bubble::palette(ColorId::ImageBg));
    // The box's own ratio, or gpui takes the image's and a still of another
    // shape stretches the bubble past its height.
    img(source)
        .w(width)
        .h(height)
        .aspect_ratio(width / height)
        .object_fit(ObjectFit::Cover)
        .map(|this| rounded(this, corners))
        .with_loading(move || match thumbnail.clone() {
            Some(thumbnail) => img(thumbnail)
                .size_full()
                .object_fit(ObjectFit::Cover)
                .map(|this| rounded(this, corners))
                .into_any_element(),
            None => div()
                .size_full()
                .bg(background)
                .map(|this| rounded(this, corners))
                .into_any_element(),
        })
        .into_any_element()
}

/// A video's frame over the whole box, cut to its corners.
// shortcut: the frame is stretched to the box, not cropped as the still is; fine for round videos, revisit with GIFs of odd sizes.
fn playing(frame: Arc<RenderImage>, corners: Corners<Pixels>) -> gpui::Canvas<()> {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            if let Err(error) = window.paint_image(bounds, bounds, corners, frame, 0, false) {
                tracing::warn!(%error, "cannot paint a video frame");
            }
        },
    )
}

/// Each corner's own radius; `Styled` has no setter for all four.
pub(super) fn rounded<E: Styled>(element: E, corners: Corners<Pixels>) -> E {
    element
        .rounded_tl(corners.top_left)
        .rounded_tr(corners.top_right)
        .rounded_bl(corners.bottom_left)
        .rounded_br(corners.bottom_right)
}

/// Stickers and round videos stand alone, outside any bubble
/// (`UnwrappedMedia`).
fn is_unwrapped(message: &Message) -> bool {
    matches!(
        message.content,
        Content::Sticker { .. } | Content::VideoNote { .. } | Content::Emoji { .. }
    )
}

/// The time on its pill in the photo's corner (`InfoDisplayType::Image`).
fn info_over(item: &Item<'_>, out: bool, cx: &App) -> Div {
    info_pill(item, out, cx)
        .absolute()
        .right(px_of(MSG_DATE_IMG_DELTA))
        .bottom(px_of(MSG_DATE_IMG_DELTA))
}

/// The time on its pill, wherever it goes.
pub(super) fn info_pill(item: &Item<'_>, out: bool, cx: &App) -> Div {
    // Unwrapped media have the time on the service pill (`InfoDisplayType::Background`).
    let bg = match (is_unwrapped(item.message), item.is_selected) {
        (true, false) => ColorId::MsgServiceBg,
        (true, true) => ColorId::MsgServiceBgSelected,
        (false, true) => ColorId::MsgDateImgBgSelected,
        (false, false) => ColorId::MsgDateImgBg,
    };
    div()
        .px(px_of(MSG_DATE_IMG_PADDING.x))
        .py(px_of(MSG_DATE_IMG_PADDING.y))
        .rounded_full()
        .bg(color(cx, bubble::palette(bg)))
        .child(bubble::info_content(item, out, InfoOn::Image, cx))
}

/// The bubble: name and headers, the picture, then the caption and the
/// reactions under it with the time.
pub(super) fn bubble(item: &mut Item<'_>, out: bool, pictures: Pictures, cx: &App) -> Div {
    let width = px_of(Px(pictures.width));
    let name = bubble::name_line(item, out, cx);
    let headers = bubble::headers(item, out, cx);
    let has_header = name.is_some() || !headers.is_empty();
    let has_caption = bubble::caption(item.message).is_some_and(|caption| !caption.text.is_empty());
    let place = super::location::lines(item.message, cx);
    let has_below = has_caption || place.is_some() || !item.reactions.is_empty();
    let comments = item
        .comments
        .take()
        .map(|comments| super::comments::button(comments, item.message.id.0, item.is_selected, cx));
    let foot = if has_below || comments.is_some() {
        Foot::Tail
    } else {
        Foot::Media
    };
    let (frame, fg) = bubble::frame(out, item.is_selected, &item.run, foot, cx);
    let edges = match &item.message.content {
        Content::Sticker { .. } | Content::Emoji { .. } => Corners::default(),
        Content::VideoNote { .. } => Corners::all(px_of(Px(pictures.width)) / 2.0),
        _ => bubble::corners(out, &item.run, foot),
    };
    let corners = Corners {
        top_left: if has_header { px(0.0) } else { edges.top_left },
        top_right: if has_header { px(0.0) } else { edges.top_right },
        bottom_left: if has_below || comments.is_some() {
            px(0.0)
        } else {
            edges.bottom_left
        },
        bottom_right: if has_below || comments.is_some() {
            px(0.0)
        } else {
            edges.bottom_right
        },
    };
    let header = has_header.then(|| {
        div()
            .pt(px_of(MSG_PADDING.top))
            .pl(px_of(MSG_PADDING.left))
            .pr(px_of(MSG_PADDING.right))
            .pb(px_of(MEDIA_IN_BUBBLE_SKIP))
            .children(name)
            .children(headers)
    });
    // Media alone has no bubble behind it, nor its tail (`needsBubble`).
    let frame = if has_header || has_below || comments.is_some() {
        frame
    } else {
        div().relative()
    };
    let picture = div()
        .relative()
        .child(album::mosaic(pictures, corners, item.is_selected, cx))
        .when(!has_below, |this| this.child(info_over(item, out, cx)));
    let below = has_below.then(|| {
        let room = bubble::info_content(item, out, InfoOn::Bubble, cx);
        let reactions = std::mem::take(&mut item.reactions);
        div()
            .relative()
            .pt(px_of(if has_caption {
                MEDIA_CAPTION_SKIP
            } else if place.is_some() {
                MEDIA_IN_BUBBLE_SKIP
            } else {
                Px(0)
            }))
            .pl(px_of(MSG_PADDING.left))
            .pr(px_of(MSG_PADDING.right))
            .pb(px_of(MSG_PADDING.bottom))
            .when(has_caption, |this| {
                this.child(bubble::text_block(item, out, fg, cx))
            })
            .when_some(place, |this, place| {
                this.child(place_lines(
                    place,
                    (reactions.is_empty()).then(|| bubble::time_room(item, out, cx)),
                    fg,
                    cx,
                ))
            })
            .when(!reactions.is_empty(), |this| {
                this.child(super::reactions::row(
                    reactions,
                    out,
                    item.is_selected,
                    room,
                    cx,
                ))
            })
            .child(bubble::info(item, out, cx))
    });
    frame
        .p_0()
        .w(width)
        .children(header)
        .child(picture)
        .children(below)
        .children(comments)
}

/// A venue's title and address in the link preview's fonts, the time
/// beside the last line.
fn place_lines(
    (title, address): (String, String),
    room: Option<String>,
    fg: ColorId,
    cx: &App,
) -> Div {
    let fg = color(cx, bubble::palette(fg));
    let mut parts = vec![(title, WEB_PAGE_TITLE_FONT, fg, 2)];
    if !address.is_empty() {
        parts.push((address, WEB_PAGE_DESCRIPTION_FONT, fg, 3));
    }
    let last = parts.len() - 1;
    div().children(parts.into_iter().enumerate().map(|(index, part)| {
        super::web_page::line(part, (index == last).then(|| room.clone()).flatten())
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized(width: i32, height: i32) -> (i32, i32) {
        size(Dimensions::new(width, height))
    }

    #[test]
    fn fits_photos_into_the_media_box() {
        assert_eq!(sized(640, 427), (430, 286));
        assert_eq!(sized(1080, 1920), (241, 430));
        assert_eq!(sized(4000, 1000), (430, 107));
    }

    #[test]
    fn small_photos_keep_the_bubble_minimum() {
        // Covering 200x100 would cut half of a square away, so it stays cropped.
        assert_eq!(sized(50, 50), (200, 100));
        // A 180x120 photo grows to show itself whole: it crops under a quarter.
        assert_eq!(sized(180, 120), (200, 133));
    }

    #[test]
    fn unknown_dimensions_take_the_whole_box() {
        assert_eq!(size(None), (430, 430));
    }
}
