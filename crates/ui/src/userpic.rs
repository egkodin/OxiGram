//! Round userpics: a downloaded photo, its blurred preview, or tdesktop's
//! empty userpic with initials on a colored gradient (`EmptyUserpic`).

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    App, Bounds, Image, ImageFormat, IntoElement, ParentElement, PathBuilder, Pixels, RenderOnce,
    SharedString, Styled, Window, canvas, div, img, linear_color_stop, linear_gradient, point, px,
};
use style::{ColorId, ColorRef};

use crate::theme::color;

/// What a userpic shows.
#[derive(Debug, Clone)]
pub enum UserpicSource {
    /// A local image file.
    File(PathBuf),
    /// A JPEG in memory, such as a minithumbnail.
    Jpeg(Arc<Image>),
    /// Initials on the color for `color_index` (TDLib's accent color).
    Empty {
        color_index: i32,
        name: SharedString,
    },
    /// The bookmark of Saved Messages.
    SavedMessages,
    /// The box of the archive folder (`Folder::paintUserpic`).
    Archive,
}

#[derive(Debug, Clone, IntoElement)]
pub struct Userpic {
    size: Pixels,
    source: UserpicSource,
}

/// tdesktop's palette order of the seven built-in colors
/// (`ColorIndexToPaletteIndex`).
const PALETTE_ORDER: [usize; 7] = [0, 7, 4, 1, 6, 3, 5];

const BACKGROUNDS: [(ColorId, ColorId); 8] = [
    (
        ColorId::HistoryPeer1UserpicBg,
        ColorId::HistoryPeer1UserpicBg2,
    ),
    (
        ColorId::HistoryPeer2UserpicBg,
        ColorId::HistoryPeer2UserpicBg2,
    ),
    (
        ColorId::HistoryPeer3UserpicBg,
        ColorId::HistoryPeer3UserpicBg2,
    ),
    (
        ColorId::HistoryPeer4UserpicBg,
        ColorId::HistoryPeer4UserpicBg2,
    ),
    (
        ColorId::HistoryPeer5UserpicBg,
        ColorId::HistoryPeer5UserpicBg2,
    ),
    (
        ColorId::HistoryPeer6UserpicBg,
        ColorId::HistoryPeer6UserpicBg2,
    ),
    (
        ColorId::HistoryPeer7UserpicBg,
        ColorId::HistoryPeer7UserpicBg2,
    ),
    (
        ColorId::HistoryPeer8UserpicBg,
        ColorId::HistoryPeer8UserpicBg2,
    ),
];

impl Userpic {
    #[must_use]
    pub fn new(size: Pixels, source: UserpicSource) -> Self {
        Self { size, source }
    }
}

/// A JPEG in memory, such as a minithumbnail, ready for `img`.
#[must_use]
pub fn jpeg_image(bytes: Vec<u8>) -> Arc<Image> {
    Arc::new(Image::from_bytes(ImageFormat::Jpeg, bytes))
}

/// Up to two initials: the first letter, and the first letter of the
/// last word, or after a hyphen (`EmptyUserpic::fillString`).
#[must_use]
pub fn initials(name: &str) -> String {
    let mut letters: Vec<(char, u8)> = Vec::new();
    let mut level = 0;
    let mut found = false;
    for ch in name.chars() {
        if !found && ch.is_alphanumeric() {
            found = true;
            letters.push((ch, level));
        } else if ch == ' ' {
            level = 0;
            found = false;
        } else if found && ch == '-' {
            level = 1;
        }
    }
    let Some(&(first, _)) = letters.first() else {
        return String::new();
    };
    let mut best = (0, 2);
    for (index, &(_, level)) in letters.iter().enumerate().skip(1).rev() {
        if level < best.1 {
            best = (index, level);
        }
    }
    let mut result: String = first.to_uppercase().collect();
    if best.0 > 0 {
        result.extend(letters[best.0].0.to_uppercase());
    }
    result
}

/// Which of the eight `historyPeerN` colors TDLib's accent color index
/// picks, from 0 (`ColorIndexToPaletteIndex`).
#[must_use]
pub fn peer_palette_index(color_index: i32) -> usize {
    PALETTE_ORDER[usize::try_from(color_index).unwrap_or(0) % PALETTE_ORDER.len()]
}

fn backgrounds(color_index: i32) -> (ColorRef, ColorRef) {
    let (top, bottom) = BACKGROUNDS[peer_palette_index(color_index)];
    (ColorRef::Palette(top), ColorRef::Palette(bottom))
}

fn gradient(top: ColorRef, bottom: ColorRef, cx: &App) -> gpui::Background {
    linear_gradient(
        180.0,
        linear_color_stop(color(cx, top), 0.0),
        linear_color_stop(color(cx, bottom), 1.0),
    )
}

impl RenderOnce for Userpic {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let size = self.size;
        let circle = div()
            .size(size)
            .flex_none()
            .rounded_full()
            .overflow_hidden();
        match self.source {
            UserpicSource::File(path) => {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a userpic is a few hundred pixels"
                )]
                let side = (f32::from(size) * window.scale_factor()).round() as u32;
                match crate::round_image::round_file(&path, side, cx) {
                    Some(round) => circle.child(img(round).size(size)),
                    None => circle.child(img(path).size(size).rounded_full()),
                }
            }
            // The preview is 40 px or less, so it only grows; gpui clips to rectangles.
            UserpicSource::Jpeg(image) => circle.child(img(image).size(size).rounded_full()),
            UserpicSource::Empty { color_index, name } => {
                let (top, bottom) = backgrounds(color_index);
                circle
                    .bg(gradient(top, bottom, cx))
                    .flex()
                    .items_center()
                    .justify_center()
                    .font(crate::font::font(style::basic::SEMIBOLD_FONT))
                    // `EmptyUserpic::paint`: a font of 13/33 of the size.
                    .text_size(size * (13.0 / 33.0))
                    .text_color(color(cx, ColorRef::Palette(ColorId::HistoryPeerUserpicFg)))
                    .child(initials(&name))
            }
            // shortcut: the icon keeps its chat list size; scale it when a smaller archive userpic shows (narrow mode, M2).
            UserpicSource::Archive => circle
                .bg(color(
                    cx,
                    ColorRef::Palette(ColorId::HistoryPeerArchiveUserpicBg),
                ))
                .flex()
                .items_center()
                .justify_center()
                .child(crate::icon::icon(style::dialogs::DIALOGS_ARCHIVE_USERPIC)),
            UserpicSource::SavedMessages => {
                let fg = color(cx, ColorRef::Palette(ColorId::HistoryPeerUserpicFg));
                circle
                    .bg(gradient(
                        ColorRef::Palette(ColorId::HistoryPeerSavedMessagesBg),
                        ColorRef::Palette(ColorId::HistoryPeerSavedMessagesBg2),
                        cx,
                    ))
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, (), window, _| {
                                paint_bookmark(bounds, fg, window);
                            },
                        )
                        .size(size),
                    )
            }
        }
    }
}

/// `PaintSavedMessagesInner`: a bookmark outline in the middle.
fn paint_bookmark(bounds: Bounds<Pixels>, fg: gpui::Hsla, window: &mut Window) {
    let size = f32::from(bounds.size.width);
    let thickness = (size * 0.055).round();
    let width = (size * 0.15).round() * 2.0;
    let height = (size * 0.19).round() * 2.0;
    let add = (size * 0.064).round();
    let left = f32::from(bounds.origin.x) + (size - width) / 2.0;
    let top = f32::from(bounds.origin.y) + (size - height) / 2.0;
    let (right, bottom) = (left + width, top + height);
    let middle = (left + right) / 2.0;
    let corners = [
        (left, bottom),
        (left, top),
        (right, top),
        (right, bottom),
        (middle, bottom - add),
    ];
    let points: Vec<_> = corners.iter().map(|&(x, y)| point(px(x), px(y))).collect();
    let mut builder = PathBuilder::stroke(px(thickness));
    builder.add_polygon(&points, true);
    if let Ok(path) = builder.build() {
        window.paint_path(path, fg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_first_letters_of_the_first_and_last_words() {
        assert_eq!(initials("Ann Lee"), "AL");
        assert_eq!(initials("ann marie lee"), "AL");
        // tdesktop keeps looking past a hyphen only for a new word.
        assert_eq!(initials("Jean-Luc"), "J");
        assert_eq!(initials("  rgram"), "R");
        assert_eq!(initials("😀 Club"), "C");
        assert_eq!(initials(""), "");
    }
}
