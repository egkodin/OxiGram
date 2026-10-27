//! The field's text shaped into rows: paragraphs between newlines, each
//! wrapped to the field's width.

use std::ops::Range;

use gpui::{Pixels, Point, SharedString, TextRun, Window, WrappedLine, point, px};

/// The sizes the text sits in (`InitMessageFieldGeometry`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Metrics {
    pub(super) font_size: Pixels,
    pub(super) line_height: Pixels,
    /// Around the text on every side (`setDocumentMargin`).
    pub(super) margin: Pixels,
    pub(super) min_height: Pixels,
    pub(super) max_height: Pixels,
}

/// Room for the cursor after the longest row (`updateRootFrameFormat`).
const CURSOR_ROOM: f32 = 2.0;

impl Metrics {
    pub(super) fn wrap_width(&self, width: Pixels) -> Pixels {
        (width - self.margin * 2.0 - px(CURSOR_ROOM)).max(px(1.0))
    }

    /// The field's height for `rows` of text: it grows with them up to the
    /// maximum, then scrolls.
    pub(super) fn height(&self, rows: usize) -> Pixels {
        #[expect(clippy::cast_precision_loss, reason = "row counts are small")]
        let content = self.line_height * rows as f32 + self.margin * 2.0;
        content.clamp(self.min_height, self.max_height.max(self.min_height))
    }

    /// How far the text may scroll inside a field of `height`.
    pub(super) fn max_scroll(&self, rows: usize, height: Pixels) -> Pixels {
        #[expect(clippy::cast_precision_loss, reason = "row counts are small")]
        let content = self.line_height * rows as f32 + self.margin * 2.0;
        (content - height).max(px(0.0))
    }
}

#[derive(Debug)]
pub(super) struct Layout {
    lines: Vec<WrappedLine>,
    /// Where each paragraph starts in the text.
    starts: Vec<usize>,
    /// The visual row each paragraph starts on.
    first_rows: Vec<usize>,
    rows: usize,
    pub(super) line_height: Pixels,
}

impl Layout {
    pub(super) fn shape(
        text: &str,
        runs: &[TextRun],
        metrics: Metrics,
        width: Pixels,
        window: &Window,
    ) -> Self {
        let lines: Vec<WrappedLine> = window
            .text_system()
            .shape_text(
                SharedString::from(text.to_owned()),
                metrics.font_size,
                runs,
                Some(metrics.wrap_width(width)),
                None,
            )
            .map(Vec::from_iter)
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "cannot shape a field's text");
                Vec::new()
            });
        let mut starts = Vec::with_capacity(lines.len());
        let mut first_rows = Vec::with_capacity(lines.len());
        let (mut start, mut rows) = (0, 0);
        for line in &lines {
            starts.push(start);
            first_rows.push(rows);
            start += line.len() + 1;
            rows += line.wrap_boundaries().len() + 1;
        }
        Self {
            lines,
            starts,
            first_rows,
            rows: rows.max(1),
            line_height: metrics.line_height,
        }
    }

    pub(super) fn rows(&self) -> usize {
        self.rows
    }

    pub(super) fn lines(&self) -> impl Iterator<Item = (&WrappedLine, Pixels)> {
        self.lines
            .iter()
            .zip(&self.first_rows)
            .map(|(line, row)| (line, self.row_top(*row)))
    }

    fn row_top(&self, row: usize) -> Pixels {
        #[expect(clippy::cast_precision_loss, reason = "row counts are small")]
        let top = self.line_height * row as f32;
        top
    }

    fn paragraph(&self, offset: usize) -> usize {
        self.starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1)
    }

    /// Where `offset` is drawn, from the text's top left.
    pub(super) fn position(&self, offset: usize) -> Point<Pixels> {
        let paragraph = self.paragraph(offset);
        let (Some(line), Some(start)) = (self.lines.get(paragraph), self.starts.get(paragraph))
        else {
            return point(px(0.0), px(0.0));
        };
        let local = line
            .position_for_index(offset - start, self.line_height)
            .unwrap_or_default();
        point(local.x, local.y + self.row_top(self.first_rows[paragraph]))
    }

    pub(super) fn row_of(&self, offset: usize) -> usize {
        // Rounded: positions are whole rows apart.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a row index from a non-negative position"
        )]
        let row = (self.position(offset).y / self.line_height)
            .round()
            .max(0.0) as usize;
        row
    }

    /// The offset nearest to `position`, from the text's top left.
    pub(super) fn index_at(&self, position: Point<Pixels>) -> usize {
        if self.lines.is_empty() {
            return 0;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a row index from a clamped position"
        )]
        let row = ((position.y / self.line_height).floor().max(0.0) as usize).min(self.rows - 1);
        let paragraph = self
            .first_rows
            .partition_point(|first| *first <= row)
            .saturating_sub(1);
        let local = point(
            position.x.max(px(0.0)),
            self.row_top(row - self.first_rows[paragraph]) + self.line_height / 2.0,
        );
        let index = self.lines[paragraph]
            .closest_index_for_position(local, self.line_height)
            .unwrap_or_else(|index| index);
        self.starts[paragraph] + index
    }

    /// The text on visual `row`.
    pub(super) fn row_range(&self, row: usize) -> Range<usize> {
        let paragraph = self
            .first_rows
            .partition_point(|first| *first <= row)
            .saturating_sub(1);
        let Some(line) = self.lines.get(paragraph) else {
            return 0..0;
        };
        let ends: Vec<usize> = line
            .wrap_boundaries()
            .iter()
            .map(|boundary| line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index)
            .chain([line.len()])
            .collect();
        let local = row - self.first_rows[paragraph];
        let start = if local == 0 { 0 } else { ends[local - 1] };
        let end = ends.get(local).copied().unwrap_or(line.len());
        self.starts[paragraph] + start..self.starts[paragraph] + end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics() -> Metrics {
        Metrics {
            font_size: px(13.0),
            line_height: px(18.0),
            margin: px(4.0),
            min_height: px(28.0),
            max_height: px(224.0),
        }
    }

    #[test]
    fn grows_with_rows_up_to_the_maximum() {
        let metrics = metrics();
        assert_eq!(metrics.height(1), px(28.0));
        assert_eq!(metrics.height(3), px(62.0));
        assert_eq!(metrics.height(40), px(224.0));
    }

    #[test]
    fn scrolls_only_past_the_maximum() {
        let metrics = metrics();
        assert_eq!(metrics.max_scroll(3, metrics.height(3)), px(0.0));
        assert_eq!(metrics.max_scroll(20, px(224.0)), px(368.0 - 224.0));
    }
}
