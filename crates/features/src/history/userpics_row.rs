//! Userpics overlapping left to right (`GenerateUserpicsInRow`), as under
//! comments and in reactions.

use gpui::{Div, Hsla, ParentElement as _, Styled as _, div, prelude::FluentBuilder as _};
use style::chat_helpers::GroupCallUserpics;
use ui::{Userpic, UserpicSource};

use crate::px_of;

/// The first userpic on top, each with a ring in `ring`, the background,
/// cut around it.
pub(super) fn userpics_row(
    userpics: Vec<UserpicSource>,
    st: &GroupCallUserpics,
    ring: Hsla,
) -> Div {
    let count = userpics.len();
    let (size, stroke) = (px_of(st.size), px_of(st.stroke));
    // Reversed so the first paints last, over the others.
    div().flex_none().flex().flex_row_reverse().children(
        userpics
            .into_iter()
            .enumerate()
            .rev()
            .map(move |(index, userpic)| {
                div()
                    .relative()
                    .size(size)
                    .when(index + 1 < count, |this| this.mr(-px_of(st.shift)))
                    .child(Userpic::new(size, userpic))
                    .child(
                        div()
                            .absolute()
                            .inset(-stroke / 2.0)
                            .rounded_full()
                            .border(stroke)
                            .border_color(ring),
                    )
            }),
    )
}
