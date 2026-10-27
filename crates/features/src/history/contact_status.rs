//! The bar under the pinned message that offers to add, block or report
//! the other side of a new chat (`ContactStatus`).

use std::rc::Rc;

use chrono::Local;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div,
};
use i18n::key;
use model::action_bar::ActionBar;
use model::chat::{Chat, ChatKind};
use model::chat_actions::Removal;
use style::chat_helpers::{
    HISTORY_CONTACT_STATUS_BLOCK, HISTORY_CONTACT_STATUS_BUTTON, HISTORY_CONTACT_STATUS_LABEL,
    HISTORY_CONTACT_STATUS_MIN_SKIP, HISTORY_REPLY_CANCEL,
};
use style::info::TOP_BAR_ARROW_PADDING;
use style::widgets::{DEFAULT_TOAST, FlatButton};
use ui::{FlatLabel, IconButton, Markup, color, font, font_height};

use super::{HistoryEvent, HistoryView, layout, menu};
use crate::boxes::{self, Callback, ConfirmArgs};
use crate::lang::{format_phone, strings, tr, tr_with};
use crate::peer_menu::{on_confirm, send};
use crate::px_of;

/// What the bar offers (`ContactStatus::State::Type`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Offer {
    Add,
    AddOrBlock,
    UnarchiveOrBlock,
    UnarchiveOrReport,
    ReportSpam,
    Share,
    RequestInfo {
        title: String,
        is_channel: bool,
        date: i32,
    },
}

/// A request that runs after another one.
pub(super) type Step =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), model::RequestError>> + Send>>;

/// A button's action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Press {
    Add,
    Block,
    Unarchive,
    Report,
    Share,
}

impl Offer {
    /// tdesktop words a private chat's spam report as add or block.
    fn of(bar: &ActionBar, is_private: bool) -> Self {
        match bar {
            ActionBar::ReportSpam { can_unarchive }
            | ActionBar::ReportAddBlock { can_unarchive }
                if is_private =>
            {
                if *can_unarchive {
                    Self::UnarchiveOrBlock
                } else {
                    Self::AddOrBlock
                }
            }
            ActionBar::ReportSpam {
                can_unarchive: true,
            } => Self::UnarchiveOrReport,
            ActionBar::ReportSpam { .. } => Self::ReportSpam,
            ActionBar::ReportAddBlock {
                can_unarchive: true,
            } => Self::UnarchiveOrBlock,
            ActionBar::ReportAddBlock { .. } => Self::AddOrBlock,
            ActionBar::AddContact => Self::Add,
            ActionBar::SharePhoneNumber => Self::Share,
            ActionBar::JoinRequest {
                title,
                is_channel,
                request_date,
            } => Self::RequestInfo {
                title: title.clone(),
                is_channel: *is_channel,
                date: *request_date,
            },
        }
    }

    /// The buttons left to right.
    fn presses(&self) -> &'static [Press] {
        match self {
            Self::Add => &[Press::Add],
            Self::AddOrBlock => &[Press::Add, Press::Block],
            Self::UnarchiveOrBlock => &[Press::Unarchive, Press::Block],
            Self::UnarchiveOrReport => &[Press::Unarchive, Press::Report],
            Self::ReportSpam => &[Press::Report],
            Self::Share => &[Press::Share],
            Self::RequestInfo { .. } => &[],
        }
    }
}

impl HistoryView {
    /// The bar, while TDLib offers one for the chat.
    // shortcut: no narrow-window icons, no emoji status notice and no long-text shift away from the close button; with M8 polish.
    pub(super) fn contact_status(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let chat = self.store.read(cx).state().chats.chat(self.chat)?;
        let offer = Offer::of(chat.action_bar.as_ref()?, chat.user().is_some());
        if let Offer::RequestInfo {
            title, is_channel, ..
        } = &offer
        {
            return Some(self.request_info_bar(chat, title, *is_channel, &offer, cx));
        }
        let weak = cx.weak_entity();
        let close = IconButton::new("contact-status-close", &HISTORY_REPLY_CANCEL).on_click(
            move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.close_contact_status(cx));
            },
        );
        let buttons = offer
            .presses()
            .iter()
            .map(|&press| self.status_button(press, offer == Offer::ReportSpam, cx));
        Some(
            div()
                .relative()
                .flex_none()
                .flex()
                .children(buttons)
                .child(div().absolute().top_0().right_0().child(close))
                .into_any_element(),
        )
    }

    fn status_button(&self, press: Press, is_leaving: bool, cx: &Context<Self>) -> AnyElement {
        let (text, st) = match press {
            Press::Add => (self.add_text(cx), &HISTORY_CONTACT_STATUS_BUTTON),
            Press::Block => (
                tr(cx, key::lng_new_contact_block).to_string(),
                &HISTORY_CONTACT_STATUS_BLOCK,
            ),
            Press::Unarchive => (
                tr(cx, key::lng_new_contact_unarchive).to_string(),
                &HISTORY_CONTACT_STATUS_BUTTON,
            ),
            Press::Report if is_leaving => (
                tr(cx, key::lng_report_spam_and_leave).to_string(),
                &HISTORY_CONTACT_STATUS_BLOCK,
            ),
            Press::Report => (
                tr(cx, key::lng_report_spam).to_string(),
                &HISTORY_CONTACT_STATUS_BLOCK,
            ),
            Press::Share => (
                tr(cx, key::lng_new_contact_share).to_string(),
                &HISTORY_CONTACT_STATUS_BUTTON,
            ),
        };
        let weak = cx.weak_entity();
        flat_button(
            ("contact-status", press as usize),
            text.to_uppercase(),
            st,
            cx,
        )
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |this, cx| this.press_status(press, window, cx));
        })
        .into_any_element()
    }

    /// "Add contact", or "Add Ben to contacts" when it is the only button.
    fn add_text(&self, cx: &Context<Self>) -> String {
        let state = self.store.read(cx).state();
        let chat = state.chats.chat(self.chat);
        let is_alone = chat
            .and_then(|chat| chat.action_bar.as_ref())
            .is_some_and(|bar| *bar == ActionBar::AddContact);
        let name = chat
            .and_then(Chat::user)
            .and_then(|user| state.chats.user(user))
            .map(|user| user.first_name.clone());
        match name.filter(|_| is_alone) {
            Some(name) => tr_with(cx, key::lng_new_contact_add_name, &[("user", &name)]),
            None => tr(cx, key::lng_new_contact_add).to_string(),
        }
    }

    fn request_info_bar(
        &self,
        chat: &Chat,
        title: &str,
        is_channel: bool,
        offer: &Offer,
        cx: &Context<Self>,
    ) -> AnyElement {
        let template = if is_channel {
            key::lng_new_contact_from_request_channel
        } else {
            key::lng_new_contact_from_request_group
        };
        let text = bold_fill(
            strings(cx).text(template),
            &[("user", &chat.title), ("name", title)],
        );
        let st = &HISTORY_CONTACT_STATUS_BUTTON;
        let (skip, vskip) = (
            px_of(HISTORY_CONTACT_STATUS_MIN_SKIP),
            px_of(TOP_BAR_ARROW_PADDING.top),
        );
        let (weak, offer) = (cx.weak_entity(), offer.clone());
        div()
            .id("contact-status-request")
            .flex_none()
            .px(skip)
            .py(vskip)
            .bg(color(cx, st.bg_color))
            .cursor_pointer()
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| this.show_request_info(&offer, window, cx));
            })
            .child(FlatLabel::new(
                "contact-status-request-text",
                &HISTORY_CONTACT_STATUS_LABEL,
                text,
            ))
            .into_any_element()
    }

    fn close_contact_status(&self, cx: &mut Context<Self>) {
        let session = self.store.read(cx).session();
        send(
            session.remove_action_bar(self.chat),
            "hide the action bar",
            cx,
        );
    }

    fn press_status(&mut self, press: Press, window: &mut Window, cx: &mut Context<Self>) {
        match press {
            Press::Add => self.add_contact(cx),
            Press::Block => self.confirm_block(window, cx),
            Press::Unarchive => {
                let session = self.store.read(cx).session();
                send(session.unarchive(self.chat), "unarchive", cx);
            }
            Press::Report => self.confirm_report(window, cx),
            Press::Share => self.confirm_share(window, cx),
        }
    }

    /// Adds them under the name they go by.
    // shortcut: no EditContactBox to change the name or share the phone first; the box comes with contacts in M6.
    fn add_contact(&self, cx: &mut Context<Self>) {
        let state = self.store.read(cx).state();
        let Some(user) = state
            .chats
            .chat(self.chat)
            .and_then(Chat::user)
            .and_then(|user| state.chats.user(user))
        else {
            return;
        };
        let (id, first, last) = (user.id, user.first_name.clone(), user.last_name.clone());
        let done = tr_with(cx, key::lng_new_contact_add_done, &[("user", &first)]);
        let task = self.store.read(cx).session().add_contact(id, first, last);
        self.after(task, "add a contact", Some(done), None, cx);
    }

    // shortcut: tdesktop's block box also offers to report and delete the chat (`PeerMenuBlockUserBox`); with M6.
    fn confirm_block(&self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.store.read(cx).state();
        let Some(user) = state.chats.chat(self.chat).and_then(Chat::user) else {
            return;
        };
        let name = state
            .chats
            .chat(self.chat)
            .map_or_else(String::new, |chat| chat.title.clone());
        let confirmed = on_confirm(
            self.store.read(cx).session(),
            move |session| Box::pin(session.block_user(user)),
            "block",
        );
        boxes::confirm_attention(
            &self.layers,
            tr_with(cx, key::lng_blocked_list_confirm_text, &[("name", &name)]),
            tr(cx, key::lng_blocked_list_confirm_ok),
            confirmed,
            window,
            cx,
        );
    }

    /// Reports the group or channel, leaves it and goes back.
    fn confirm_report(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chat) = self.store.read(cx).state().chats.chat(self.chat) else {
            return;
        };
        let (sure, removal) = match chat.kind {
            ChatKind::Group { .. } => (key::lng_report_spam_sure_group, Removal::LeaveAndDelete),
            ChatKind::Supergroup {
                is_channel: false, ..
            } => (key::lng_report_spam_sure_group, Removal::Leave),
            ChatKind::Supergroup { .. } => (key::lng_report_spam_sure_channel, Removal::Leave),
            ChatKind::Private { .. } | ChatKind::Secret { .. } => return,
        };
        let id = self.chat;
        let weak = cx.weak_entity();
        let confirmed: Callback = Rc::new(move |_, cx| {
            let _ = weak.update(cx, |this, cx| {
                let session = this.store.read(cx).session();
                let (report, leave) = (session.report_spam(id), session.remove_chat(id, removal));
                let done = tr(cx, key::lng_report_spam_done).to_string();
                this.after(report, "report spam", Some(done), Some(Box::pin(leave)), cx);
                cx.emit(HistoryEvent::Close);
            });
        });
        boxes::confirm_attention(
            &self.layers,
            tr(cx, sure).to_string(),
            tr(cx, key::lng_report_spam_ok),
            confirmed,
            window,
            cx,
        );
    }

    fn confirm_share(&self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.store.read(cx).state();
        let Some(user) = state
            .chats
            .chat(self.chat)
            .and_then(Chat::user)
            .and_then(|user| state.chats.user(user))
        else {
            return;
        };
        let phone = state
            .me
            .and_then(|me| state.chats.user(me))
            .map_or_else(String::new, |me| format_phone(me.phone.expose()));
        let text = bold_fill(
            strings(cx).text(key::lng_new_contact_share_sure),
            &[("phone", &phone), ("user", &user.name())],
        );
        let (id, done) = (
            user.id,
            tr_with(
                cx,
                key::lng_new_contact_share_done,
                &[("user", &user.first_name)],
            ),
        );
        let weak = cx.weak_entity();
        let mut args = ConfirmArgs::new(text);
        args.confirm_text = Some(tr(cx, key::lng_box_ok));
        args.cancel_text = Some(tr(cx, key::lng_cancel));
        args.confirmed = Some(Rc::new(move |_, cx| {
            let _ = weak.update(cx, |this, cx| {
                let task = this.store.read(cx).session().share_phone_number(id);
                this.after(task, "share the phone number", Some(done.clone()), None, cx);
            });
        }));
        boxes::confirm(&self.layers, args, window, cx);
    }

    /// Why the admin wrote; "I understand" hides the bar.
    fn show_request_info(&self, offer: &Offer, window: &mut Window, cx: &mut Context<Self>) {
        let Offer::RequestInfo {
            title,
            is_channel,
            date,
        } = offer
        else {
            return;
        };
        let now = Local::now();
        let day = layout::day(*date, &Local).unwrap_or_else(|| now.date_naive());
        let when = tr_with(
            cx,
            key::lng_mediaview_date_time,
            &[
                ("date", &layout::date_badge(strings(cx), day, &now)),
                ("time", &layout::time(*date, &Local)),
            ],
        );
        let body = bold_fill(
            strings(cx).text(key::lng_from_request_body),
            &[("name", title), ("date", &when)],
        );
        let title_key = if *is_channel {
            key::lng_from_request_title_channel
        } else {
            key::lng_from_request_title_group
        };
        let weak = cx.weak_entity();
        let mut args = ConfirmArgs::new(body);
        args.title = Some(tr(cx, title_key));
        args.confirm_text = Some(tr(cx, key::lng_from_request_understand));
        args.confirmed = Some(Rc::new(move |_, cx| {
            let _ = weak.update(cx, |this, cx| this.close_contact_status(cx));
        }));
        args.strict_cancel = true;
        boxes::confirm(&self.layers, args, window, cx);
    }

    /// Runs `task`, then `next`; toasts `done` once `task` succeeds.
    pub(super) fn after(
        &self,
        task: impl std::future::Future<Output = Result<(), model::RequestError>> + Send + 'static,
        what: &'static str,
        done: Option<String>,
        next: Option<Step>,
        cx: &mut Context<Self>,
    ) {
        let task = crate::store::spawn(
            async move {
                let result = task.await;
                if let Some(next) = next
                    && let Err(error) = next.await
                {
                    tracing::warn!(%error, what, "the step after a chat action failed");
                }
                result
            },
            cx,
        );
        let layers = self.layers.clone();
        cx.spawn(async move |_, cx| match task.await {
            Ok(()) => {
                if let Some(text) = done {
                    layers.update(cx, |layers, cx| {
                        layers.show_toast(text.into(), &DEFAULT_TOAST, menu::TOAST_DURATION, cx);
                    });
                }
            }
            Err(error) => tracing::warn!(%error, what, "chat action failed"),
        })
        .detach();
    }
}

/// A full-width flat button of the bar (`FlatButton`).
// shortcut: no ripple on press; ripples come with M8 polish.
pub(super) fn flat_button(
    id: (&'static str, usize),
    text: String,
    st: &'static FlatButton,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_1()
        .min_w_0()
        .h(px_of(st.height))
        .pt(px_of(st.text_top))
        .text_center()
        .overflow_hidden()
        .whitespace_nowrap()
        .cursor_pointer()
        .bg(color(cx, st.bg_color))
        .hover(|this| this.bg(color(cx, st.over_bg_color)))
        .font(font(st.font))
        .text_size(px_of(st.font.size))
        .line_height(font_height(st.font))
        .text_color(color(cx, st.color))
        .child(SharedString::from(text))
}

/// `template` with each `{name}` replaced by its value in bold
/// (`tr::bold`).
fn bold_fill(template: &str, args: &[(&str, &str)]) -> Markup {
    let mut markup = Markup::default();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}').map(|end| start + end) else {
            break;
        };
        let name = &rest[start + 1..end];
        match args.iter().find(|(arg, _)| *arg == name) {
            Some((_, value)) => markup = markup.text(&rest[..start]).bold(value),
            None => markup = markup.text(&rest[..=end]),
        }
        rest = &rest[end + 1..];
    }
    markup.text(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_private_spam_report_offers_add_or_block() {
        let spam = ActionBar::ReportSpam {
            can_unarchive: false,
        };
        assert_eq!(Offer::of(&spam, true), Offer::AddOrBlock);
        assert_eq!(Offer::of(&spam, false), Offer::ReportSpam);
        let archived = ActionBar::ReportSpam {
            can_unarchive: true,
        };
        assert_eq!(
            Offer::of(&archived, false).presses(),
            [Press::Unarchive, Press::Report]
        );
    }

    #[test]
    fn placeholders_turn_bold_and_others_stay() {
        let markup = bold_fill(
            "{user} is an admin of {name} {x}",
            &[("user", "Ann"), ("name", "Hikers")],
        );
        assert_eq!(
            markup,
            Markup::default()
                .bold("Ann")
                .text(" is an admin of ")
                .bold("Hikers")
                .text(" {x}")
        );
    }
}
