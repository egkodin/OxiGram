//! Custom emoji inside a message's text (`Ui::Text::CustomEmoji`): the
//! fallback glyph keeps its place but goes transparent, and the emoji's
//! picture is painted over it once the text is laid out.

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, Bounds, Corners, Hsla, ImageAssetLoader, IntoElement as _, Resource, Styled as _,
    TextLayout, TextRun, canvas, point, size,
};
use model::CustomEmojiId;
use model::text::{EntityKind, FormattedText};
use style::basic::EMOJI_SIZE;

use super::HistoryView;
use crate::px_of;
use crate::store::EmojiKey;

/// The pictures of a text's custom emoji that are on disk.
pub(super) type Pictures = Rc<HashMap<CustomEmojiId, String>>;

impl HistoryView {
    /// Asks for the pictures of `text`'s custom emoji; the ones on disk.
    // shortcut: animated custom emoji show their still; tdesktop plays them, which comes with a Lottie player per emoji.
    pub(super) fn emoji_pictures(
        &self,
        text: Option<&FormattedText>,
        cx: &mut gpui::Context<Self>,
    ) -> Pictures {
        let ids = text.into_iter().flat_map(|text| {
            text.entities.iter().filter_map(|entity| match entity.kind {
                EntityKind::CustomEmoji { id } => Some(id),
                _ => None,
            })
        });
        let ids: Vec<CustomEmojiId> = ids.collect();
        let pictures = self.store.update(cx, |store, cx| {
            ids.into_iter()
                .filter_map(|id| Some((id, store.emoji_picture(&EmojiKey::Custom(id), cx)?)))
                .collect()
        });
        Rc::new(pictures)
    }
}

/// Where each emoji with a picture sits in `range`, from 0, and its file.
pub(super) fn places(
    text: &FormattedText,
    range: &Range<usize>,
    pictures: &Pictures,
) -> Vec<(Range<usize>, String)> {
    text.entities
        .iter()
        .filter(|entity| range.start <= entity.range.start && entity.range.end <= range.end)
        .filter_map(|entity| match entity.kind {
            EntityKind::CustomEmoji { id } => {
                let path = pictures.get(&id)?.clone();
                Some((
                    entity.range.start - range.start..entity.range.end - range.start,
                    path,
                ))
            }
            _ => None,
        })
        .collect()
}

/// `runs` with the bytes in `hidden` drawn transparent, runs split at
/// their edges.
pub(super) fn hide(runs: Vec<TextRun>, hidden: &[Range<usize>]) -> Vec<TextRun> {
    if hidden.is_empty() {
        return runs;
    }
    let mut result = Vec::with_capacity(runs.len() + hidden.len() * 2);
    let mut start = 0;
    for run in runs {
        let end = start + run.len;
        let mut cuts: Vec<usize> = hidden
            .iter()
            .flat_map(|range| [range.start, range.end])
            .filter(|cut| start < *cut && *cut < end)
            .collect();
        cuts.sort_unstable();
        cuts.dedup();
        let mut from = start;
        for to in cuts.into_iter().chain(std::iter::once(end)) {
            let is_hidden = hidden.iter().any(|range| range.contains(&from));
            result.push(TextRun {
                len: to - from,
                color: if is_hidden {
                    Hsla::transparent_black()
                } else {
                    run.color
                },
                ..run.clone()
            });
            from = to;
        }
        start = end;
    }
    result
}

/// The pictures over the laid-out text.
pub(super) fn overlay(layout: TextLayout, places: Vec<(Range<usize>, String)>) -> AnyElement {
    canvas(
        |_, _, _| (),
        move |_, (), window, cx| {
            let side = px_of(EMOJI_SIZE);
            let line_height = layout.line_height();
            for (range, path) in &places {
                let (Some(start), Some(end)) = (
                    layout.position_for_index(range.start),
                    layout.position_for_index(range.end),
                ) else {
                    continue;
                };
                // A glyph cut by a line break keeps its start.
                let width = if end.y == start.y {
                    end.x - start.x
                } else {
                    side
                };
                let at = point(
                    start.x + (width - side) / 2.0,
                    start.y + (line_height - side) / 2.0,
                );
                let source = Resource::Path(Arc::from(Path::new(path)));
                let Some(Ok(image)) = window.use_asset::<ImageAssetLoader>(&source, cx) else {
                    continue;
                };
                let square = Bounds::new(at, size(side, side));
                if let Err(error) =
                    window.paint_image(square, square, Corners::default(), image, 0, false)
                {
                    tracing::warn!(%error, "painting a custom emoji failed");
                }
            }
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(len: usize) -> TextRun {
        TextRun {
            len,
            font: gpui::font("Helvetica"),
            color: gpui::white(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    #[test]
    fn hidden_bytes_split_the_runs() {
        let runs = hide(vec![run(10), run(5)], &[3..5, 9..12]);
        let shape: Vec<(usize, bool)> = runs
            .iter()
            .map(|run| (run.len, run.color == Hsla::transparent_black()))
            .collect();
        assert_eq!(
            shape,
            [
                (3, false),
                (2, true),
                (4, false),
                (1, true),
                (2, true),
                (3, false)
            ]
        );
    }
}
