//! lib_ui's `RoundButton`, `LinkButton` and `IconButton`.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, CursorStyle, ElementId, IntoElement, MouseButton, ParentElement, Pixels,
    RenderOnce, SharedString, TextRun, Window, div, prelude::*, px,
};
use style::widgets::{
    BUTTON_RADIUS, CrossButton as CrossStyle, IconButton as IconButtonStyle,
    LinkButton as LinkStyle,
};
use style::{Color, ColorRef};

use crate::anim::mix;
use crate::font::{font, font_height};
use crate::icon::{icon, icon_size};
use crate::ripple::{self, ButtonState, Shape, use_button_state};
use crate::theme::{Theme, color, hsla};

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub(crate) fn px_of(value: style::Px) -> Pixels {
    #[expect(clippy::cast_precision_loss, reason = "style sizes are small")]
    px(value.0 as f32)
}

/// `color` drawn over `base`, as two painted rectangles would look.
fn over(base: Color, top: Color) -> Color {
    let alpha = f32::from(top.a) / 255.0;
    let mut opaque = top;
    opaque.a = 255;
    let mut mixed = mix(base, opaque, alpha);
    mixed.a = base.a.max(top.a);
    mixed
}

fn text_width(text: &SharedString, style: style::Font, window: &Window) -> Pixels {
    let run = TextRun {
        len: text.len(),
        font: font(style),
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(text.clone(), px_of(style.size), &[run], None)
        .width
}

/// A filled button with centered text.
#[derive(IntoElement)]
pub struct RoundButton {
    id: ElementId,
    style: &'static style::widgets::RoundButton,
    text: SharedString,
    uppercase: bool,
    on_click: Option<ClickHandler>,
}

impl RoundButton {
    pub fn new(
        id: impl Into<ElementId>,
        style: &'static style::widgets::RoundButton,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style,
            text: text.into(),
            uppercase: false,
            on_click: None,
        }
    }

    /// `RoundButtonTextTransform::ToUpper`.
    #[must_use]
    pub fn uppercase(mut self) -> Self {
        self.uppercase = true;
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

    /// The width `resizeToText` gives the button.
    #[must_use]
    pub fn width(&self, window: &Window) -> Pixels {
        let st = self.style;
        let padding = px_of(st.padding.left) + px_of(st.padding.right);
        if st.width.0 > 0 {
            px_of(st.width) + padding
        } else {
            text_width(&self.display_text(), st.style.font, window) - px_of(st.width) + padding
        }
    }

    fn display_text(&self) -> SharedString {
        if self.uppercase {
            self.text.to_uppercase().into()
        } else {
            self.text.clone()
        }
    }
}

impl std::fmt::Debug for RoundButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RoundButton")
            .field("id", &self.id)
            .field("text", &self.text)
            .finish_non_exhaustive()
    }
}

impl RenderOnce for RoundButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let st = self.style;
        let state = use_button_state(self.id.clone(), window, cx);
        let hovered = state.read(cx).hovered;
        let palette = &cx.global::<Theme>().palette;
        let base = st.text_bg.resolve(palette);
        let background = if hovered {
            over(base, st.text_bg_over.resolve(palette))
        } else {
            base
        };
        let text_color = if hovered { st.text_fg_over } else { st.text_fg };
        let width = self.width(window);
        let height = px_of(st.height) + px_of(st.padding.top) + px_of(st.padding.bottom);
        let inner_height = px_of(st.height);
        let radius = if st.full_radius {
            inner_height / 2.0
        } else if st.radius.0 > 0 {
            px_of(st.radius)
        } else {
            px_of(BUTTON_RADIUS)
        };
        let text = self.display_text();
        let on_click = self.on_click.clone();

        div()
            .id(self.id)
            .relative()
            .w(width)
            .h(height)
            .cursor(CursorStyle::PointingHand)
            .child(
                div()
                    .absolute()
                    .left(px_of(st.padding.left))
                    .top(px_of(st.padding.top))
                    .right(px_of(st.padding.right))
                    .h(inner_height)
                    .rounded(radius)
                    .bg(hsla(background))
                    .child(ripple::layer(
                        &state,
                        st.ripple,
                        Shape::RoundRect { radius },
                    )),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px_of(st.padding.top) + px_of(st.text_top))
                    .flex()
                    .justify_center()
                    .font(font(st.style.font))
                    .text_size(px_of(st.style.font.size))
                    .line_height(font_height(st.style.font))
                    .text_color(color(cx, text_color))
                    .child(text),
            )
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

pub(crate) fn hover_listener(
    state: &gpui::Entity<ButtonState>,
) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
    let state = state.clone();
    move |hovered, _, cx| {
        let hovered = *hovered;
        state.update(cx, |state, cx| {
            if state.hovered != hovered {
                state.hovered = hovered;
                cx.notify();
            }
        });
    }
}

pub(crate) fn press_listener(
    state: &gpui::Entity<ButtonState>,
) -> impl Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static {
    let state = state.clone();
    move |event, _, cx| {
        let position = event.position;
        state.update(cx, |state, cx| state.press(position, cx));
    }
}

pub(crate) fn release_listener(
    state: &gpui::Entity<ButtonState>,
) -> impl Fn(&gpui::MouseUpEvent, &mut Window, &mut App) + 'static {
    let state = state.clone();
    move |_, _, cx| state.update(cx, ButtonState::release)
}

/// A text link that underlines on hover.
#[derive(IntoElement)]
pub struct LinkButton {
    id: ElementId,
    style: &'static LinkStyle,
    text: SharedString,
    on_click: Option<ClickHandler>,
}

impl LinkButton {
    pub fn new(
        id: impl Into<ElementId>,
        style: &'static LinkStyle,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style,
            text: text.into(),
            on_click: None,
        }
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

impl std::fmt::Debug for LinkButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkButton")
            .field("id", &self.id)
            .field("text", &self.text)
            .finish_non_exhaustive()
    }
}

impl RenderOnce for LinkButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let st = self.style;
        let state = use_button_state(self.id.clone(), window, cx);
        let hovered = state.read(cx).hovered;
        let font_style = if hovered { st.over_font } else { st.font };
        let text_color: ColorRef = if hovered { st.over_color } else { st.color };
        let on_click = self.on_click.clone();
        div()
            .id(self.id)
            .pl(px_of(st.padding.left))
            .pt(px_of(st.padding.top))
            .pr(px_of(st.padding.right))
            .pb(px_of(st.padding.bottom))
            .cursor(CursorStyle::PointingHand)
            .font(font(font_style))
            .text_size(px_of(font_style.size))
            .line_height(font_height(font_style))
            .text_color(color(cx, text_color))
            .when(
                font_style.flags.contains(style::FontFlags::UNDERLINE),
                |link| link.underline(),
            )
            .child(self.text)
            .on_hover(hover_listener(&state))
            .on_click(move |event, window, cx| {
                if let Some(on_click) = &on_click {
                    on_click(event, window, cx);
                }
            })
    }
}

/// An icon with a circular ripple, such as the intro back button.
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    style: &'static IconButtonStyle,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, style: &'static IconButtonStyle) -> Self {
        Self {
            id: id.into(),
            style,
            on_click: None,
        }
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

impl std::fmt::Debug for IconButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IconButton")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl RenderOnce for IconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let st = self.style;
        let state = use_button_state(self.id.clone(), window, cx);
        let hovered = state.read(cx).hovered;
        let current = if hovered && !st.icon_over.is_empty() {
            st.icon_over
        } else {
            st.icon
        };
        let (width, height) = (px_of(st.width), px_of(st.height));
        let size = icon_size(current);
        // A negative position centers the icon (`IconButton::paintEvent`).
        let left = if st.icon_position.x.0 < 0 {
            (width - size.width) / 2.0
        } else {
            px_of(st.icon_position.x)
        };
        let top = if st.icon_position.y.0 < 0 {
            (height - size.height) / 2.0
        } else {
            px_of(st.icon_position.y)
        };
        let ripple_size = px_of(st.ripple_area_size);
        let on_click = self.on_click.clone();
        div()
            .id(self.id)
            .relative()
            .w(width)
            .h(height)
            .cursor(CursorStyle::PointingHand)
            .child(
                div()
                    .absolute()
                    .left(px_of(st.ripple_area_position.x))
                    .top(px_of(st.ripple_area_position.y))
                    .w(ripple_size)
                    .h(ripple_size)
                    .child(ripple::layer(&state, st.ripple, Shape::Circle)),
            )
            .child(div().absolute().left(left).top(top).child(icon(current)))
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

/// A round button with a cross, such as the chat list's cancel search.
// shortcut: the cross does not scale in and out and has no loading state; add them when a screen needs them.
#[derive(IntoElement)]
pub struct CrossButton {
    id: ElementId,
    style: &'static CrossStyle,
    on_click: Option<ClickHandler>,
}

impl CrossButton {
    pub fn new(id: impl Into<ElementId>, style: &'static CrossStyle) -> Self {
        Self {
            id: id.into(),
            style,
            on_click: None,
        }
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

impl std::fmt::Debug for CrossButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrossButton")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl RenderOnce for CrossButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let st = self.style;
        let state = use_button_state(self.id.clone(), window, cx);
        let fg = color(
            cx,
            if state.read(cx).hovered {
                st.cross_fg_over
            } else {
                st.cross_fg
            },
        );
        let size = px_of(st.cross.size);
        let skip = px_of(st.cross.skip);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a stroke width of a few pixels"
        )]
        let stroke = px(st.cross.stroke as f32);
        let on_click = self.on_click.clone();
        div()
            .id(self.id)
            .relative()
            .w(px_of(st.width))
            .h(px_of(st.height))
            .cursor(CursorStyle::PointingHand)
            .child(div().absolute().size_full().child(ripple::layer(
                &state,
                st.ripple,
                Shape::Circle,
            )))
            .child(
                gpui::canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        let origin = bounds.origin
                            + gpui::point(px_of(st.cross_position.x), px_of(st.cross_position.y));
                        let (from, to) = (skip, size - skip);
                        for (start, end) in [((from, from), (to, to)), ((to, from), (from, to))] {
                            let mut builder = gpui::PathBuilder::stroke(stroke);
                            builder.move_to(origin + gpui::point(start.0, start.1));
                            builder.line_to(origin + gpui::point(end.0, end.1));
                            if let Ok(path) = builder.build() {
                                window.paint_path(path, fg);
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
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
