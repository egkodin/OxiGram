//! lib_ui's `SettingsButton`: a full-width row with an icon, a label and an
//! optional toggle, as in the main menu and the settings sections.

use std::rc::Rc;

use gpui::{
    App, Bounds, ClickEvent, CursorStyle, ElementId, IntoElement, MouseButton, ParentElement,
    Pixels, RenderOnce, SharedString, Styled, Window, canvas, div, point, prelude::*, px, size,
};
use style::widgets::{SettingsButton as SettingsStyle, Toggle as ToggleStyle};
use style::{ColorId, ColorRef};

use crate::button::{hover_listener, press_listener, px_of, release_listener};
use crate::font::{font, font_height};
use crate::icon::{icon, icon_size};
use crate::ripple::{self, Shape, use_button_state};
use crate::theme::color;

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct SettingsButton {
    id: ElementId,
    style: &'static SettingsStyle,
    text: SharedString,
    icon: Option<style::Icon>,
    /// The value at the right (`CreateRightLabel`).
    label: Option<SharedString>,
    /// `Some` draws a toggle in that state at the right.
    toggled: Option<bool>,
    on_click: Option<ClickHandler>,
}

impl SettingsButton {
    pub fn new(
        id: impl Into<ElementId>,
        style: &'static SettingsStyle,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style,
            text: text.into(),
            icon: None,
            label: None,
            toggled: None,
            on_click: None,
        }
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn icon(mut self, icon: style::Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    #[must_use]
    pub fn toggle(mut self, toggled: bool) -> Self {
        self.toggled = Some(toggled);
        self
    }

    #[must_use]
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl std::fmt::Debug for SettingsButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsButton")
            .field("id", &self.id)
            .field("text", &self.text)
            .finish_non_exhaustive()
    }
}

impl RenderOnce for SettingsButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let st = self.style;
        let state = use_button_state(self.id.clone(), window, cx);
        let hovered = state.read(cx).hovered;
        let height = px_of(st.padding.top) + px_of(st.height) + px_of(st.padding.bottom);
        let text_font = st.style.font;
        let on_click = self.on_click.clone();
        let toggle = if hovered { &st.toggle_over } else { &st.toggle };
        div()
            .id(self.id)
            .relative()
            .w_full()
            .h(height)
            .cursor(CursorStyle::PointingHand)
            .bg(color(
                cx,
                if hovered { st.text_bg_over } else { st.text_bg },
            ))
            .child(div().absolute().size_full().child(ripple::layer(
                &state,
                st.ripple,
                Shape::RoundRect { radius: px(0.0) },
            )))
            .when_some(self.icon, |this, shown| {
                let top = (height - icon_size(shown).height) / 2.0;
                let view = if hovered {
                    icon(shown).color(ColorRef::Palette(ColorId::MenuIconFgOver))
                } else {
                    icon(shown)
                };
                this.child(
                    div()
                        .absolute()
                        .left(px_of(st.icon_left))
                        .top(top)
                        .child(view),
                )
            })
            .child(
                div()
                    .absolute()
                    .left(px_of(st.padding.left))
                    .top(px_of(st.padding.top))
                    .right(px_of(st.padding.right))
                    .truncate()
                    .font(font(text_font))
                    .text_size(px_of(text_font.size))
                    .line_height(font_height(text_font))
                    .text_color(color(
                        cx,
                        if hovered { st.text_fg_over } else { st.text_fg },
                    ))
                    .child(self.text),
            )
            .when_some(self.label, |this, label| {
                let right = &st.right_label;
                let label_font = right.style.font;
                this.child(
                    div()
                        .absolute()
                        .right(px_of(st.padding.right))
                        .top(px_of(st.padding.top))
                        .font(font(label_font))
                        .text_size(px_of(label_font.size))
                        .line_height(font_height(label_font))
                        .text_color(color(cx, right.text_fg))
                        .child(label),
                )
            })
            .when_some(self.toggled, |this, toggled| {
                let toggle_size = toggle_size(toggle);
                this.child(
                    div()
                        .absolute()
                        .right(px_of(st.toggle_skip))
                        .top((height - toggle_size.height) / 2.0)
                        .child(toggle_view(toggle, toggled, cx)),
                )
            })
            .on_hover(hover_listener(&state))
            .on_mouse_down(MouseButton::Left, press_listener(&state))
            .on_mouse_up(MouseButton::Left, release_listener(&state))
            .on_mouse_up_out(MouseButton::Left, release_listener(&state))
            .on_click(move |event, window, cx| {
                if let Some(on_click) = &on_click {
                    on_click(event, window, cx);
                }
            })
    }
}

/// `ToggleView::getSize`.
fn toggle_size(st: &ToggleStyle) -> gpui::Size<Pixels> {
    let border = px_of(st.border) * 2.0;
    size(
        border + px_of(st.diameter) + px_of(st.width),
        border + px_of(st.diameter),
    )
}

/// `ToggleView::paint`: a filled track and a ringed knob at one end.
// shortcut: no slide between states; add the animation with the settings screens in M7.
fn toggle_view(st: &'static ToggleStyle, toggled: bool, cx: &App) -> impl IntoElement {
    let fg = color(
        cx,
        if toggled {
            st.toggled_fg
        } else {
            st.untoggled_fg
        },
    );
    let bg = color(
        cx,
        if toggled {
            st.toggled_bg
        } else {
            st.untoggled_bg
        },
    );
    let box_size = toggle_size(st);
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, (), window, _| {
            let border = px_of(st.border);
            let shift = px_of(st.shift);
            let diameter = px_of(st.diameter);
            let full = diameter + px_of(st.width);
            let inner = diameter - shift * 2.0;
            let origin = bounds.origin + point(border, border);
            let track = Bounds::new(
                origin + point(shift, shift),
                size(full - shift * 2.0, inner),
            );
            window.paint_quad(gpui::fill(track, fg).corner_radii(inner / 2.0));
            let left = if toggled { full - diameter } else { px(0.0) };
            let knob = Bounds::new(
                origin + point(left + border / 2.0, border / 2.0),
                size(diameter - border, diameter - border),
            );
            window.paint_quad(gpui::quad(
                knob,
                (diameter - border) / 2.0,
                bg,
                border,
                fg,
                gpui::BorderStyle::Solid,
            ));
        },
    )
    .w(box_size.width)
    .h(box_size.height)
}
