//! QR code login (`intro_qr.cpp`): another signed-in device scans the code.

use std::sync::Arc;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    SharedString, Styled, Window, div, prelude::*, px,
};
use i18n::key;
use model::auth;
use style::intro::{
    INTRO_QR_ERROR_TOP, INTRO_QR_LABELS_WIDTH, INTRO_QR_SKIP_TOP, INTRO_QR_STEP,
    INTRO_QR_STEP_MARGINS, INTRO_QR_STEPS_TOP, INTRO_QR_TITLE, INTRO_QR_TITLE_TOP,
    INTRO_QR_TITLE_WIDTH, INTRO_QR_TOP,
};
use style::widgets::{DEFAULT_FLAT_LABEL, DEFAULT_LINK_BUTTON};
use ui::{FlatLabel, LinkButton, Markup, QrCode, QrData};

use super::errors;
use super::step::{Geometry, Header, Kind, Shared, StepError, StepEvent, StepView};
use crate::lang::tr;
use crate::px_of;

pub(crate) struct QrStep {
    shared: Shared,
    focus: FocusHandle,
    /// The link the code encodes; compared to skip re-encoding.
    link: Option<model::Secret>,
    code: Option<Arc<QrData>>,
    is_requesting: bool,
    error: Option<SharedString>,
}

impl std::fmt::Debug for QrStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QrStep")
            .field("has_code", &self.code.is_some())
            .finish_non_exhaustive()
    }
}

impl QrStep {
    pub fn new(shared: Shared, cx: &mut Context<Self>) -> Self {
        let mut step = Self {
            shared,
            focus: cx.focus_handle(),
            link: None,
            code: None,
            is_requesting: false,
            error: None,
        };
        let state = step.shared.auth(cx).clone();
        step.update_code(&state, cx);
        step
    }

    fn update_code(&mut self, state: &auth::State, cx: &mut Context<Self>) {
        match state {
            auth::State::WaitQrScan { link } => {
                self.is_requesting = false;
                if self.link.as_ref() == Some(link) {
                    return;
                }
                match QrData::encode(link.expose()) {
                    Ok(code) => self.code = Some(Arc::new(code)),
                    Err(error) => tracing::error!(%error, "encoding the login QR code failed"),
                }
                self.link = Some(link.clone());
                self.error = None;
                cx.notify();
            }
            // TDLib accepts the request in these states; the code arrives
            // as WaitQrScan.
            auth::State::WaitPhoneNumber
            | auth::State::WaitCode(_)
            | auth::State::WaitPassword(_)
            | auth::State::WaitRegistration { .. }
            | auth::State::WaitEmailAddress
            | auth::State::WaitEmailCode(_)
            | auth::State::WaitPremiumPurchase { .. } => self.request(cx),
            _ => {}
        }
    }

    fn request(&mut self, cx: &mut Context<Self>) {
        if self.is_requesting {
            return;
        }
        self.is_requesting = true;
        let task = self
            .shared
            .store
            .read(cx)
            .auth(auth::Action::RequestQrCode, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if let Err(error) = result {
                    this.is_requesting = false;
                    this.error = Some(errors::text(&error, cx));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// `QrWidget::submit`: on to the phone number. TDLib takes a number only
    /// after leaving QR login, which takes a new client (log out).
    fn switch_to_phone(&mut self, cx: &mut Context<Self>) {
        if matches!(self.shared.auth(cx), auth::State::WaitQrScan { .. }) {
            let task = self.shared.store.read(cx).auth(auth::Action::LogOut, cx);
            cx.spawn(async move |_, _| {
                if let Err(error) = task.await {
                    tracing::warn!(%error, "leaving QR login failed");
                }
            })
            .detach();
        }
        cx.emit(StepEvent::NextOrBack(Kind::Phone));
    }
}

impl EventEmitter<StepEvent> for QrStep {}

impl Focusable for QrStep {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl StepView for QrStep {
    fn header(&self, _: &App) -> Header {
        Header {
            title: SharedString::default(),
            description: Markup::default(),
            has_cover: false,
            has_back: true,
            next: None,
            next_style: &style::intro::INTRO_NEXT_BUTTON,
            error: self.error.clone().map(|text| StepError {
                text,
                centered: true,
            }),
            error_top: px_of(INTRO_QR_ERROR_TOP),
        }
    }

    fn submit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_to_phone(cx);
    }

    fn auth_changed(&mut self, state: &auth::State, _: &mut Window, cx: &mut Context<Self>) {
        self.update_code(state, cx);
    }
}

impl Render for QrStep {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = Geometry::of(window);
        let top = g.content_top(false);
        let centered = |y: gpui::Pixels| {
            div()
                .absolute()
                .left_0()
                .w(g.width)
                .top(y)
                .flex()
                .justify_center()
        };
        let margins = INTRO_QR_STEP_MARGINS;
        let number_width = px_of(margins.left);
        let steps = [
            key::lng_intro_qr_step1,
            key::lng_intro_qr_step2,
            key::lng_intro_qr_step3,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, step)| {
            div()
                .flex()
                .pt(px_of(margins.top))
                .child(
                    div()
                        .w(number_width)
                        .flex()
                        .justify_end()
                        .pr(px(3.0))
                        .child(FlatLabel::new(
                            ("qr-step-number", index),
                            &DEFAULT_FLAT_LABEL,
                            Markup::default().bold(&format!("{}.", index + 1)),
                        )),
                )
                .child(
                    FlatLabel::new(
                        ("qr-step", index),
                        &INTRO_QR_STEP,
                        Markup::rich(&tr(cx, step)),
                    )
                    .width(px_of(INTRO_QR_LABELS_WIDTH) - number_width),
                )
        });
        let this = cx.entity();
        div()
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .child(
                centered(top + px_of(INTRO_QR_TOP))
                    .child(QrCode::new("intro-qr", self.code.clone())),
            )
            .child(
                centered(top + px_of(INTRO_QR_TITLE_TOP)).child(
                    FlatLabel::new(
                        "qr-title",
                        &INTRO_QR_TITLE,
                        Markup::plain(tr(cx, key::lng_intro_qr_title).to_string()),
                    )
                    .width(px_of(INTRO_QR_TITLE_WIDTH)),
                ),
            )
            .child(
                centered(top + px_of(INTRO_QR_STEPS_TOP)).child(
                    div()
                        .w(px_of(INTRO_QR_LABELS_WIDTH))
                        .flex()
                        .flex_col()
                        .children(steps),
                ),
            )
            .child(
                centered(top + px_of(INTRO_QR_SKIP_TOP)).child(
                    LinkButton::new(
                        "qr-skip",
                        &DEFAULT_LINK_BUTTON,
                        tr(cx, key::lng_intro_qr_phone),
                    )
                    .on_click(move |_, _, cx| this.update(cx, |this, cx| this.switch_to_phone(cx))),
                ),
            )
    }
}
