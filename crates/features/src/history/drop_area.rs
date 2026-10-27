//! The zones over the chat while files are dragged in (`DragArea`): one
//! for files, or for images a smaller one to send them as documents above
//! a larger one to send them compressed. Dropping on a zone opens the send
//! box that way.

use std::path::PathBuf;

use gpui::{
    AnyElement, App, Bounds, BoxShadow, Context, DragMoveEvent, ExternalPaths, IntoElement as _,
    ParentElement as _, Pixels, Point, Styled as _, Window, div, hsla, point, px,
};
use i18n::key;
use style::chat_helpers::{
    DRAG_COLOR, DRAG_DROP_COLOR, DRAG_FONT, DRAG_HEIGHT, DRAG_ICON_FILES, DRAG_ICON_MEDIA,
    DRAG_ICON_SKIP, DRAG_MARGIN, DRAG_PADDING, DRAG_SUBFONT,
};
use style::layers::BOX_RADIUS;
use style::{ColorId, ColorRef, Icon};
use ui::{color, font, font_height, icon_size};

use super::HistoryView;
use super::attach::is_photo_path;
use super::send_files_box::Compression;
use crate::lang::tr;
use crate::px_of;

/// What the dragged files are offered as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zones {
    /// One zone: they go as documents.
    Files,
    /// All images: as documents above, compressed below.
    Photos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zone {
    Documents,
    Compressed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DropZones {
    zones: Zones,
    bounds: Bounds<Pixels>,
    over: Option<Zone>,
}

/// Each zone's bounds within the chat's (`updateAttachGeometry`).
fn layout(zones: Zones, bounds: Bounds<Pixels>) -> Vec<(Zone, Bounds<Pixels>)> {
    let top = bounds.origin.y + px_of(DRAG_MARGIN.top);
    let full = bounds.size.height - px_of(DRAG_MARGIN.top) - px_of(DRAG_MARGIN.bottom);
    let width = bounds.size.width - px_of(DRAG_MARGIN.left) - px_of(DRAG_MARGIN.right);
    let left = bounds.origin.x + px_of(DRAG_MARGIN.left);
    let zone = |y: Pixels, height: Pixels| Bounds {
        origin: point(left, y),
        size: gpui::size(width, height),
    };
    match zones {
        Zones::Files => vec![(Zone::Documents, zone(top, full))],
        Zones::Photos => {
            let smaller = (full / 3.0).floor();
            let fits = smaller
                >= minimal_height() + icon_size(DRAG_ICON_FILES).height + px_of(DRAG_ICON_SKIP);
            let documents = if fits { smaller } else { (full / 2.0).floor() };
            vec![
                (Zone::Documents, zone(top, documents)),
                (Zone::Compressed, zone(top + documents, full - documents)),
            ]
        }
    }
}

/// `DragArea::MinimalHeight`.
fn minimal_height() -> Pixels {
    let padding =
        px_of(DRAG_PADDING.top).max(font_height(DRAG_SUBFONT) + px_of(DRAG_PADDING.bottom));
    px_of(DRAG_HEIGHT) + padding * 2.0
}

fn inner(zone: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds {
        origin: point(
            zone.origin.x + px_of(DRAG_PADDING.left),
            zone.origin.y + px_of(DRAG_PADDING.top),
        ),
        size: gpui::size(
            zone.size.width - px_of(DRAG_PADDING.left) - px_of(DRAG_PADDING.right),
            zone.size.height - px_of(DRAG_PADDING.top) - px_of(DRAG_PADDING.bottom),
        ),
    }
}

fn zone_at(zones: Zones, bounds: Bounds<Pixels>, position: Point<Pixels>) -> Option<Zone> {
    layout(zones, bounds)
        .into_iter()
        .find(|(_, zone)| inner(*zone).contains(&position))
        .map(|(zone, _)| zone)
}

fn zones_for(paths: &[PathBuf]) -> Zones {
    if !paths.is_empty() && paths.iter().all(|path| is_photo_path(path)) {
        Zones::Photos
    } else {
        Zones::Files
    }
}

impl HistoryView {
    /// Follows dragged files over the chat; leaving it hides the zones.
    pub(super) fn drag_moved(
        &mut self,
        event: &DragMoveEvent<ExternalPaths>,
        cx: &mut Context<Self>,
    ) {
        let position = event.event.position;
        let next = event.bounds.contains(&position).then(|| {
            let zones = zones_for(event.drag(cx).paths());
            DropZones {
                zones,
                bounds: event.bounds,
                over: zone_at(zones, event.bounds, position),
            }
        });
        if self.drop_zones != next {
            self.drop_zones = next;
            cx.notify();
        }
    }

    pub(super) fn dropped(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let zone = self.drop_zones.take().and_then(|zones| zones.over);
        cx.notify();
        let compression = match zone {
            Some(Zone::Documents) => Compression::AsFiles,
            Some(Zone::Compressed) => Compression::Compressed,
            None => return,
        };
        self.confirm_files(paths.paths().to_vec(), compression, window, cx);
    }

    /// The zones, while files are dragged over the chat.
    pub(super) fn drop_area(&mut self, cx: &App) -> Option<AnyElement> {
        if !cx.has_active_drag() {
            // The drag left the window or ended elsewhere.
            self.drop_zones = None;
        }
        let zones = self.drop_zones?;
        let origin = zones.bounds.origin;
        Some(
            div()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .size_full()
                .children(
                    layout(zones.zones, zones.bounds)
                        .into_iter()
                        .map(|(zone, bounds)| {
                            let (title, subtitle, icon) = texts(zones.zones, zone);
                            let is_over = zones.over == Some(zone);
                            zone_element(
                                bounds - origin,
                                (tr(cx, title), tr(cx, subtitle)),
                                icon,
                                is_over,
                                cx,
                            )
                        }),
                )
                .into_any_element(),
        )
    }
}

fn texts(zones: Zones, zone: Zone) -> (i18n::Key, i18n::Key, Icon) {
    match (zones, zone) {
        (Zones::Files, _) => (
            key::lng_drag_files_here,
            key::lng_drag_to_send_files,
            DRAG_ICON_FILES,
        ),
        (Zones::Photos, Zone::Documents) => (
            key::lng_drag_images_here,
            key::lng_drag_to_send_no_compression,
            DRAG_ICON_FILES,
        ),
        (Zones::Photos, Zone::Compressed) => (
            key::lng_drag_photos_here,
            key::lng_drag_to_send_quick,
            DRAG_ICON_MEDIA,
        ),
    }
}

/// A zone in the chat's coordinates (`DragArea::paintEvent`).
fn zone_element(
    bounds: Bounds<Pixels>,
    (title, subtitle): (gpui::SharedString, gpui::SharedString),
    icon: Icon,
    is_over: bool,
    cx: &App,
) -> AnyElement {
    let fg = if is_over { DRAG_DROP_COLOR } else { DRAG_COLOR };
    let height = bounds.size.height;
    let icon_height = icon_size(icon).height;
    let shows_icon = height >= minimal_height() + icon_height + px_of(DRAG_ICON_SKIP);
    let shift = if shows_icon {
        ((icon_height + px_of(DRAG_ICON_SKIP)) / 2.0).floor()
    } else {
        px(0.0)
    };
    let text_top = ((height - px_of(DRAG_HEIGHT)) / 2.0).floor() + shift;
    let inner_bounds = inner(bounds) - bounds.origin;
    let line = |text, top: Pixels, text_font: style::Font| {
        div()
            .absolute()
            .top(top)
            .left(px(0.0))
            .right(px(0.0))
            .flex()
            .justify_center()
            .font(font(text_font))
            .text_size(px_of(text_font.size))
            .line_height(font_height(text_font))
            .text_color(color(cx, fg))
            .child(text)
    };
    div()
        .absolute()
        .left(bounds.origin.x)
        .top(bounds.origin.y)
        .w(bounds.size.width)
        .h(height)
        .child(
            div()
                .absolute()
                .left(inner_bounds.origin.x)
                .top(inner_bounds.origin.y)
                .w(inner_bounds.size.width)
                .h(inner_bounds.size.height)
                .rounded(px_of(BOX_RADIUS))
                .bg(color(cx, ColorRef::Palette(ColorId::BoxBg)))
                // shortcut: one blurred shadow for boxRoundShadow's images, as the boxes draw it.
                .shadow(vec![BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.19),
                    offset: point(px(0.0), px(1.0)),
                    blur_radius: px(8.0),
                    spread_radius: px(0.0),
                    inset: false,
                }]),
        )
        .children(shows_icon.then(|| {
            div()
                .absolute()
                .top(text_top - px_of(DRAG_ICON_SKIP) - icon_height)
                .left(px(0.0))
                .right(px(0.0))
                .flex()
                .justify_center()
                .child(ui::icon(icon).color(fg))
        }))
        .child(line(title, text_top, DRAG_FONT))
        .child(line(
            subtitle,
            text_top + px_of(DRAG_HEIGHT) - font_height(DRAG_SUBFONT),
            DRAG_SUBFONT,
        ))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(height: f32) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(288.0), px(0.0)),
            size: gpui::size(px(512.0), px(height)),
        }
    }

    #[test]
    fn images_get_a_third_for_documents_and_the_rest_compressed() {
        let tall = chat(920.0);
        let zones = layout(Zones::Photos, tall);
        assert_eq!(zones[0].1.origin.y, px(10.0));
        assert_eq!(zones[0].1.size.height, px(300.0));
        assert_eq!(zones[1].1.origin.y, px(310.0));
        assert_eq!(zones[1].1.size.height, px(600.0));
        assert_eq!(
            zone_at(Zones::Photos, tall, point(px(500.0), px(500.0))),
            Some(Zone::Compressed)
        );
        // Between the zones' padding nothing takes the drop.
        assert_eq!(
            zone_at(Zones::Photos, tall, point(px(500.0), px(310.0))),
            None
        );
    }

    #[test]
    fn a_third_too_short_for_its_icon_gives_halves() {
        let zones = layout(Zones::Photos, chat(620.0));
        assert_eq!(zones[0].1.size.height, px(300.0));
        assert_eq!(zones[1].1.size.height, px(300.0));
    }

    #[test]
    fn other_files_get_one_zone() {
        assert_eq!(
            zones_for(&[PathBuf::from("a.png"), PathBuf::from("b.txt")]),
            Zones::Files
        );
        assert_eq!(zones_for(&[PathBuf::from("a.JPG")]), Zones::Photos);
        assert_eq!(layout(Zones::Files, chat(620.0)).len(), 1);
    }
}
