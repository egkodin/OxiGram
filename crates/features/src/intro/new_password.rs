//! `PasscodeBox` after a recovery code: a new cloud password, entered twice,
//! and its hint.

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, SharedString, Styled, Subscription, WeakEntity, Window, div, px,
};
use i18n::key;
use model::{Secret, auth};
use style::layers::{BOX_PADDING, DEFAULT_BOX};
use style::widgets::DEFAULT_INPUT_FIELD;
use ui::{BoxFrame, FlatLabel, InputEvent, InputField, InputMode, Markup, RoundButton};

use super::errors;
use super::step::Shared;
use crate::lang::tr;
use crate::layers::{Dismiss, Layers};
use crate::px_of;

/// The gap between the fields.
// shortcut: passcode_box.style is not exported; these gaps approximate passcodeLittleSkip and passcodeSkip.
const FIELD_SKIP: f32 = 8.0;

pub(crate) struct NewPasswordBox {
    shared: Shared,
    layers: WeakEntity<Layers>,
    code: Secret,
    password: Entity<InputField>,
    repeat: Entity<InputField>,
    hint: Entity<InputField>,
    error: Option<SharedString>,
    is_sent: bool,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for NewPasswordBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewPasswordBox").finish_non_exhaustive()
    }
}

impl Focusable for NewPasswordBox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.password.focus_handle(cx)
    }
}

/// Opens the box for an accepted recovery `code`.
pub(crate) fn show(shared: &Shared, code: Secret, window: &mut Window, cx: &mut App) {
    let layers = shared.layers.clone();
    let shared = shared.clone();
    let view = cx.new(|cx| {
        let field = |placeholder, mode, window: &mut Window, cx: &mut Context<NewPasswordBox>| {
            cx.new(|cx| {
                InputField::new(&DEFAULT_INPUT_FIELD, tr(cx, placeholder), window, cx)
                    .with_mode(mode)
            })
        };
        let password = field(
            key::lng_cloud_password_enter_new,
            InputMode::Password,
            window,
            cx,
        );
        let repeat = field(
            key::lng_cloud_password_confirm_new,
            InputMode::Password,
            window,
            cx,
        );
        let hint = field(key::lng_cloud_password_hint, InputMode::Text, window, cx);
        let subscriptions = [&password, &repeat, &hint]
            .into_iter()
            .map(|field| {
                cx.subscribe_in(
                    field,
                    window,
                    |this: &mut NewPasswordBox, _, event, window, cx| match event {
                        InputEvent::Changed => {
                            if this.error.take().is_some() {
                                cx.notify();
                            }
                        }
                        InputEvent::Submitted => this.save(window, cx),
                        _ => {}
                    },
                )
            })
            .collect();
        NewPasswordBox {
            shared,
            layers: layers.downgrade(),
            code,
            password,
            repeat,
            hint,
            error: None,
            is_sent: false,
            _subscriptions: subscriptions,
        }
    });
    layers.update(cx, |layers, cx| {
        layers.show(view, Dismiss::Allowed, None, window, cx)
    });
}

impl NewPasswordBox {
    /// `PasscodeBox::save` for a recovered password.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_sent {
            return;
        }
        let password = self.password.read(cx).text().to_owned();
        let repeat = self.repeat.read(cx).text().to_owned();
        let hint = self.hint.read(cx).text().to_owned();
        if password.is_empty() {
            self.password.update(cx, |field, cx| field.show_error(cx));
            crate::focus(&self.password, window, cx);
            return;
        }
        if repeat != password {
            self.repeat.update(cx, |field, cx| field.show_error(cx));
            crate::focus(&self.repeat, window, cx);
            if !repeat.is_empty() {
                self.error = Some(tr(cx, key::lng_cloud_password_differ));
            }
            cx.notify();
            return;
        }
        if hint == password {
            self.hint.update(cx, |field, cx| field.show_error(cx));
            self.error = Some(tr(cx, key::lng_cloud_password_bad));
            cx.notify();
            return;
        }
        self.is_sent = true;
        let action = auth::Action::RecoverPassword {
            code: self.code.clone(),
            new_password: Secret::new(password),
            hint,
        };
        let task = self.shared.store.read(cx).auth(action, cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.is_sent = false;
                match result {
                    // Signed in: the intro gives way to the main window.
                    Ok(()) => this.close(window, cx),
                    Err(error) => {
                        this.error = Some(errors::text(&error, cx));
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
    }
}

impl Render for NewPasswordBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let padding = BOX_PADDING;
        let width = px_of(style::layers::BOX_WIDTH) - px_of(padding.left) - px_of(padding.right);
        let fields = div()
            .pl(px_of(padding.left))
            .pr(px_of(padding.right))
            .pt(px_of(padding.top))
            .pb(px_of(padding.bottom))
            .flex()
            .flex_col()
            .gap(px(FIELD_SKIP))
            .child(div().w(width).child(self.password.clone()))
            .child(div().w(width).child(self.repeat.clone()))
            .child(div().w(width).child(self.hint.clone()))
            .children(self.error.clone().map(|error| {
                FlatLabel::new(
                    "new-password-error",
                    &style::intro::INTRO_ERROR,
                    Markup::plain(error.to_string()),
                )
                .width(width)
            }));
        let entity = cx.entity();
        let cancel_entity = entity.clone();
        BoxFrame::new("new-password", &DEFAULT_BOX)
            .title(tr(cx, key::lng_cloud_password_change))
            .row(fields)
            .button(
                RoundButton::new(
                    "new-password-save",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_settings_save),
                )
                .on_click(move |_, window, cx| entity.update(cx, |this, cx| this.save(window, cx))),
            )
            .button(
                RoundButton::new(
                    "new-password-cancel",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_cancel),
                )
                .on_click(move |_, window, cx| {
                    cancel_entity.update(cx, |this, cx| this.close(window, cx))
                }),
            )
    }
}
