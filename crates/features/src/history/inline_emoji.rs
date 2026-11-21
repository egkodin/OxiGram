//! Custom emoji inside a message's text (`Ui::Text::CustomEmoji`): the
//! fallback glyph keeps its place but goes transparent, and the emoji's
//! picture is painted over it once the text is laid out.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, Bounds, Corners, Hsla, ImageAssetLoader, IntoElement as _, RenderImage, Resource,
    Styled as _, TextLayout, TextRun, canvas, point, size,
};
use model::CustomEmojiId;
use model::text::{EntityKind, FormattedText};
use style::basic::EMOJI_SIZE;

use super::HistoryView;
use crate::px_of;
use crate::store::EmojiKey;

#[derive(Clone)]
pub(super) enum Picture {
    Frame(Arc<RenderImage>),
    Still(String),
}

pub(super) type Pictures = Rc<HashMap<CustomEmojiId, Picture>>;

impl HistoryView {
    /// Loads the artwork once per distinct custom emoji in the message.
    pub(super) fn emoji_pictures(
        &mut self,
        text: Option<&FormattedText>,
        cx: &mut gpui::Context<Self>,
    ) -> Pictures {
        let ids: HashSet<_> = text
            .into_iter()
            .flat_map(|text| {
                text.entities.iter().filter_map(|entity| match entity.kind {
                    EntityKind::CustomEmoji { id } => Some(id),
                    _ => None,
                })
            })
            .collect();
        let side: f32 = px_of(EMOJI_SIZE).into();
        Rc::new(
            ids.into_iter()
                .filter_map(|id| {
                    let picture = self
                        .custom_emoji(id, side, cx)
                        .map(Picture::Frame)
                        .or_else(|| {
                            self.store
                                .update(cx, |store, cx| {
                                    store.emoji_picture(&EmojiKey::Custom(id), cx)
                                })
                                .map(Picture::Still)
                        })?;
                    Some((id, picture))
                })
                .collect(),
        )
    }
}

/// Where each emoji with artwork sits in `range`, from 0.
pub(super) fn places(
    text: &FormattedText,
    range: &Range<usize>,
    pictures: &Pictures,
) -> Vec<(Range<usize>, Picture)> {
    text.entities
        .iter()
        .filter(|entity| range.start <= entity.range.start && entity.range.end <= range.end)
        .filter_map(|entity| match entity.kind {
            EntityKind::CustomEmoji { id } => {
                let picture = pictures.get(&id)?.clone();
                Some((
                    entity.range.start - range.start..entity.range.end - range.start,
                    picture,
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
pub(super) fn overlay(layout: TextLayout, places: Vec<(Range<usize>, Picture)>) -> AnyElement {
    canvas(
        |_, _, _| (),
        move |_, (), window, cx| {
            let side = px_of(EMOJI_SIZE);
            let line_height = layout.line_height();
            for (range, picture) in &places {
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
                let image = match picture {
                    Picture::Frame(image) => image.clone(),
                    Picture::Still(path) => {
                        let source = Resource::Path(Arc::from(Path::new(path)));
                        let Some(Ok(image)) = window.use_asset::<ImageAssetLoader>(&source, cx)
                        else {
                            continue;
                        };
                        image
                    }
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
