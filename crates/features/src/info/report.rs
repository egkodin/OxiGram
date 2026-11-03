//! "Report" from a profile (`ShowReportMessageBox` over TDLib's
//! `reportChat`): the reasons TDLib offers, then a comment when it asks
//! for one, then the thanks toast.

use std::time::Duration;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
};
use i18n::key;
use model::ChatId;
use model::profile::Report;
use style::boxes::INFO_LABEL;
use style::info::INFO_PROFILE_BUTTON;
use style::layers::DEFAULT_BOX;
use style::widgets::{DEFAULT_INPUT_FIELD, DEFAULT_TOAST};
use ui::{BoxFrame, InputField, RoundButton, SettingsButton};

use super::{InfoView, text};
use crate::lang::tr;
use crate::links::Targets;
use crate::px_of;

/// `Ui::Toast::kDefaultDuration`.
const TOAST_DURATION: Duration = Duration::from_millis(1500);
/// TDLib's limit for a report's comment.
const MAX_COMMENT: usize = 512;

enum Step {
    Options(Vec<(String, String)>),
    Comment {
        option: String,
        field: Entity<InputField>,
        is_optional: bool,
    },
}

struct ReportBox {
    title: SharedString,
    step: Step,
    targets: Targets,
    chat: ChatId,
    focus: FocusHandle,
}

impl Focusable for ReportBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl InfoView {
    pub(super) fn report(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        send(
            self.targets.clone(),
            self.chat,
            String::new(),
            String::new(),
            window,
            cx,
        );
    }
}

/// Sends one step and shows what TDLib answers.
fn send(
    targets: Targets,
    chat: ChatId,
    option: String,
    comment: String,
    window: &mut Window,
    cx: &mut App,
) {
    let task = targets
        .store
        .read(cx)
        .session()
        .report_chat(chat, option, comment);
    let task = crate::store::spawn(task, cx);
    window
        .spawn(cx, async move |cx| match task.await {
            Ok(report) => {
                let _ = cx.update(|window, cx| answered(targets, chat, report, window, cx));
            }
            Err(error) => tracing::warn!(%error, "report failed"),
        })
        .detach();
}

fn answered(targets: Targets, chat: ChatId, report: Report, window: &mut Window, cx: &mut App) {
    let layers = targets.layers.clone();
    let step = match report {
        Report::Sent | Report::Messages => {
            // shortcut: reports that need messages picked thank too; tdesktop asks to select them in the chat (M6 report from history).
            let thanks = tr(cx, key::lng_report_thanks);
            layers.update(cx, |layers, cx| {
                layers.close_all(window, cx);
                layers.show_toast(thanks, &DEFAULT_TOAST, TOAST_DURATION, cx);
            });
            return;
        }
        Report::Options { title, options } => (title.into(), Step::Options(options)),
        Report::Text {
            option,
            is_optional,
        } => {
            let placeholder = if is_optional {
                tr(cx, key::lng_report_details_optional)
            } else {
                tr(cx, key::lng_report_details_non_optional)
            };
            let field = cx.new(|cx| {
                InputField::new(&DEFAULT_INPUT_FIELD, placeholder, window, cx)
                    .with_max_length(MAX_COMMENT)
            });
            field.update(cx, |field, cx| field.focus(window, cx));
            (
                tr(cx, key::lng_report_details),
                Step::Comment {
                    option,
                    field,
                    is_optional,
                },
            )
        }
    };
    let (title, step) = step;
    let view = cx.new(|cx| ReportBox {
        title,
        step,
        targets,
        chat,
        focus: cx.focus_handle(),
    });
    layers.update(cx, |layers, cx| layers.show_above(view, window, cx));
}

impl ReportBox {
    fn submit(&self, window: &mut Window, cx: &mut App) {
        let Step::Comment {
            option,
            field,
            is_optional,
        } = &self.step
        else {
            return;
        };
        let comment = field.read(cx).text().trim().to_owned();
        if comment.is_empty() && !is_optional {
            field.update(cx, |field, cx| field.show_error(cx));
            return;
        }
        send(
            self.targets.clone(),
            self.chat,
            option.clone(),
            comment,
            window,
            cx,
        );
    }
}

impl Render for ReportBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let mut frame = BoxFrame::new("info-report", &DEFAULT_BOX).title(self.title.clone());
        match &self.step {
            Step::Options(options) => {
                let rows = options.iter().enumerate().map(|(index, (id, label))| {
                    let (targets, chat, id) = (self.targets.clone(), self.chat, id.clone());
                    SettingsButton::new(
                        ("info-report-option", index),
                        &INFO_PROFILE_BUTTON,
                        label.clone(),
                    )
                    .on_click(move |_, window, cx| {
                        send(targets.clone(), chat, id.clone(), String::new(), window, cx);
                    })
                });
                frame = frame.row(div().children(rows));
            }
            Step::Comment { field, .. } => {
                let pad = px_of(style::layers::BOX_PADDING.left);
                frame = frame
                    .row(div().px(pad).child(field.clone()))
                    .row(
                        div().px(pad).pt(pad).child(
                            text(tr(cx, key::lng_report_details_about), &INFO_LABEL, cx)
                                .whitespace_normal(),
                        ),
                    )
                    .button(
                        RoundButton::new(
                            "info-report-send",
                            &DEFAULT_BOX.button,
                            tr(cx, key::lng_report_button),
                        )
                        .on_click(move |_, window, cx| {
                            entity.update(cx, |this, cx| this.submit(window, cx))
                        }),
                    );
            }
        }
        let layers = self.targets.layers.clone();
        div().track_focus(&self.focus).child(
            frame.button(
                RoundButton::new(
                    "info-report-cancel",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_cancel),
                )
                .on_click(move |_, window, cx| {
                    layers.update(cx, |layers, cx| layers.close(window, cx));
                }),
            ),
        )
    }
}
