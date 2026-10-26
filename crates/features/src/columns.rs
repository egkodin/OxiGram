//! How the window splits into the chat list and the chat
//! (`SessionController::computeColumnLayout`), and the drag that resizes
//! the list (`MainWidget::ensureFirstColumnResizeAreaCreated`).

use gpui::{Context, IntoElement, Render, Window};
use style::dialogs::DEFAULT_DIALOG_ROW;
use style::window::{COLUMN_MINIMAL_WIDTH_LEFT, COLUMN_MINIMAL_WIDTH_MAIN};

/// The list's share of the window until the user drags it
/// (`kDefaultDialogsWidthRatio`).
pub(crate) const DEFAULT_RATIO: f32 = 5.0 / 14.0;

/// How the chat list shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Layout {
    /// The list fills the window; a chat would replace it.
    OneColumn,
    /// The list beside the chat, `width` wide.
    Wide { width: f32 },
    /// Userpics only, beside the chat.
    Narrow,
}

impl Layout {
    /// The list's width in a window `window_width` wide.
    pub(crate) fn list_width(self, window_width: f32) -> f32 {
        match self {
            Self::OneColumn => window_width,
            Self::Wide { width } => width,
            Self::Narrow => narrow_width(),
        }
    }
}

fn logical(value: style::Px) -> f32 {
    f32::from(crate::px_of(value))
}

/// Userpic and its padding on both sides (`_narrowWidth`).
pub(crate) fn narrow_width() -> f32 {
    logical(DEFAULT_DIALOG_ROW.padding.left) * 2.0 + logical(DEFAULT_DIALOG_ROW.photo_size)
}

/// The layout of a window `window_width` wide with the list at `ratio` of
/// it; a zero ratio is the narrow list.
pub(crate) fn layout(window_width: f32, ratio: f32) -> Layout {
    let min_left = logical(COLUMN_MINIMAL_WIDTH_LEFT);
    let min_main = logical(COLUMN_MINIMAL_WIDTH_MAIN);
    if window_width < min_left + min_main {
        Layout::OneColumn
    } else if ratio <= 0.0 {
        Layout::Narrow
    } else {
        let width = (window_width * ratio)
            .round()
            .max(min_left)
            .min(window_width - min_main);
        Layout::Wide { width }
    }
}

/// The ratio after dragging the list's edge to `left`: narrow below half
/// the minimal width.
pub(crate) fn ratio_for_drag(left: f32, window_width: f32) -> f32 {
    if left < logical(COLUMN_MINIMAL_WIDTH_LEFT) / 2.0 || window_width <= 0.0 {
        0.0
    } else {
        left / window_width
    }
}

/// The value of a column edge drag.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ColumnResize;

/// Nothing follows the pointer; the column itself moves.
pub(crate) struct ResizeGhost;

impl Render for ResizeGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lays_out_one_wide_or_narrow_column() {
        assert_eq!(layout(600.0, DEFAULT_RATIO), Layout::OneColumn);
        assert_eq!(layout(1400.0, DEFAULT_RATIO), Layout::Wide { width: 500.0 });
        // Never under the minimum, and the chat keeps its minimum.
        assert_eq!(layout(800.0, 0.1), Layout::Wide { width: 260.0 });
        assert_eq!(layout(800.0, 0.9), Layout::Wide { width: 420.0 });
        assert_eq!(layout(800.0, 0.0), Layout::Narrow);
        assert_eq!(Layout::Narrow.list_width(800.0), 66.0);
    }

    #[test]
    fn dragging_below_half_the_minimum_goes_narrow() {
        assert_eq!(ratio_for_drag(100.0, 1000.0), 0.0);
        assert!((ratio_for_drag(300.0, 1000.0) - 0.3).abs() < f32::EPSILON);
    }
}
