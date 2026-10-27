//! lib_ui's `BoxLayerWidget` frame: a rounded box with an optional title,
//! rows of content and a row of buttons at the bottom right, over a dimmed
//! window (`LayerStackWidget`).

use gpui::{
    AnyElement, App, BoxShadow, ElementId, IntoElement, ParentElement, Pixels, RenderOnce,
    SharedString, Styled, Window, div, point, prelude::*, px,
};
use style::layers::{
    BOX_RADIUS, BOX_TITLE, BOX_TITLE_HEIGHT, BOX_TITLE_POSITION, BOX_TOP_MARGIN, BOX_WIDTH,
    Box as BoxStyle,
};

use crate::button::RoundButton;
use crate::label::{FlatLabel, Markup};
use crate::theme::{color, hsla};

/// A box: its title, content rows and buttons. The first button is the
/// rightmost, as `addButton` orders them.
#[derive(IntoElement)]
pub struct BoxFrame {
    id: ElementId,
    style: &'static BoxStyle,
    width: Pixels,
    title: Option<SharedString>,
    content: Vec<AnyElement>,
    buttons: Vec<RoundButton>,
}

impl std::fmt::Debug for BoxFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoxFrame")
            .field("id", &self.id)
            .field("title", &self.title)
            .finish_non_exhaustive()
    }
}

impl BoxFrame {
    pub fn new(id: impl Into<ElementId>, style: &'static BoxStyle) -> Self {
        Self {
            id: id.into(),
            style,
            width: px_of(BOX_WIDTH.0),
            title: None,
            content: Vec::new(),
            buttons: Vec::new(),
        }
    }

    #[must_use]
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Adds a content row; rows bring their own padding (`addRow`).
    #[must_use]
    pub fn row(mut self, row: impl IntoElement) -> Self {
        self.content.push(row.into_any_element());
        self
    }

    #[must_use]
    pub fn button(mut self, button: RoundButton) -> Self {
        self.buttons.push(button);
        self
    }
}

impl RenderOnce for BoxFrame {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let st = self.style;
        // As tall as the window leaves inside the style's margin (`updateMaxRealHeight`).
        let max_height =
            window.viewport_size().height - px_of(st.margin.top.0) - px_of(st.margin.bottom.0);
        let padding = st.button_padding;
        let has_buttons = !self.buttons.is_empty();
        let title = self.title.map(|title| {
            div().relative().h(px_of(BOX_TITLE_HEIGHT.0)).child(
                div()
                    .absolute()
                    .left(px_of(BOX_TITLE_POSITION.x.0))
                    .top(px_of(BOX_TITLE_POSITION.y.0))
                    .child(FlatLabel::new(
                        "box-title",
                        &BOX_TITLE,
                        Markup::plain(title.to_string()),
                    )),
            )
        });
        let has_title = title.is_some();
        div()
            .id(self.id)
            .w(self.width)
            .max_h(max_height)
            .flex()
            .flex_col()
            .rounded(px_of(BOX_RADIUS.0))
            .bg(color(cx, st.bg))
            // shortcut: one blurred shadow for boxRoundShadow's images; draw those if the edge differs.
            .shadow(vec![BoxShadow {
                color: hsla(style::Color::rgba(0, 0, 0, 0x30)),
                offset: point(px(0.0), px(1.0)),
                blur_radius: px(8.0),
                spread_radius: px(0.0),
                inset: false,
            }])
            // A box swallows clicks, so they do not close the layer.
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(title)
            .when(!has_title, |frame| frame.pt(px_of(BOX_TOP_MARGIN.0)))
            // Rows taller than the window scroll between the title and the buttons.
            .child(
                div()
                    .id("box-content")
                    .flex_shrink_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(self.content),
            )
            .when(has_buttons, |frame| {
                frame.child(
                    div()
                        .flex()
                        .flex_row_reverse()
                        .gap(px_of(padding.left.0))
                        .pt(px_of(padding.top.0))
                        .pr(px_of(padding.right.0))
                        .pb(px_of(padding.bottom.0))
                        .children(self.buttons),
                )
            })
    }
}

fn px_of(value: i32) -> Pixels {
    #[expect(clippy::cast_precision_loss, reason = "style sizes are small")]
    px(value as f32)
}
