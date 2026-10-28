//! Albums (`GroupedMedia` in grid mode): the photos and videos of one album
//! shown as one message in tdesktop's mosaic, with the caption of the one
//! part that has one. The other parts' rows stay in the list, empty, as
//! tdesktop hides them (`HiddenByGroup`).

use std::ops::Range;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, Corners, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Pixels, StatefulInteractiveElement as _, Styled as _, div, px,
};
use model::message::{Content, Message};
use style::chat::{HISTORY_GROUP_SKIP, HISTORY_GROUP_WIDTH_MAX, HISTORY_GROUP_WIDTH_MIN};
use style::{ColorId, Px};
use ui::color;

use super::HistoryView;
use super::bubble;
use super::file_row::FileRow;
use super::grouped_layout::{self, Placed, Rect, Sides};
use super::photo::{self, Photo};
use crate::px_of;

/// One picture or a mosaic of them, sized and placed.
pub(super) struct Pictures {
    pub width: i32,
    pub height: i32,
    parts: Vec<Part>,
}

struct Part {
    /// The part's message, for its button's id.
    id: i64,
    photo: Photo,
    placed: Placed,
}

impl Pictures {
    fn single(id: i64, photo: Photo) -> Self {
        let (width, height) = (photo.width, photo.height);
        let all = Sides {
            left: true,
            top: true,
            right: true,
            bottom: true,
        };
        let rect = Rect {
            x: 0,
            y: 0,
            width,
            height,
        };
        Self {
            width,
            height,
            parts: vec![Part {
                id,
                photo,
                placed: Placed { rect, sides: all },
            }],
        }
    }
}

/// What tdesktop groups into a mosaic; files and music go in a column.
// shortcut: albums of files or music show as separate messages; with the music player row.
fn is_grid_part(message: &Message) -> bool {
    matches!(message.content, Content::Photo(_) | Content::Video(_))
}

impl HistoryView {
    /// The rows of the album the row at `index` belongs to, when more than
    /// one of its photos and videos is in the list.
    pub(super) fn album_rows(&self, index: usize) -> Option<Range<usize>> {
        let album = self.shown.get(index)?.album?;
        let belongs = |i: &usize| {
            let message = &self.shown[*i];
            message.album == Some(album) && is_grid_part(message)
        };
        if !belongs(&index) {
            return None;
        }
        let start = (0..index).rev().take_while(belongs).last().unwrap_or(index);
        let end = (index + 1..self.shown.len())
            .take_while(belongs)
            .last()
            .map_or(index + 1, |last| last + 1);
        (end - start > 1).then_some(start..end)
    }

    /// The row that speaks for the album: the only part with a caption,
    /// else the first (`GroupedMedia::itemForText`).
    pub(super) fn album_face(&self, rows: &Range<usize>) -> usize {
        let mut captioned = rows.clone().filter(|&i| {
            bubble::caption(&self.shown[i]).is_some_and(|caption| !caption.text.is_empty())
        });
        match (captioned.next(), captioned.next()) {
            (Some(only), None) => only,
            _ => rows.start,
        }
    }

    /// The album's or the message's pictures, else its file row.
    pub(super) fn media(
        &mut self,
        album: Option<Range<usize>>,
        message: &Message,
        cx: &mut Context<Self>,
    ) -> (Option<FileRow>, Option<Pictures>) {
        if let Some(pictures) = album.and_then(|rows| self.album(rows, cx)) {
            return (None, Some(pictures));
        }
        if let Some(file) = self
            .upload(message, cx)
            .or_else(|| self.document(message, cx))
            .or_else(|| self.voice(message, cx))
        {
            return (Some(file), None);
        }
        let pictures = self
            .photo(message, cx)
            .map(|photo| Pictures::single(message.id.0, photo));
        (None, pictures)
    }

    /// The album's pictures in their mosaic, downloading each.
    // shortcut: the mosaic keeps its 430 px in a narrower history; with the photo's own narrowing.
    pub(super) fn album(&mut self, rows: Range<usize>, cx: &mut Context<Self>) -> Option<Pictures> {
        let messages: Vec<_> = self.shown[rows].to_vec();
        let photos: Vec<(i64, Photo)> = messages
            .iter()
            .map(|message| Some((message.id.0, self.photo(message, cx)?)))
            .collect::<Option<_>>()?;
        let sizes: Vec<(i32, i32)> = messages
            .iter()
            .map(|message| match &message.content {
                Content::Photo(media) | Content::Video(media) => {
                    media.dimensions.map_or((1, 1), |d| {
                        let side = |value: u32| i32::try_from(value).unwrap_or(i32::MAX).max(1);
                        (side(d.width), side(d.height))
                    })
                }
                _ => (1, 1),
            })
            .collect();
        let placed = grouped_layout::layout(
            &sizes,
            HISTORY_GROUP_WIDTH_MAX.0,
            HISTORY_GROUP_WIDTH_MIN.0,
            HISTORY_GROUP_SKIP.0,
        );
        let width = placed.iter().map(|p| p.rect.x + p.rect.width).max()?;
        let height = placed.iter().map(|p| p.rect.y + p.rect.height).max()?;
        let parts = photos
            .into_iter()
            .zip(placed)
            .map(|((id, photo), placed)| Part {
                id,
                photo: Photo {
                    width: placed.rect.width,
                    height: placed.rect.height,
                    ..photo
                },
                placed,
            })
            .collect();
        Some(Pictures {
            width,
            height,
            parts,
        })
    }
}

/// A part's corners: the bubble's where the part meets both its edges.
fn part_corners(sides: Sides, corners: Corners<Pixels>) -> Corners<Pixels> {
    let pick = |a: bool, b: bool, radius: Pixels| if a && b { radius } else { px(0.0) };
    Corners {
        top_left: pick(sides.top, sides.left, corners.top_left),
        top_right: pick(sides.top, sides.right, corners.top_right),
        bottom_left: pick(sides.bottom, sides.left, corners.bottom_left),
        bottom_right: pick(sides.bottom, sides.right, corners.bottom_right),
    }
}

/// The pictures in place, each with its video button and the selection
/// tint over it.
pub(super) fn mosaic(
    pictures: Pictures,
    corners: Corners<Pixels>,
    is_selected: bool,
    cx: &App,
) -> AnyElement {
    let parts = pictures.parts.into_iter().map(|mut part| {
        let rect = part.placed.rect;
        let corners = part_corners(part.placed.sides, corners);
        let video = part
            .photo
            .video
            .take()
            .map(|video| super::video::overlays(video, part.id, corners, is_selected, cx));
        let open = part.photo.open.take().map(|on_click| {
            div()
                .id(("photo-open", part.id.unsigned_abs()))
                .absolute()
                .inset_0()
                .cursor_pointer()
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    on_click(event, window, cx);
                })
        });
        let spoiler = part
            .photo
            .spoiler
            .take()
            .map(|on_click| super::spoiler::cover(on_click, part.id));
        let once = part
            .photo
            .once
            .take()
            .map(|kind| super::view_once::fire(kind, is_selected, cx));
        let map = part
            .photo
            .map
            .take()
            .map(|url| super::location::pin(url, part.id, (rect.width, rect.height)));
        let tint = is_selected.then(|| {
            div()
                .absolute()
                .inset_0()
                .bg(color(cx, bubble::palette(ColorId::MsgSelectOverlay)))
                .map(|this| photo::rounded(this, corners))
        });
        div()
            .absolute()
            .left(px_of(Px(rect.x)))
            .top(px_of(Px(rect.y)))
            .w(px_of(Px(rect.width)))
            .h(px_of(Px(rect.height)))
            .child(photo::picture(part.photo, corners, cx))
            .children(map)
            .children(open)
            .children(spoiler)
            .children(once)
            .children(tint)
            .children(video.into_iter().flatten())
    });
    div()
        .relative()
        .w(px_of(Px(pictures.width)))
        .h(px_of(Px(pictures.height)))
        .children(parts)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_outer_corners_round() {
        let round = Corners::all(px(16.0));
        let top_left = Sides {
            left: true,
            top: true,
            ..Sides::default()
        };
        let corners = part_corners(top_left, round);
        assert_eq!(corners.top_left, px(16.0));
        assert_eq!(corners.top_right, px(0.0));
        assert_eq!(corners.bottom_left, px(0.0));
        assert_eq!(corners.bottom_right, px(0.0));
    }
}
