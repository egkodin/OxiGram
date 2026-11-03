//! The profile's members (`Info::Profile::Members`) and actions
//! (`ActionsFiller`): send a message, share, delete or block the contact,
//! report, leave or delete the chat.

use std::rc::Rc;

use chrono::Local;
use gpui::{AnyElement, App, Context, IntoElement, Styled as _, Window, prelude::*};
use i18n::key;
use model::ChatId;
use model::profile::Blocking;
use model::user::UserKind;
use style::info::{
    INFO_BLOCK_BUTTON, INFO_BLOCK_HEADER_LABEL, INFO_BLOCK_HEADER_POSITION, INFO_ICON_BLOCK,
    INFO_ICON_DELETE_RED, INFO_ICON_REPORT, INFO_MEMBERS_HEADER, INFO_PROFILE_BUTTON,
};
use ui::SettingsButton;

use super::{InfoView, Subject, divider, text};
use crate::boxes::{self, Callback};
use crate::dialogs::photo_userpic;
use crate::lang::{strings, tr, tr_plural, tr_with};
use crate::peer_list::{self, Look};
use crate::peer_menu::{on_confirm, send};
use crate::px_of;

impl InfoView {
    pub(super) fn members_section(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if self.members.is_empty() {
            return Vec::new();
        }
        let count = self.group.as_ref().map_or(self.members.len(), |group| {
            usize::try_from(group.member_count).unwrap_or(0)
        });
        let header = text(
            tr_plural(
                cx,
                key::lng_chat_status_members,
                i64::try_from(count).unwrap_or(0),
                &[],
            ),
            &INFO_BLOCK_HEADER_LABEL,
            cx,
        )
        .h(px_of(INFO_MEMBERS_HEADER))
        .pl(px_of(INFO_BLOCK_HEADER_POSITION.x))
        .pt(px_of(INFO_BLOCK_HEADER_POSITION.y));
        let mut rows = vec![header.into_any_element()];
        let members: Vec<_> = self.members.iter().map(|member| member.user).collect();
        for (index, user) in members.into_iter().enumerate() {
            if let Some(row) = self.member_row(index, user, cx) {
                rows.push(row);
            }
        }
        rows.push(divider(cx).into_any_element());
        rows
    }

    fn member_row(
        &mut self,
        index: usize,
        user: model::UserId,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.store().read(cx).state();
        let found = state.chats.user(user)?;
        let name = found.name();
        let (userpic, download) = photo_userpic(
            &state.chats,
            found.photo.as_ref(),
            found.accent_color,
            &name,
        );
        let status = crate::status::status(strings(cx), found, &Local::now());
        if let Some(file) = download {
            self.download(file, cx);
        }
        let look = Look {
            name: name.into(),
            userpic,
            status: status.text.into(),
            is_active: status.is_online,
        };
        let targets = self.targets.clone();
        Some(
            peer_list::row(
                ("info-member", index),
                look,
                self.hovered == Some(index),
                cx,
            )
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                let next = peer_list::hovered_after(this.hovered, index, *hovered);
                if this.hovered != next {
                    this.hovered = next;
                    cx.notify();
                }
            }))
            // A private chat's id is its user's id.
            .on_click(move |_, window, cx| {
                super::show(targets.clone(), ChatId(user.0), window, cx);
            })
            .into_any_element(),
        )
    }

    pub(super) fn actions(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if self.is_self(cx) {
            return Vec::new();
        }
        let mut rows = Vec::new();
        if let Subject::User(user) = self.subject {
            self.user_actions(user, &mut rows, cx);
        }
        let entity = cx.entity();
        if self.is_reportable(cx) {
            rows.push(
                SettingsButton::new(
                    "info-report",
                    &INFO_BLOCK_BUTTON,
                    tr(cx, key::lng_profile_report),
                )
                .icon(INFO_ICON_REPORT)
                .on_click(move |_, window, cx| {
                    entity.update(cx, |this, cx| this.report(window, cx));
                })
                .into_any_element(),
            );
        }
        if let Some(removal) = self.removal(cx) {
            rows.push(removal);
        }
        rows
    }

    /// Bots, groups and channels (`ReportAction`); users report from the
    /// chat's bar.
    fn is_reportable(&self, cx: &App) -> bool {
        match self.subject {
            Subject::User(user) => self
                .store()
                .read(cx)
                .state()
                .chats
                .user(user)
                .is_some_and(|user| user.kind == UserKind::Bot),
            Subject::Group(_) | Subject::Supergroup { .. } => true,
        }
    }

    fn user_actions(&self, id: model::UserId, rows: &mut Vec<AnyElement>, cx: &mut Context<Self>) {
        let state = self.store().read(cx).state();
        let Some(user) = state.chats.user(id).cloned() else {
            return;
        };
        let entity = cx.entity();
        rows.push(
            SettingsButton::new(
                "info-send",
                &INFO_PROFILE_BUTTON,
                tr(cx, key::lng_profile_send_message),
            )
            .on_click(move |_, window, cx| {
                entity.update(cx, |this, cx| this.send_message(window, cx))
            })
            .into_any_element(),
        );
        let (session, layers, store) = (
            self.store().read(cx).session().clone(),
            self.layers().clone(),
            self.store().clone(),
        );
        if user.is_contact && !user.phone.is_empty() {
            let (session, layers, store, user) =
                (session.clone(), layers.clone(), store.clone(), user.clone());
            rows.push(
                SettingsButton::new(
                    "info-share",
                    &INFO_PROFILE_BUTTON,
                    tr(cx, key::lng_info_share_contact),
                )
                .on_click(move |_, window, cx| {
                    let (session, user) = (session.clone(), user.clone());
                    let on_chosen: crate::recipients_box::OnChosen = Rc::new(move |chat, _, cx| {
                        send(session.send_contact(chat, &user), "share contact", cx);
                    });
                    crate::recipients_box::show(&store, &layers, on_chosen, window, cx);
                })
                .into_any_element(),
            );
        }
        if user.is_contact {
            let question = tr_with(
                cx,
                key::lng_sure_delete_contact,
                &[("contact", &user.name())],
            );
            let confirmed = on_confirm(
                &session,
                move |session| Box::pin(session.remove_contact(id)),
                "delete contact",
            );
            rows.push(self.confirmed_row(
                "info-delete-contact",
                key::lng_info_delete_contact,
                question,
                confirmed,
                cx,
            ));
        }
        if matches!(user.kind, UserKind::Regular | UserKind::Bot) {
            rows.push(self.block_row(id, &user.name(), cx));
        }
    }

    fn block_row(&self, id: model::UserId, name: &str, cx: &mut Context<Self>) -> AnyElement {
        let is_blocked = self.user.as_ref().is_some_and(|profile| profile.is_blocked);
        let session = self.store().read(cx).session().clone();
        let weak = cx.weak_entity();
        let blocking = if is_blocked {
            Blocking::Unblocked
        } else {
            Blocking::Blocked
        };
        let block: Callback = Rc::new(move |_, cx| {
            let task = session.set_blocked(id, blocking);
            let _ = weak.update(cx, |this, cx| {
                // The block state comes from `userFullInfo`.
                this.load(task, "block", |this, (), cx| this.reload(cx), cx);
            });
        });
        if is_blocked {
            // Unblocking asks nothing (`Window::PeerMenuUnblockUserWithBotRestart`).
            return SettingsButton::new(
                "info-block",
                &INFO_BLOCK_BUTTON,
                tr(cx, key::lng_profile_unblock_user),
            )
            .icon(INFO_ICON_BLOCK)
            .on_click(move |_, window, cx| block(window, cx))
            .into_any_element();
        }
        let question = tr_with(cx, key::lng_blocked_list_confirm_text, &[("name", name)]);
        SettingsButton::new(
            "info-block",
            &INFO_BLOCK_BUTTON,
            tr(cx, key::lng_profile_block_user),
        )
        .icon(INFO_ICON_BLOCK)
        .on_click({
            let layers = self.layers().clone();
            let button = tr(cx, key::lng_blocked_list_confirm_ok);
            move |_, window, cx| {
                boxes::confirm_attention(
                    &layers,
                    question.clone(),
                    button.clone(),
                    block.clone(),
                    window,
                    cx,
                );
            }
        })
        .into_any_element()
    }

    fn confirmed_row(
        &self,
        id: &'static str,
        label: i18n::Key,
        question: String,
        confirmed: Callback,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let layers = self.layers().clone();
        let button = tr(cx, key::lng_box_delete);
        SettingsButton::new(id, &INFO_BLOCK_BUTTON, tr(cx, label))
            .icon(INFO_ICON_DELETE_RED)
            .on_click(move |_, window, cx| {
                boxes::confirm_attention(
                    &layers,
                    question.clone(),
                    button.clone(),
                    confirmed.clone(),
                    window,
                    cx,
                );
            })
            .into_any_element()
    }

    /// Delete the chat, or leave the group or channel, as its menu does.
    fn removal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.store().read(cx).state();
        let chat = self.chat(state)?;
        let (text, _, removal, question, _) = crate::peer_menu::removal(chat, cx);
        let id = chat.id;
        let session = self.store().read(cx).session().clone();
        let confirmed = on_confirm(
            &session,
            move |session| Box::pin(session.remove_chat(id, removal)),
            "delete chat",
        );
        let layers = self.layers().clone();
        let button = tr(cx, key::lng_box_leave);
        let button = if matches!(removal, model::chat_actions::Removal::DeleteHistory) {
            tr(cx, key::lng_box_delete)
        } else {
            button
        };
        Some(
            SettingsButton::new("info-removal", &INFO_BLOCK_BUTTON, text)
                .icon(INFO_ICON_DELETE_RED)
                .on_click(move |_, window, cx| {
                    boxes::confirm_attention(
                        &layers,
                        question.clone(),
                        button.clone(),
                        confirmed.clone(),
                        window,
                        cx,
                    );
                })
                .into_any_element(),
        )
    }

    /// Opens the private chat, creating it when it is not loaded yet.
    fn send_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.chat(self.store().read(cx).state()).is_some() {
            self.open_chat(self.chat, window, cx);
            return;
        }
        let Subject::User(user) = self.subject else {
            return;
        };
        let task = crate::store::spawn(
            self.store().read(cx).session().create_private_chat(user),
            cx,
        );
        cx.spawn_in(window, async move |this, cx| match task.await {
            Ok(chat) => {
                let _ = this.update_in(cx, |this, window, cx| this.open_chat(chat, window, cx));
            }
            Err(error) => tracing::warn!(%error, "opening a private chat failed"),
        })
        .detach();
    }
}
