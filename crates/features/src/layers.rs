//! lib_ui's `LayerStackWidget`: one box at a time over the window, which
//! dims behind it. Escape and a click outside close it unless the box is
//! strict.

use gpui::{
    AnyView, App, Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding, MouseButton,
    ParentElement, Pixels, Render, Styled, Window, actions, div, prelude::*,
};
use style::layers::BOX_DURATION;
use ui::anim::{Easing, Simple};
use ui::hsla;

actions!(layers, [CloseLayer]);

const CONTEXT: &str = "LayerStack";

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CloseLayer, Some(CONTEXT))]);
}

/// Whether Escape and clicks outside close the box (`setCloseByEscape`,
/// `setCloseByOutsideClick`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dismiss {
    Allowed,
    /// The box closes only through its own buttons, as the terms box does.
    ButtonsOnly,
}

/// Runs when the box is dismissed by Escape or a click outside.
pub type OnDismiss = Box<dyn FnOnce(&mut Window, &mut App)>;

/// Where the layer sits: boxes in the middle, the main menu along the left
/// edge, sliding in (`showMainMenu`).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Placement {
    Center,
    Left { width: Pixels },
}

struct Shown {
    view: AnyView,
    placement: Placement,
    dismiss: Dismiss,
    on_dismiss: Option<OnDismiss>,
}

/// The window's box layer.
pub struct Layers {
    current: Option<Shown>,
    /// A layer fading out after it closed.
    leaving: Option<(AnyView, Placement)>,
    background: Simple,
    layer: Simple,
    restore_focus: Option<FocusHandle>,
}

impl std::fmt::Debug for Layers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Layers")
            .field("is_shown", &self.current.is_some())
            .finish_non_exhaustive()
    }
}

impl Layers {
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: None,
            leaving: None,
            background: Simple::default(),
            layer: Simple::default(),
            restore_focus: None,
        }
    }

    #[must_use]
    pub fn is_shown(&self) -> bool {
        self.current.is_some()
    }

    /// Shows `view`, replacing any open box (`LayerOption::CloseOther`), and
    /// focuses it.
    pub fn show<V: Render + Focusable>(
        &mut self,
        view: Entity<V>,
        dismiss: Dismiss,
        on_dismiss: Option<OnDismiss>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_at(view, Placement::Center, dismiss, on_dismiss, window, cx);
    }

    /// Slides `view`, `width` wide, in from the left edge, as the main menu.
    pub fn show_panel<V: Render + Focusable>(
        &mut self,
        view: Entity<V>,
        width: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let placement = Placement::Left { width };
        self.show_at(view, placement, Dismiss::Allowed, None, window, cx);
    }

    fn show_at<V: Render + Focusable>(
        &mut self,
        view: Entity<V>,
        placement: Placement,
        dismiss: Dismiss,
        on_dismiss: Option<OnDismiss>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus = view.focus_handle(cx);
        if self.current.is_none() {
            self.restore_focus = window.focused(cx);
            self.background
                .start(0.0, 1.0, BOX_DURATION, Easing::EaseOutCirc);
            self.layer.start(0.0, 1.0, BOX_DURATION, Easing::Linear);
        }
        self.leaving = None;
        self.current = Some(Shown {
            view: view.into(),
            placement,
            dismiss,
            on_dismiss,
        });
        focus.focus(window, cx);
        cx.notify();
    }

    /// Hides the open box with a fade (`closeBox`).
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(shown) = self.current.take() else {
            return;
        };
        self.leaving = Some((shown.view, shown.placement));
        self.background
            .start(1.0, 0.0, BOX_DURATION, Easing::EaseOutCirc);
        self.layer.start(1.0, 0.0, BOX_DURATION, Easing::Linear);
        if let Some(focus) = self.restore_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(shown) = self.current.as_mut() else {
            return;
        };
        if shown.dismiss != Dismiss::Allowed {
            return;
        }
        let on_dismiss = shown.on_dismiss.take();
        self.close(window, cx);
        if let Some(on_dismiss) = on_dismiss {
            on_dismiss(window, cx);
        }
    }
}

impl Default for Layers {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for Layers {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = self.current.is_some();
        let background = self
            .background
            .value_in(window, if shown { 1.0 } else { 0.0 });
        let opacity = self.layer.value_in(window, if shown { 1.0 } else { 0.0 });
        if !shown && !self.background.animating() {
            self.leaving = None;
        }
        let view = self
            .current
            .as_ref()
            .map(|shown| (shown.view.clone(), shown.placement))
            .or_else(|| self.leaving.clone());
        let Some((view, placement)) = view else {
            return div().into_any_element();
        };
        let mut dim = hsla(cx.global::<ui::Theme>().palette.layer_bg);
        dim.a *= background;
        div()
            .id("layers")
            .key_context(CONTEXT)
            .absolute()
            .inset_0()
            .flex()
            .bg(dim)
            // Nothing behind the layer gets the mouse, right clicks included.
            .occlude()
            .on_action(cx.listener(|this, _: &CloseLayer, window, cx| this.dismiss(window, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.dismiss(window, cx)),
            )
            .map(|this| match placement {
                Placement::Center => this
                    .items_center()
                    .justify_center()
                    .child(div().opacity(opacity).child(view)),
                Placement::Left { width } => this.child(
                    div()
                        .relative()
                        .h_full()
                        .left(-width * (1.0 - background))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(view),
                ),
            })
            .into_any_element()
    }
}
