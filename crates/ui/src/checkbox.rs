//! lib_ui's `Checkbox` with a `CheckView`: a rounded square and a label.

use std::rc::Rc;

use gpui::{
    App, ElementId, IntoElement, ParentElement, Pixels, RenderOnce, SharedString, Window, div,
    prelude::*,
};
use style::ColorRef;
use style::basic::ROUND_RADIUS_SMALL;
use style::widgets::{Check, Checkbox as CheckboxStyle};

use crate::font::{font, font_height};
use crate::icon::icon;
use crate::theme::color;

type ToggleHandler = Rc<dyn Fn(&mut Window, &mut App)>;

fn px_of(value: style::Px) -> Pixels {
    #[expect(clippy::cast_precision_loss, reason = "style sizes are small")]
    gpui::px(value.0 as f32)
}

/// Whether the box is ticked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checked {
    Yes,
    No,
}

/// A checkbox; the owner keeps the state and flips it in `on_toggle`.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    style: &'static CheckboxStyle,
    check: &'static Check,
    text: SharedString,
    checked: Checked,
    /// Paints the empty box in this color (`setUntoggledOverride`).
    untoggled_override: Option<ColorRef>,
    on_toggle: Option<ToggleHandler>,
}

impl Checkbox {
    pub fn new(
        id: impl Into<ElementId>,
        style: &'static CheckboxStyle,
        check: &'static Check,
        text: impl Into<SharedString>,
        checked: Checked,
    ) -> Self {
        Self {
            id: id.into(),
            style,
            check,
            text: text.into(),
            checked,
            untoggled_override: None,
            on_toggle: None,
        }
    }

    #[must_use]
    pub fn untoggled_override(mut self, color: Option<ColorRef>) -> Self {
        self.untoggled_override = color;
        self
    }

    #[must_use]
    pub fn on_toggle(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl std::fmt::Debug for Checkbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Checkbox")
            .field("id", &self.id)
            .field("checked", &self.checked)
            .finish_non_exhaustive()
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let st = self.style;
        let check = self.check;
        let diameter = px_of(check.diameter);
        let is_checked = self.checked == Checked::Yes;
        let border = if is_checked {
            check.toggled_fg
        } else {
            self.untoggled_override.unwrap_or(check.untoggled_fg)
        };
        let square = div()
            .size(diameter)
            .flex_none()
            .rounded(px_of(ROUND_RADIUS_SMALL))
            .border(px_of(check.thickness))
            .border_color(color(cx, border))
            .flex()
            .items_center()
            .justify_center()
            .map(|square| {
                if is_checked {
                    square
                        .bg(color(cx, check.toggled_fg))
                        .child(icon(check.icon))
                } else {
                    square.bg(color(cx, check.bg))
                }
            });
        let on_toggle = self.on_toggle;
        let text_font = st.style.font;
        div()
            .id(self.id)
            .flex()
            .items_start()
            // The check sits at `check_position` inside the margins, so at the origin.
            .cursor_pointer()
            .child(square)
            .child(
                div()
                    .pl(px_of(st.text_position.x))
                    .pt(px_of(st.text_position.y))
                    .font(font(text_font))
                    .text_size(px_of(text_font.size))
                    .line_height(font_height(text_font))
                    .text_color(color(cx, st.text_fg))
                    .child(self.text),
            )
            .on_click(move |_, window, cx| {
                if let Some(on_toggle) = &on_toggle {
                    on_toggle(window, cx);
                }
            })
    }
}
