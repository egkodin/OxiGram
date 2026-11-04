//! "Default interface scale" (`SetupInterfaceScale`): the switch for the
//! default 100 % and the slider with its percent; a new scale asks to
//! restart, as tdesktop applies it at start.
// shortcut: no live preview of the scale over the slider (`SetupScalePreview`); with M8 polish.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, Bounds, Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement, Pixels, Render, Styled, Window, canvas, div,
};
use i18n::key;
use style::menu_icons::MENU_ICON_SHOW_IN_CHAT;
use style::settings::{SETTINGS_SCALE, SETTINGS_SCALE_LABEL, SETTINGS_SCALE_PADDING};
use ui::{Markup, color};

use super::{Env, rows};
use crate::boxes::ConfirmArgs;
use crate::lang::tr;
use crate::px_of;
use crate::settings;

/// `style::kScaleMin`, `kScaleDefault`, the slider's step.
const SCALE_MIN: u32 = 50;
const SCALE_DEFAULT: u32 = 100;
const STEP: u32 = 5;
/// The label's room, "300%" wide.
const LABEL_WIDTH: f32 = 40.0;

/// `style::MaxScaleForRatio`: 300 % over the screen's own scale, 200 % at
/// least.
fn max_scale(screen_ratio: f32) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "1 to 3"
    )]
    let ratio = screen_ratio.round().clamp(1.0, 3.0) as u32;
    (300 / ratio).max(200)
}

/// `cEvalScale`: the default is 100 %.
fn effective(scale: u32) -> u32 {
    if scale == 0 { SCALE_DEFAULT } else { scale }
}

/// The step nearest to `share` of the slider.
fn scale_at(share: f32, max: u32) -> u32 {
    let steps = (max - SCALE_MIN) / STEP;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to the steps"
    )]
    let step = (share.clamp(0.0, 1.0) * steps as f32).round() as u32;
    SCALE_MIN + step * STEP
}

fn share_of(scale: u32, max: u32) -> f32 {
    #[expect(clippy::cast_precision_loss, reason = "a few hundred percent")]
    let share =
        (effective(scale).clamp(SCALE_MIN, max) - SCALE_MIN) as f32 / (max - SCALE_MIN) as f32;
    share
}

pub(super) struct ScaleRow {
    env: Env,
    /// The value the slider shows while it is dragged.
    dragged: Option<u32>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl ScaleRow {
    pub(super) fn new(env: Env) -> Self {
        Self {
            env,
            dragged: None,
            bounds: Rc::default(),
        }
    }

    fn max(window: &Window, cx: &App) -> u32 {
        max_scale(window.scale_factor() / settings::interface_zoom(cx))
    }

    fn drag_to(&mut self, x: Pixels, window: &Window, cx: &mut Context<Self>) {
        let bar = self.bounds.get();
        let share = (x - bar.left()) / bar.size.width;
        self.dragged = Some(scale_at(share, Self::max(window, cx)));
        cx.notify();
    }

    /// `setScale`: a new size asks to restart; the default and the same
    /// percent only save.
    fn set(&mut self, scale: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.dragged = None;
        cx.notify();
        let current = settings::interface_scale(cx);
        if effective(scale) == effective(current) {
            settings::set_interface_scale(scale, cx);
            return;
        }
        let mut args = ConfirmArgs::new(Markup::plain(
            tr(cx, key::lng_settings_need_restart).to_string(),
        ));
        args.confirm_text = Some(tr(cx, key::lng_settings_restart_now));
        args.cancel_text = Some(tr(cx, key::lng_cancel));
        args.confirmed = Some(Rc::new(move |window, cx| {
            settings::set_interface_scale(scale, cx);
            window.dispatch_action(Box::new(crate::RestartApp), cx);
        }));
        crate::boxes::confirm(&self.env.targets.layers, args, window, cx);
    }

    fn slider(&self, share: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let st = &SETTINGS_SCALE;
        let (active, inactive) = (color(cx, st.active_fg), color(cx, st.inactive_fg));
        let line = px_of(st.width);
        let knob = px_of(st.seek_size.width);
        let bounds = self.bounds.clone();
        div()
            .id("settings-scale")
            .flex_1()
            .h(px_of(st.seek_size.height))
            .cursor_pointer()
            .child(
                canvas(
                    move |bar, _, _| bounds.set(bar),
                    move |bar, (), window, _| {
                        let room = bar.size.width - knob;
                        let left = bar.left() + knob / 2.0;
                        let top = bar.top() + (bar.size.height - line) / 2.0;
                        let fill = |from, width, fg| {
                            gpui::fill(
                                Bounds::new(gpui::point(from, top), gpui::size(width, line)),
                                fg,
                            )
                            .corner_radii(line / 2.0)
                        };
                        window.paint_quad(fill(left, room, inactive));
                        window.paint_quad(fill(left, room * share, active));
                        let center = gpui::point(left + room * share - knob / 2.0, bar.top());
                        window.paint_quad(
                            gpui::fill(Bounds::new(center, gpui::size(knob, knob)), active)
                                .corner_radii(knob / 2.0),
                        );
                    },
                )
                .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.drag_to(event.position.x, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if this.dragged.is_some() && event.dragging() {
                    this.drag_to(event.position.x, window, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if let Some(scale) = this.dragged {
                        this.set(scale, window, cx);
                    }
                }),
            )
    }
}

impl Render for ScaleRow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let saved = settings::interface_scale(cx);
        let shown = self.dragged.unwrap_or(saved);
        let max = Self::max(window, cx);
        let is_default = self.dragged.is_none() && saved == 0;
        let weak = cx.weak_entity();
        let toggle = rows::button(
            "settings-scale-default",
            tr(cx, key::lng_settings_default_scale),
            MENU_ICON_SHOW_IN_CHAT,
        )
        .toggle(is_default)
        .on_click(move |_, window, cx| {
            // On: the default; off keeps the same percent (`toggledValue`).
            let scale = if is_default { effective(saved) } else { 0 };
            let _ = weak.update(cx, |this, cx| this.set(scale, window, cx));
        });
        let padding = SETTINGS_SCALE_PADDING;
        let label = rows::text(format!("{}%", effective(shown)), &SETTINGS_SCALE_LABEL, cx)
            .w(gpui::px(LABEL_WIDTH))
            .flex_none();
        div().flex().flex_col().child(toggle).child(
            div()
                .flex()
                .items_center()
                .gap(gpui::px(LABEL_WIDTH / 4.0))
                .pl(px_of(padding.left))
                .pr(px_of(padding.right))
                .pt(px_of(padding.top))
                .pb(px_of(padding.bottom))
                .child(self.slider(share_of(shown, max), cx))
                .child(label),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_steps_follow_tdesktops_range() {
        assert_eq!(max_scale(2.0), 200);
        assert_eq!(max_scale(1.0), 300);
        assert_eq!(scale_at(0.0, 200), 50);
        assert_eq!(scale_at(1.0, 200), 200);
        assert_eq!(scale_at(1.0 / 3.0, 200), 100);
        assert_eq!(scale_at(0.34, 200), 100);
        assert!((share_of(0, 200) - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(effective(0), 100);
    }
}
