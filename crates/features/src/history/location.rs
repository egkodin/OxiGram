//! Locations and venues in the history (`HistoryView::Location`): TDLib's
//! map picture with the pin in its middle, a venue's title and address
//! under it, and a click that opens Maps.

use std::collections::HashMap;

use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement as _, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, div,
};
use i18n::key;
use model::file::{Dimensions, File};
use model::location::{MapSize, Point};
use model::message::{Content, Media, Message};
use style::basic::LOCATION_SIZE;
use style::chat::{HISTORY_MAP_POINT, HISTORY_MAP_POINT_INNER};
use style::{ColorId, Px};
use ui::{icon, icon_size};

use super::HistoryView;
use super::photo::Photo;
use crate::lang::tr;
use crate::px_of;

/// TDLib's zoom for the scale below (`13 + (scale - 1)`).
const ZOOM: i32 = 14;
/// The picture's pixels per point.
// shortcut: always the 2x picture, as on Retina screens; a 1x screen would take TDLib's scale 1 to save traffic.
const SCALE: i32 = 2;

/// Map pictures by point: `None` while TDLib looks up the file.
#[derive(Default)]
pub(super) struct Maps(HashMap<Point, Option<File>>);

fn point(content: &Content) -> Option<Point> {
    match content {
        Content::Location(point) | Content::LiveLocation(point) => Some(*point),
        Content::Venue { point, .. } => Some(*point),
        _ => None,
    }
}

/// The title and the line under it, for a venue or a live location.
// shortcut: a live location shows as a still point with its title; the sender's pin, "updated" line and countdown come with live locations (row 269).
pub(super) fn lines(message: &Message, cx: &App) -> Option<(String, String)> {
    match &message.content {
        Content::Venue { title, address, .. } => Some((title.clone(), address.clone())),
        Content::LiveLocation(_) => {
            Some((tr(cx, key::lng_live_location).to_string(), String::new()))
        }
        _ => None,
    }
}

impl HistoryView {
    /// A location's map picture, asking TDLib for its file the first time.
    pub(super) fn map(&mut self, message: &Message, cx: &mut Context<Self>) -> Option<Photo> {
        let point = point(&message.content)?;
        let file = match self.maps.0.get(&point) {
            Some(file) => file.clone(),
            None => {
                self.request_map(point, cx);
                None
            }
        };
        let (width, height) = (LOCATION_SIZE.width.0, LOCATION_SIZE.height.0);
        let media = Media {
            dimensions: Dimensions::new(width, height),
            ..Media::default()
        };
        Some(Photo {
            map: Some(point.maps_url().into()),
            // The bubble's color until TDLib draws the map (`PaintBubble`).
            placeholder: Some(if message.is_outgoing {
                ColorId::MsgOutBg
            } else {
                ColorId::MsgInBg
            }),
            ..self.still(file.as_ref(), &media, cx)
        })
    }

    fn request_map(&mut self, point: Point, cx: &mut Context<Self>) {
        self.maps.0.insert(point, None);
        let size = MapSize {
            width: LOCATION_SIZE.width.0 / SCALE,
            height: LOCATION_SIZE.height.0 / SCALE,
            zoom: ZOOM,
            scale: SCALE,
        };
        let request = self
            .store
            .read(cx)
            .session()
            .map_thumbnail(point, size, self.chat);
        let task = crate::store::spawn(request, cx);
        cx.spawn(async move |this, cx| match task.await {
            Ok(file) => {
                let _ = this.update(cx, |this, cx| {
                    this.maps.0.insert(point, Some(file));
                    cx.notify();
                });
            }
            // shortcut: the map stays blank after a failed lookup until the chat reopens.
            Err(error) => tracing::warn!(%error, "looking up a map picture failed"),
        })
        .detach();
    }
}

/// The pin standing on the middle of the map, and the click that opens
/// the point in Maps.
pub(super) fn pin(url: SharedString, id: i64, (width, height): (i32, i32)) -> AnyElement {
    let size = icon_size(HISTORY_MAP_POINT);
    let left = px_of(Px(width)) / 2.0 - size.width / 2.0;
    let top = px_of(Px(height)) / 2.0 - size.height;
    div()
        .id(("map", id.unsigned_abs()))
        .absolute()
        .inset_0()
        .cursor_pointer()
        .on_click(move |_, _, cx| cx.open_url(&url))
        .child(
            div()
                .absolute()
                .left(left)
                .top(top)
                .child(icon(HISTORY_MAP_POINT))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .child(icon(HISTORY_MAP_POINT_INNER)),
                ),
        )
        .into_any_element()
}
