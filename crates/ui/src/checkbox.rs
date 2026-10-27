//! lib_ui's `Checkbox` with a `CheckView` (a rounded square) or a
//! `RadioView` (a ring), and a label.

use std::rc::Rc;

use gpui::{
    App, ElementId, IntoElement, ParentElement, Pixels, RenderOnce, SharedString, Window, div,
    prelude::*,
};
use style::ColorRef;
use style::basic::ROUND_RADIUS_SMALL;
use style::widgets::{Check, Checkbox as CheckboxStyle, Radio};

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

/// What sits left of the label.
#[derive(Debug, Clone, Copy)]
enum Mark {
    Check(&'static Check),
    Radio(&'static Radio),
}

/// A checkbox or a radio button; the owner keeps the state and flips it in
/// `on_toggle`.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    style: &'static CheckboxStyle,
    mark: Mark,
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
        Self::with_mark(id.into(), style, Mark::Check(check), text.into(), checked)
    }

    /// One choice of a group (`Radioenum`); the owner unticks the others.
    pub fn radio(
        id: impl Into<ElementId>,
        style: &'static CheckboxStyle,
        radio: &'static Radio,
        text: impl Into<SharedString>,
        checked: Checked,
    ) -> Self {
        Self::with_mark(id.into(), style, Mark::Radio(radio), text.into(), checked)
    }

    fn with_mark(
        id: ElementId,
        style: &'static CheckboxStyle,
        mark: Mark,
        text: SharedString,
        checked: Checked,
    ) -> Self {
        Self {
            id,
            style,
            mark,
            text,
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

impl Checkbox {
    fn square(&self, check: &'static Check, cx: &App) -> gpui::Div {
        let diameter = px_of(check.diameter);
        let is_checked = self.checked == Checked::Yes;
        let border = if is_checked {
            check.toggled_fg
        } else {
            self.untoggled_override.unwrap_or(check.untoggled_fg)
        };
        div()
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
            })
    }

    /// A ring inset by `outer_skip`, and a dot inset by `skip` when ticked
    /// (both in tenths of a pixel, as `RadioView::paint` reads them).
    // shortcut: no toggle animation, as the checkbox has none either; add both together.
    fn ring(&self, radio: &'static Radio, cx: &App) -> gpui::Div {
        let diameter = px_of(radio.diameter);
        let outer = gpui::px(tenths(radio.outer_skip));
        let dot = diameter - gpui::px(tenths(radio.skip)) * 2.0;
        let is_checked = self.checked == Checked::Yes;
        let fg = if is_checked {
            radio.toggled_fg
        } else {
            self.untoggled_override.unwrap_or(radio.untoggled_fg)
        };
        div().size(diameter).flex_none().p(outer).child(
            div()
                .size_full()
                .rounded_full()
                .border(px_of(radio.thickness))
                .border_color(color(cx, fg))
                .bg(color(cx, radio.bg))
                .flex()
                .items_center()
                .justify_center()
                .when(is_checked, |ring| {
                    ring.child(div().size(dot).rounded_full().bg(color(cx, fg)))
                }),
        )
    }
}

fn tenths(value: style::Px) -> f32 {
    #[expect(clippy::cast_precision_loss, reason = "style sizes are small")]
    let value = value.0 as f32;
    value / 10.0
}

impl RenderOnce for Checkbox {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let st = self.style;
        let square = match self.mark {
            Mark::Check(check) => self.square(check, cx),
            Mark::Radio(radio) => self.ring(radio, cx),
        };
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
