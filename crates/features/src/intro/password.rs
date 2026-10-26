//! The cloud password step (`intro_password_check.cpp`), with recovery by
//! email code and the way to an account reset.

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Window, div, prelude::*,
};
use i18n::key;
use model::Secret;
use model::auth::{self, Recovery};
use style::intro::{
    INTRO_ERROR_BELOW_LINK_TOP, INTRO_LINK, INTRO_LINK_TOP, INTRO_PASSWORD, INTRO_PASSWORD_HINT,
    INTRO_PASSWORD_HINT_TOP, INTRO_PASSWORD_TOP, INTRO_STEP_FIELD_TOP,
};
use style::widgets::BUTTON_RADIUS;
use ui::{FlatLabel, InputEvent, InputField, InputMode, LinkButton, Markup};

use super::errors;
use super::new_password;
use super::step::{Geometry, Header, Shared, StepError, StepEvent, StepView, Visibility};
use crate::boxes::{self, ConfirmArgs};
use crate::lang::{tr, tr_with};
use crate::px_of;

/// Which field the step shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Password,
    /// The code from the recovery email.
    RecoveryCode,
}

pub(crate) struct PasswordStep {
    shared: Shared,
    focus: FocusHandle,
    password: Entity<InputField>,
    code: Entity<InputField>,
    view: View,
    /// The recovery email, once a code was sent there.
    email_pattern: Option<String>,
    is_sent: bool,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for PasswordStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PasswordStep")
            .field("view", &self.view)
            .finish_non_exhaustive()
    }
}

impl PasswordStep {
    pub fn new(shared: Shared, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let password = cx.new(|cx| {
            InputField::new(
                &INTRO_PASSWORD,
                tr(cx, key::lng_signin_password),
                window,
                cx,
            )
            .with_mode(InputMode::Password)
        });
        let code =
            cx.new(|cx| InputField::new(&INTRO_PASSWORD, tr(cx, key::lng_signin_code), window, cx));
        let subscriptions = [&password, &code]
            .into_iter()
            .map(|field| {
                cx.subscribe_in(
                    field,
                    window,
                    |this: &mut PasswordStep, _, event, window, cx| match event {
                        InputEvent::Changed => {
                            if this.error.take().is_some() {
                                cx.notify();
                            }
                        }
                        InputEvent::Submitted => this.submit(window, cx),
                        _ => {}
                    },
                )
            })
            .collect();
        Self {
            shared,
            focus: cx.focus_handle(),
            password,
            code,
            view: View::Password,
            email_pattern: None,
            is_sent: false,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    fn state(&self, cx: &App) -> Option<auth::Password> {
        match self.shared.auth(cx) {
            auth::State::WaitPassword(password) => Some(password.clone()),
            _ => None,
        }
    }

    fn field(&self) -> &Entity<InputField> {
        match self.view {
            View::Password => &self.password,
            View::RecoveryCode => &self.code,
        }
    }

    /// `toRecover`: a code to the recovery email, or the reset when there
    /// is no email.
    fn switch_to_recover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.state(cx) else {
            return;
        };
        let Recovery::Email { pattern } = state.recovery else {
            self.inform_then_reset(key::lng_signin_no_email_forgot, window, cx);
            return;
        };
        self.error = None;
        self.view = View::RecoveryCode;
        self.password
            .update(cx, |field, cx| field.set_text("", None, cx));
        crate::focus(&self.code, window, cx);
        if self.email_pattern.is_none() {
            self.email_pattern = Some(pattern);
            let task = self
                .shared
                .store
                .read(cx)
                .auth(auth::Action::RequestPasswordRecovery, cx);
            cx.spawn_in(window, async move |this, cx| {
                if task.await.is_err() {
                    // `recoverStartFail`: back to the password.
                    this.update_in(cx, |this, window, cx| {
                        this.email_pattern = None;
                        this.show_password(window, cx);
                    })
                    .ok();
                }
            })
            .detach();
        }
        cx.notify();
    }

    /// `toPassword`: the user cannot reach the email either.
    fn switch_to_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.inform_then_reset(key::lng_signin_cant_email_forgot, window, cx);
    }

    fn inform_then_reset(&mut self, text: i18n::Key, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let closed: boxes::Callback = Rc::new(move |window, cx| {
            this.update(cx, |this, cx| this.show_reset(window, cx)).ok();
        });
        let text = tr(cx, text).to_string();
        boxes::inform(
            &self.shared.layers,
            Markup::plain(text),
            Some(closed),
            window,
            cx,
        );
    }

    /// `showReset`: back to the password, with the reset button.
    fn show_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_password(window, cx);
        cx.emit(StepEvent::ResetButton(Visibility::Shown));
    }

    fn show_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.view = View::Password;
        self.error = None;
        self.code
            .update(cx, |field, cx| field.set_text("", None, cx));
        crate::focus(&self.password, window, cx);
        cx.notify();
    }

    fn check_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let password = Secret::new(self.password.read(cx).text());
        self.send(auth::Action::CheckPassword(password), window, cx);
    }

    fn check_code(&mut self, code: Secret, window: &mut Window, cx: &mut Context<Self>) {
        let checked = code.clone();
        self.is_sent = true;
        let task = self
            .shared
            .store
            .read(cx)
            .auth(auth::Action::CheckRecoveryCode(code), cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.is_sent = false;
                match result {
                    // `codeSubmitDone`: the code is good; choose a new password.
                    Ok(()) => new_password::show(&this.shared, checked, window, cx),
                    Err(error) => this.failed(&error, window, cx),
                }
            })
            .ok();
        })
        .detach();
    }

    fn send(&mut self, action: auth::Action, window: &mut Window, cx: &mut Context<Self>) {
        self.is_sent = true;
        let task = self.shared.store.read(cx).auth(action, cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.is_sent = false;
                if let Err(error) = result {
                    this.failed(&error, window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// `pwdSubmitFail` and `codeSubmitFail`.
    fn failed(&mut self, error: &auth::Error, window: &mut Window, cx: &mut Context<Self>) {
        let text = match (self.view, error) {
            (View::RecoveryCode, auth::Error::CodeInvalid) => tr(cx, key::lng_signin_wrong_code),
            (View::RecoveryCode, auth::Error::RecoveryUnavailable) => {
                self.email_pattern = None;
                self.show_password(window, cx);
                return;
            }
            (View::RecoveryCode, auth::Error::RecoveryExpired) => {
                self.email_pattern = None;
                self.switch_to_password(window, cx);
                return;
            }
            (_, other) => errors::text(other, cx),
        };
        let field = self.field().clone();
        field.update(cx, |field, cx| {
            field.select_all(cx);
            field.show_error(cx);
        });
        crate::focus(&field, window, cx);
        self.error = Some(text);
        cx.notify();
    }
}

impl EventEmitter<StepEvent> for PasswordStep {}

impl Focusable for PasswordStep {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl StepView for PasswordStep {
    fn header(&self, cx: &App) -> Header {
        let description = match (self.view, &self.email_pattern) {
            (View::RecoveryCode, Some(pattern)) => Markup::plain(tr_with(
                cx,
                key::lng_signin_recover_desc,
                &[("email", pattern)],
            )),
            _ => Markup::plain(tr(cx, key::lng_signin_desc).to_string()),
        };
        Header {
            title: tr(cx, key::lng_signin_title),
            description,
            has_cover: false,
            has_back: true,
            next: Some(tr(cx, key::lng_intro_submit)),
            next_style: &style::intro::INTRO_NEXT_BUTTON,
            error: self.error.clone().map(|text| StepError {
                text,
                centered: false,
            }),
            error_top: px_of(INTRO_ERROR_BELOW_LINK_TOP),
        }
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_sent {
            return;
        }
        match self.view {
            View::Password => {
                self.error = None;
                self.check_password(window, cx);
            }
            View::RecoveryCode => {
                let code = self.code.read(cx).text().trim().to_owned();
                if code.is_empty() {
                    self.code.update(cx, |field, cx| field.show_error(cx));
                    return;
                }
                let has_passport = self.state(cx).is_some_and(|state| state.has_passport_data);
                if !has_passport {
                    self.check_code(Secret::new(code), window, cx);
                    return;
                }
                let mut args = ConfirmArgs::new(Markup::plain(
                    tr(cx, key::lng_cloud_password_passport_losing).to_string(),
                ));
                args.confirm_text = Some(tr(cx, key::lng_continue));
                args.cancel_text = Some(tr(cx, key::lng_cancel));
                let this = cx.entity().downgrade();
                let code = Secret::new(code);
                args.confirmed = Some(Rc::new(move |window, cx| {
                    let code = code.clone();
                    this.update(cx, |this, cx| this.check_code(code, window, cx))
                        .ok();
                }));
                boxes::confirm(&self.shared.layers, args, window, cx);
            }
        }
    }

    fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = self.field().clone();
        crate::focus(&field, window, cx);
    }
}

impl Render for PasswordStep {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = Geometry::of(window);
        let top = g.content_top(false);
        let left = g.content_left();
        let width = px_of(INTRO_PASSWORD.width);
        let code_top = top + px_of(INTRO_STEP_FIELD_TOP);
        let link_top = code_top + px_of(INTRO_PASSWORD.height_min) + px_of(INTRO_LINK_TOP);
        let hint = self
            .state(cx)
            .map(|state| state.hint)
            .filter(|hint| !hint.is_empty() && self.view == View::Password)
            .map(|hint| tr_with(cx, key::lng_signin_hint, &[("password_hint", &hint)]));
        let this = cx.entity();
        let (link_text, is_recover) = match self.view {
            View::Password => (tr(cx, key::lng_signin_recover), true),
            View::RecoveryCode => (tr(cx, key::lng_signin_try_password), false),
        };
        let mut root = div().track_focus(&self.focus).size_full().relative();
        root = match self.view {
            View::Password => root.child(
                div()
                    .absolute()
                    .left(left)
                    .top(top + px_of(INTRO_PASSWORD_TOP))
                    .w(width)
                    .child(self.password.clone()),
            ),
            View::RecoveryCode => root.child(
                div()
                    .absolute()
                    .left(left)
                    .top(code_top)
                    .w(width)
                    .child(self.code.clone()),
            ),
        };
        root.children(hint.map(|hint| {
            div()
                .absolute()
                .left(left + px_of(BUTTON_RADIUS))
                .top(top + px_of(INTRO_PASSWORD_HINT_TOP))
                .child(FlatLabel::new(
                    "password-hint",
                    &INTRO_PASSWORD_HINT,
                    Markup::plain(hint),
                ))
        }))
        .child(
            div()
                .absolute()
                .left(left + px_of(BUTTON_RADIUS))
                .top(link_top)
                .child(
                    LinkButton::new("password-link", &INTRO_LINK, link_text).on_click(
                        move |_, window, cx| {
                            this.update(cx, |this, cx| {
                                if is_recover {
                                    this.switch_to_recover(window, cx);
                                } else {
                                    this.switch_to_password(window, cx);
                                }
                            });
                        },
                    ),
                ),
        )
    }
}
