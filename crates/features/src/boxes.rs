//! tdesktop's `MakeConfirmBox` and `MakeInformBox`.

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding,
    ParentElement, Render, SharedString, Styled, WeakEntity, Window, actions, div, prelude::*,
};
use style::layers::{BOX_LABEL, BOX_PADDING, DEFAULT_BOX};
use style::widgets::RoundButton as RoundButtonStyle;
use ui::{BoxFrame, FlatLabel, Markup, RoundButton};

use crate::lang::tr;
use crate::layers::{Dismiss, Layers};

actions!(confirm_box, [Confirm]);

const CONTEXT: &str = "ConfirmBox";

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("enter", Confirm, Some(CONTEXT))]);
}

pub type Callback = Rc<dyn Fn(&mut Window, &mut App)>;

impl std::fmt::Debug for ConfirmArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfirmArgs")
            .field("title", &self.title)
            .field("text", &self.text)
            .finish_non_exhaustive()
    }
}

/// What a confirm box shows and does. The box closes before a callback runs.
pub struct ConfirmArgs {
    pub title: Option<SharedString>,
    pub text: Markup,
    pub confirm_text: Option<SharedString>,
    pub confirm_style: &'static RoundButtonStyle,
    pub confirmed: Option<Callback>,
    /// `None` makes an inform box: only the confirm button.
    pub cancel_text: Option<SharedString>,
    /// Runs on the cancel button, Escape and a click outside; an inform box
    /// runs `confirmed` there instead (`boxClosing`).
    pub cancelled: Option<Callback>,
    /// Escape and a click outside only close the box, also an inform box
    /// (`strictCancel`).
    pub strict_cancel: bool,
}

impl ConfirmArgs {
    #[must_use]
    pub fn new(text: Markup) -> Self {
        Self {
            title: None,
            text,
            confirm_text: None,
            confirm_style: &DEFAULT_BOX.button,
            confirmed: None,
            cancel_text: None,
            cancelled: None,
            strict_cancel: false,
        }
    }
}

pub struct ConfirmBox {
    args: ConfirmArgs,
    layers: WeakEntity<Layers>,
    focus: FocusHandle,
}

impl std::fmt::Debug for ConfirmBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfirmBox")
            .field("text", &self.args.text)
            .finish_non_exhaustive()
    }
}

impl Focusable for ConfirmBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Shows a box with an OK button; `closed` runs however it closes.
pub fn inform(
    layers: &Entity<Layers>,
    text: Markup,
    closed: Option<Callback>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut args = ConfirmArgs::new(text);
    args.confirmed = closed;
    confirm(layers, args, window, cx);
}

/// Asks before a destructive `action`: a red `confirm_text` button and
/// Cancel; Escape and a click outside cancel.
pub(crate) fn confirm_attention(
    layers: &Entity<Layers>,
    text: String,
    confirm_text: SharedString,
    action: Callback,
    window: &mut Window,
    cx: &mut App,
) {
    let args = attention_args(text, confirm_text, tr(cx, i18n::key::lng_cancel), action);
    confirm(layers, args, window, cx);
}

fn attention_args(
    text: String,
    confirm_text: SharedString,
    cancel_text: SharedString,
    action: Callback,
) -> ConfirmArgs {
    let mut args = ConfirmArgs::new(Markup::plain(text));
    args.confirm_text = Some(confirm_text);
    args.confirm_style = &style::layers::ATTENTION_BOX_BUTTON;
    args.cancel_text = Some(cancel_text);
    args.confirmed = Some(action);
    args
}

/// Shows a confirm box (an inform box when `args.cancel_text` is `None` and
/// there is no `cancelled`).
pub fn confirm(layers: &Entity<Layers>, args: ConfirmArgs, window: &mut Window, cx: &mut App) {
    let is_inform = args.cancel_text.is_none() && args.cancelled.is_none();
    let on_dismiss = if args.strict_cancel {
        None
    } else if is_inform {
        args.confirmed.clone()
    } else {
        args.cancelled.clone()
    };
    let weak = layers.downgrade();
    let view = cx.new(|cx| ConfirmBox {
        args,
        layers: weak,
        focus: cx.focus_handle(),
    });
    layers.update(cx, |layers, cx| {
        layers.show(
            view,
            Dismiss::Allowed,
            on_dismiss.map(|callback| -> crate::layers::OnDismiss {
                Box::new(move |window, cx| callback(window, cx))
            }),
            window,
            cx,
        );
    });
}

impl ConfirmBox {
    fn finish(&mut self, callback: Option<Callback>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
        if let Some(callback) = callback {
            callback(window, cx);
        }
    }

    fn is_inform(&self) -> bool {
        self.args.cancel_text.is_none() && self.args.cancelled.is_none()
    }
}

impl Render for ConfirmBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_title = self.args.title.is_some();
        let padding = BOX_PADDING;
        let top = if has_title { 0 } else { padding.top.0 };
        let label = div()
            .pl(px_of(padding.left.0))
            .pr(px_of(padding.right.0))
            .pt(px_of(top))
            .pb(px_of(padding.bottom.0))
            .child(
                FlatLabel::new("confirm-text", &BOX_LABEL, self.args.text.clone()).width(px_of(
                    style::layers::BOX_WIDTH.0 - padding.left.0 - padding.right.0,
                )),
            );
        let confirm_text = self
            .args
            .confirm_text
            .clone()
            .unwrap_or_else(|| tr(cx, i18n::key::lng_box_ok));
        let entity = cx.entity();
        let confirm =
            RoundButton::new("confirm", self.args.confirm_style, confirm_text).on_click({
                let entity = entity.clone();
                move |_, window, cx| {
                    entity.update(cx, |this, cx| {
                        this.finish(this.args.confirmed.clone(), window, cx)
                    });
                }
            });
        let mut frame = BoxFrame::new("confirm-box", &DEFAULT_BOX)
            .row(label)
            .button(confirm);
        if let Some(title) = self.args.title.clone() {
            frame = frame.title(title);
        }
        if !self.is_inform() {
            let cancel_text = self
                .args
                .cancel_text
                .clone()
                .unwrap_or_else(|| tr(cx, i18n::key::lng_cancel));
            frame = frame.button(
                RoundButton::new("cancel", &DEFAULT_BOX.button, cancel_text).on_click(
                    move |_, window, cx| {
                        entity.update(cx, |this, cx| {
                            this.finish(this.args.cancelled.clone(), window, cx)
                        });
                    },
                ),
            );
        }
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Confirm, window, cx| {
                this.finish(this.args.confirmed.clone(), window, cx);
            }))
            .child(frame)
    }
}

fn px_of(value: i32) -> gpui::Pixels {
    #[expect(clippy::cast_precision_loss, reason = "style sizes are small")]
    gpui::px(value as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An inform box runs its action on Escape, so a destructive one must
    /// never be one.
    #[test]
    fn destructive_boxes_cancel_on_escape() {
        let args = attention_args(
            "Log out?".to_owned(),
            "Log out".into(),
            "Cancel".into(),
            Rc::new(|_, _| {}),
        );
        assert!(args.cancel_text.is_some());
        assert!(args.cancelled.is_none(), "Escape must only close the box");
    }
}
