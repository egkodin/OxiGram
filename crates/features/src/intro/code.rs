//! The login code step (`intro_code.cpp`), for codes sent to the phone and
//! to the login email.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Task, Window, div, prelude::*,
};
use i18n::key;
use model::Secret;
use model::auth::{self, Delivery, EmailReset};
use model::phone::number_info;
use style::basic::{LINE_WIDTH, SHAKE_SHIFT};
use style::intro::{
    INTRO_DESCRIPTION, INTRO_ERROR_BELOW_LINK_TOP, INTRO_FRAGMENT_BUTTON, INTRO_LINK,
    INTRO_LINK_TOP, INTRO_NEXT_BUTTON, INTRO_PHONE_TOP, INTRO_STEP_FIELD_TOP,
};
use style::widgets::BUTTON_RADIUS;
use ui::{FlatLabel, LinkButton, Markup};

use super::code_input::{CodeInput, Collected};
use super::errors;
use super::step::{Geometry, Header, Shared, StepError, StepEvent, StepView};
use crate::boxes::{self, ConfirmArgs};
use crate::lang::{tr, tr_with};
use crate::px_of;

/// Boxes for codes whose length TDLib does not fix.
// shortcut: word and phrase codes get digit boxes; desktop clients are not sent them, add a text field if they appear.
const FALLBACK_LENGTH: usize = 5;
const SECOND: Duration = Duration::from_secs(1);

/// Where the code went.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Phone(auth::Code),
    Email(auth::EmailCode),
}

/// The automatic call (`CallStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Disabled,
    /// Seconds until Telegram calls.
    Waiting(u64),
    Calling,
    Called,
}

pub(crate) struct CodeStep {
    shared: Shared,
    focus: FocusHandle,
    mode: Option<Mode>,
    input: Entity<CodeInput>,
    call: Call,
    is_sent: bool,
    is_resending: bool,
    error: Option<SharedString>,
    _timer: Option<Task<()>>,
    _subscription: Subscription,
}

impl std::fmt::Debug for CodeStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeStep")
            .field("call", &self.call)
            .finish_non_exhaustive()
    }
}

fn mode_of(state: &auth::State) -> Option<Mode> {
    match state {
        auth::State::WaitCode(code) => Some(Mode::Phone(code.clone())),
        auth::State::WaitEmailCode(code) => Some(Mode::Email(code.clone())),
        _ => None,
    }
}

impl CodeStep {
    pub fn new(shared: Shared, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mode = mode_of(shared.auth(cx));
        let length = mode.as_ref().map_or(FALLBACK_LENGTH, length_of);
        let input = cx.new(|cx| CodeInput::new(length, cx));
        let subscription = Self::subscribe_input(&input, window, cx);
        let mut step = Self {
            shared,
            focus: cx.focus_handle(),
            mode: None,
            input,
            call: Call::Disabled,
            is_sent: false,
            is_resending: false,
            error: None,
            _timer: None,
            _subscription: subscription,
        };
        step.set_mode(mode, window, cx);
        step
    }

    fn subscribe_input(
        input: &Entity<CodeInput>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(input, window, |this, _, Collected(code), window, cx| {
            this.error = None;
            this.submit_code(code, window, cx);
        })
    }

    /// `updateDescText` for a new code: resent, or the step's first one.
    fn set_mode(&mut self, mode: Option<Mode>, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == mode {
            return;
        }
        let length = mode.as_ref().map_or(FALLBACK_LENGTH, length_of);
        let old_length = self.mode.as_ref().map_or(FALLBACK_LENGTH, length_of);
        if length != old_length {
            self.input = cx.new(|cx| CodeInput::new(length, cx));
            self._subscription = Self::subscribe_input(&self.input.clone(), window, cx);
            crate::focus(&self.input, window, cx);
        }
        let was_calling = matches!(self.call, Call::Calling);
        self.call = match &mode {
            Some(Mode::Phone(code)) => match (&code.delivery, &code.next_delivery) {
                (Delivery::Call { .. }, _) if was_calling => Call::Called,
                (Delivery::Telegram { .. }, _) => Call::Disabled,
                (_, Some(Delivery::Call { .. })) => Call::Waiting(code.resend_after.as_secs()),
                _ => Call::Disabled,
            },
            _ => Call::Disabled,
        };
        self._timer = matches!(self.call, Call::Waiting(_)).then(|| self.start_timer(cx));
        self.mode = mode;
        cx.notify();
    }

    fn start_timer(&self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(SECOND).await;
                let Ok(is_done) = this.update(cx, |this, cx| this.tick(cx)) else {
                    return;
                };
                if is_done {
                    return;
                }
            }
        })
    }

    /// Counts the call down and asks for it at zero (`sendCall`).
    fn tick(&mut self, cx: &mut Context<Self>) -> bool {
        let Call::Waiting(seconds) = self.call else {
            return true;
        };
        if seconds > 1 {
            self.call = Call::Waiting(seconds - 1);
            cx.notify();
            return false;
        }
        self.call = Call::Calling;
        self.resend(cx);
        cx.notify();
        true
    }

    fn resend(&mut self, cx: &mut Context<Self>) {
        if self.is_resending {
            return;
        }
        self.is_resending = true;
        let task = self
            .shared
            .store
            .read(cx)
            .auth(auth::Action::ResendCode, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.is_resending = false;
                match result {
                    Ok(()) => {}
                    // `noTelegramCodeFail`: nothing to show when no other way exists.
                    Err(auth::Error::SendCodeUnavailable) => {}
                    Err(error) => this.show_code_error(errors::text(&error, cx), cx),
                }
            })
            .ok();
        })
        .detach();
    }

    fn submit_code(&mut self, code: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_sent {
            return;
        }
        let action = match self.mode {
            Some(Mode::Email(_)) => auth::Action::CheckEmailCode(Secret::new(code)),
            _ => auth::Action::CheckCode(Secret::new(code)),
        };
        self.is_sent = true;
        let task = self.shared.store.read(cx).auth(action, cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.is_sent = false;
                match result {
                    Ok(()) => {}
                    Err(auth::Error::CodeExpired) => cx.emit(StepEvent::Back),
                    Err(error) => {
                        this.show_code_error(errors::text(&error, cx), cx);
                        crate::focus(&this.input, window, cx);
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn show_code_error(&mut self, text: SharedString, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.show_error(cx));
        self.error = Some(text);
        cx.notify();
    }

    fn phone_text(&self, cx: &App) -> String {
        let shown = self.shared.data.read(cx).phone.expose().to_owned();
        if !shown.is_empty() {
            return shown;
        }
        let Some(Mode::Phone(code)) = &self.mode else {
            return String::new();
        };
        let digits = code.phone_number.expose().trim_start_matches('+');
        match number_info("en", digits) {
            Ok(info) if !info.formatted.contains('-') => {
                format!("+{} {}", info.calling_code, info.formatted)
            }
            _ => format!("+{digits}"),
        }
    }

    fn fragment_url(&self) -> Option<&str> {
        match &self.mode {
            Some(Mode::Phone(auth::Code {
                delivery: Delivery::Fragment { url, .. },
                ..
            })) => Some(url),
            _ => None,
        }
    }

    fn description(&self, cx: &App) -> Markup {
        match &self.mode {
            Some(Mode::Email(code)) => Markup::plain(tr_with(
                cx,
                key::lng_intro_email_confirm_subtitle,
                &[("email", &code.email_pattern)],
            )),
            Some(Mode::Phone(code)) => match &code.delivery {
                Delivery::Fragment { .. } => Markup::rich(&tr_with(
                    cx,
                    key::lng_intro_fragment_about,
                    &[("phone_number", &self.phone_text(cx))],
                )),
                Delivery::Telegram { .. } => Markup::rich(&tr(cx, key::lng_code_from_telegram)),
                _ => Markup::rich(&tr(cx, key::lng_code_desc)),
            },
            None => Markup::rich(&tr(cx, key::lng_code_desc)),
        }
    }

    /// The text under the code: the call countdown, or nothing.
    fn call_text(&self, cx: &App) -> Option<String> {
        match self.call {
            Call::Disabled => None,
            Call::Waiting(seconds) => {
                let minutes = if seconds >= 3600 {
                    format!("{}:{:02}", seconds / 3600, (seconds / 60) % 60)
                } else {
                    (seconds / 60).to_string()
                };
                let seconds = format!("{:02}", seconds % 60);
                Some(tr_with(
                    cx,
                    key::lng_code_call,
                    &[("minutes", &minutes), ("seconds", &seconds)],
                ))
            }
            Call::Calling => Some(tr(cx, key::lng_code_calling).to_string()),
            Call::Called => Some(tr(cx, key::lng_code_called).to_string()),
        }
    }

    /// The link under the code: another way to get it.
    fn link(&self, cx: &App) -> Option<(SharedString, Link)> {
        match &self.mode {
            Some(Mode::Phone(code)) => {
                let is_telegram = matches!(code.delivery, Delivery::Telegram { .. });
                (is_telegram && code.next_delivery.is_some())
                    .then(|| (tr(cx, key::lng_code_no_telegram), Link::Resend))
            }
            Some(Mode::Email(code)) => match code.reset? {
                EmailReset::Available { wait } => {
                    Some((tr(cx, key::rgram_intro_email_reset), Link::ResetEmail(wait)))
                }
                EmailReset::Pending { remaining } if remaining.is_zero() => Some((
                    tr(cx, key::rgram_intro_email_reset),
                    Link::ResetEmail(Duration::ZERO),
                )),
                EmailReset::Pending { .. } => None,
            },
            None => None,
        }
    }

    fn email_reset_pending(&self, cx: &App) -> Option<String> {
        match &self.mode {
            Some(Mode::Email(auth::EmailCode {
                reset: Some(EmailReset::Pending { remaining }),
                ..
            })) if !remaining.is_zero() => Some(tr_with(
                cx,
                key::rgram_intro_email_reset_pending,
                &[("when", &errors::duration_text(*remaining, cx))],
            )),
            _ => None,
        }
    }

    fn link_clicked(&mut self, link: Link, window: &mut Window, cx: &mut Context<Self>) {
        match link {
            Link::Resend => self.resend(cx),
            Link::ResetEmail(wait) if wait.is_zero() => self.reset_email(cx),
            Link::ResetEmail(wait) => {
                let text = tr_with(
                    cx,
                    key::rgram_intro_email_reset_about,
                    &[("when", &errors::duration_text(wait, cx))],
                );
                let mut args = ConfirmArgs::new(Markup::plain(text));
                args.confirm_text = Some(tr(cx, key::rgram_intro_email_reset));
                args.cancel_text = Some(tr(cx, key::lng_cancel));
                let this = cx.entity().downgrade();
                args.confirmed = Some(Rc::new(move |_, cx| {
                    this.update(cx, |this, cx| this.reset_email(cx)).ok();
                }));
                boxes::confirm(&self.shared.layers, args, window, cx);
            }
        }
    }

    fn reset_email(&mut self, cx: &mut Context<Self>) {
        let task = self
            .shared
            .store
            .read(cx)
            .auth(auth::Action::ResetEmail, cx);
        cx.spawn(async move |this, cx| {
            if let Err(error) = task.await {
                this.update(cx, |this, cx| {
                    this.error = Some(errors::text(&error, cx));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Link {
    Resend,
    ResetEmail(Duration),
}

fn length_of(mode: &Mode) -> usize {
    match mode {
        Mode::Phone(code) => code.delivery.code_length().unwrap_or(FALLBACK_LENGTH),
        Mode::Email(code) if code.length > 0 => code.length,
        Mode::Email(_) => FALLBACK_LENGTH,
    }
}

impl EventEmitter<StepEvent> for CodeStep {}

impl Focusable for CodeStep {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl StepView for CodeStep {
    fn header(&self, cx: &App) -> Header {
        let is_fragment = self.fragment_url().is_some();
        Header {
            title: if is_fragment {
                tr(cx, key::lng_intro_fragment_title)
            } else {
                self.phone_text(cx).into()
            },
            description: self.description(cx),
            has_cover: false,
            has_back: true,
            next: Some(if is_fragment {
                tr(cx, key::lng_intro_fragment_button)
            } else {
                tr(cx, key::lng_intro_next)
            }),
            next_style: if is_fragment {
                &INTRO_FRAGMENT_BUTTON
            } else {
                &INTRO_NEXT_BUTTON
            },
            error: self.error.clone().map(|text| StepError {
                text,
                centered: false,
            }),
            error_top: px_of(INTRO_ERROR_BELOW_LINK_TOP),
        }
    }

    fn submit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(url) = self.fragment_url() {
            cx.open_url(url);
            return;
        }
        self.input.update(cx, |input, cx| input.request(cx));
    }

    fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::focus(&self.input, window, cx);
    }

    fn auth_changed(&mut self, state: &auth::State, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(mode) = mode_of(state) {
            self.set_mode(Some(mode), window, cx);
        }
    }
}

impl Render for CodeStep {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = Geometry::of(window);
        let top = g.content_top(false);
        let code_left = g.content_left() - px_of(SHAKE_SHIFT) - px_of(LINE_WIDTH);
        let code_top = top + px_of(INTRO_STEP_FIELD_TOP) + px_of(INTRO_PHONE_TOP) * 3.0;
        let link_top =
            code_top + px_of(style::intro::INTRO_CODE_DIGIT_HEIGHT) + px_of(INTRO_LINK_TOP);
        let link_left = g.content_left() + px_of(BUTTON_RADIUS);
        let under = if let Some((text, link)) = self.link(cx) {
            let this = cx.entity();
            Some(
                LinkButton::new("code-link", &INTRO_LINK, text)
                    .on_click(move |_, window, cx| {
                        this.update(cx, |this, cx| this.link_clicked(link, window, cx))
                    })
                    .into_any_element(),
            )
        } else {
            self.call_text(cx)
                .or_else(|| self.email_reset_pending(cx))
                .map(|text| {
                    FlatLabel::new("code-call", &INTRO_DESCRIPTION, Markup::plain(text))
                        .width(px_of(INTRO_DESCRIPTION.min_width))
                        .into_any_element()
                })
        };
        div()
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .child(
                div()
                    .absolute()
                    .left(code_left)
                    .top(code_top)
                    .child(self.input.clone()),
            )
            .children(
                under.map(|under| div().absolute().left(link_left).top(link_top).child(under)),
            )
    }
}
