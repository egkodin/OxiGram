//! Login email setup (`intro_email.cpp`): the server asks for an email to
//! send login codes to.

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Window, div, prelude::*,
};
use i18n::key;
use model::{Secret, auth};
use style::intro::{
    INTRO_DESCRIPTION, INTRO_ERROR_TOP, INTRO_NEXT_BUTTON, INTRO_PASSWORD, INTRO_STEP_FIELD_TOP,
};
use ui::{InputEvent, InputField, Markup};

use super::errors;
use super::step::{Geometry, Header, Shared, StepError, StepEvent, StepView};
use crate::lang::tr;
use crate::px_of;

pub(crate) struct EmailStep {
    shared: Shared,
    focus: FocusHandle,
    email: Entity<InputField>,
    is_sent: bool,
    error: Option<SharedString>,
    _subscription: Subscription,
}

impl std::fmt::Debug for EmailStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailStep").finish_non_exhaustive()
    }
}

impl EmailStep {
    pub fn new(shared: Shared, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let placeholder = tr(cx, key::lng_settings_cloud_login_email_placeholder);
        let email = cx.new(|cx| InputField::new(&INTRO_PASSWORD, placeholder, window, cx));
        let subscription = cx.subscribe_in(
            &email,
            window,
            |this: &mut EmailStep, _, event, window, cx| match event {
                InputEvent::Changed => {
                    if this.error.take().is_some() {
                        cx.notify();
                    }
                }
                InputEvent::Submitted => this.submit(window, cx),
                _ => {}
            },
        );
        Self {
            shared,
            focus: cx.focus_handle(),
            email,
            is_sent: false,
            error: None,
            _subscription: subscription,
        }
    }

    fn failed(&mut self, error: &auth::Error, window: &mut Window, cx: &mut Context<Self>) {
        crate::focus(&self.email, window, cx);
        self.email.update(cx, |field, cx| {
            field.show_error(cx);
            field.select_all(cx);
        });
        self.error = Some(errors::text(error, cx));
        cx.notify();
    }
}

impl EventEmitter<StepEvent> for EmailStep {}

impl Focusable for EmailStep {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl StepView for EmailStep {
    fn header(&self, cx: &App) -> Header {
        Header {
            title: tr(cx, key::lng_intro_email_setup_title),
            description: Markup::plain(
                tr(cx, key::lng_settings_cloud_login_email_about).to_string(),
            ),
            has_cover: false,
            has_back: true,
            next: Some(tr(cx, key::lng_intro_next)),
            next_style: &INTRO_NEXT_BUTTON,
            error: self.error.clone().map(|text| StepError {
                text,
                centered: false,
            }),
            error_top: px_of(INTRO_ERROR_TOP),
        }
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_sent {
            return;
        }
        let email = self.email.read(cx).text().trim().to_owned();
        if email.is_empty() {
            crate::focus(&self.email, window, cx);
            self.email.update(cx, |field, cx| field.show_error(cx));
            return;
        }
        self.is_sent = true;
        let task = self
            .shared
            .store
            .read(cx)
            .auth(auth::Action::SetEmail(Secret::new(email)), cx);
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

    fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::focus(&self.email, window, cx);
    }
}

impl Render for EmailStep {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let g = Geometry::of(window);
        // shortcut: the title sits in the step header, the field moves down one line for the three-line description, and the Lottie email icon is left out; the icon comes with Lottie in M5.
        div().track_focus(&self.focus).size_full().relative().child(
            div()
                .absolute()
                .left(g.content_left())
                .top(
                    g.content_top(false)
                        + px_of(INTRO_STEP_FIELD_TOP)
                        + px_of(INTRO_DESCRIPTION.style.line_height),
                )
                .w(px_of(INTRO_NEXT_BUTTON.width))
                .child(self.email.clone()),
        )
    }
}
