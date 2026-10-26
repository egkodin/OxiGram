//! The welcome screen (`intro_start.cpp`) with its cover. rgram names
//! itself here, not Telegram Desktop.

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window, div,
    prelude::*,
};
use i18n::key;
use ui::Markup;

use super::step::{Header, Kind, StepEvent, StepView};
use crate::lang::tr;

pub(crate) struct StartStep {
    focus: FocusHandle,
}

impl std::fmt::Debug for StartStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartStep").finish_non_exhaustive()
    }
}

impl StartStep {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
        }
    }
}

impl EventEmitter<StepEvent> for StartStep {}

impl Focusable for StartStep {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl StepView for StartStep {
    fn header(&self, cx: &App) -> Header {
        Header {
            title: tr(cx, key::rgram_app_name),
            description: Markup::plain(tr(cx, key::rgram_intro_about).to_string()),
            has_cover: true,
            has_back: false,
            next: Some(tr(cx, key::lng_start_msgs)),
            next_style: &style::intro::INTRO_NEXT_BUTTON,
            error: None,
            error_top: gpui::Pixels::ZERO,
        }
    }

    /// `goNext<QrWidget>()`: QR login is where tdesktop starts; the QR step
    /// asks TDLib for a code.
    fn submit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(StepEvent::NextOrBack(Kind::Qr));
    }
}

impl Render for StartStep {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().track_focus(&self.focus).size_full()
    }
}
