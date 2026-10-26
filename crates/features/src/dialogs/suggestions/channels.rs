//! The suggestions' channels and apps tabs (`MyChannelsController`,
//! `RecommendationsController`, `RecentAppsController`).

use std::cmp::Reverse;

use gpui::{AnyElement, Context};
use i18n::{Key, key};
use model::ChatId;
use model::chat::{ChatKind, ChatList};

use super::super::Dialogs;
use super::super::search::{bar, bar_link};
use super::Suggestions;
use super::chats::empty_text;
use super::row::{Counter, RowMenu};
use crate::lang::tr;

/// Rows a long section shows until "Show more" (`kCollapsedChannelsCount`,
/// `kCollapsedAppsCount`).
const COLLAPSED_ROWS: usize = 5;

/// A titled list that folds to its first rows (`setupExpandDivider`).
struct Section {
    id: &'static str,
    title: Key,
    chats: Vec<ChatId>,
    is_expanded: bool,
    toggle: fn(&mut Suggestions),
    menu: RowMenu,
}

impl Dialogs {
    /// Channels the user joined, the most recently active first, from the
    /// main list and the archive.
    fn joined_channels(&self, cx: &Context<Self>) -> Vec<ChatId> {
        let chats = &self.store.read(cx).state().chats;
        let mut channels: Vec<(i32, ChatId)> = chats
            .list(ChatList::Main)
            .chain(chats.list(ChatList::Archive))
            .filter_map(|id| chats.chat(id))
            .filter(|chat| {
                matches!(
                    chat.kind,
                    ChatKind::Supergroup {
                        is_channel: true,
                        ..
                    }
                )
            })
            .map(|chat| {
                let date = chat.last_message.as_ref().map_or(0, |message| message.date);
                (date, chat.id)
            })
            .collect();
        channels.sort_by_key(|(date, _)| Reverse(*date));
        channels.into_iter().map(|(_, id)| id).collect()
    }

    pub(super) fn channels_tab(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(suggestions) = &self.suggestions else {
            return Vec::new();
        };
        let is_expanded = suggestions.channels_expanded;
        let recommended = suggestions.recommended.clone().unwrap_or_default();
        let joined = self.joined_channels(cx);
        let mut children = if joined.is_empty() {
            vec![empty_text(
                "channels-none",
                tr(cx, key::lng_channels_none_about),
            )]
        } else {
            self.section(
                Section {
                    id: "channels-more",
                    title: key::lng_channels_your_title,
                    chats: joined,
                    is_expanded,
                    toggle: |suggestions| {
                        suggestions.channels_expanded = !suggestions.channels_expanded;
                    },
                    menu: RowMenu::Chat,
                },
                cx,
            )
        };
        if !recommended.is_empty() {
            // shortcut: no Join button on the rows; opening a channel to join it comes with history in M3.
            children.push(bar(
                tr(cx, key::lng_channels_recommended).to_string(),
                None,
                cx,
            ));
            children.extend(
                recommended
                    .into_iter()
                    .map(|chat| self.peer_row(chat, Counter::Hidden, RowMenu::None, cx)),
            );
        }
        children
    }

    // shortcut: TDLib 1.8.69 has no popular Web Apps list, so the tab shows the user's own apps only.
    pub(super) fn apps_tab(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(suggestions) = &self.suggestions else {
            return Vec::new();
        };
        let is_expanded = suggestions.apps_expanded;
        let apps = suggestions.apps.clone().unwrap_or_default();
        if apps.is_empty() {
            return vec![empty_text(
                "apps-none",
                tr(cx, key::lng_search_tab_no_results),
            )];
        }
        self.section(
            Section {
                id: "apps-more",
                title: key::lng_bot_apps_your,
                chats: apps,
                is_expanded,
                toggle: |suggestions| suggestions.apps_expanded = !suggestions.apps_expanded,
                menu: RowMenu::None,
            },
            cx,
        )
    }

    fn section(&mut self, section: Section, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Section {
            id,
            title,
            chats,
            is_expanded,
            toggle,
            menu,
        } = section;
        let link = (chats.len() > COLLAPSED_ROWS).then(|| {
            let text = if is_expanded {
                key::lng_channels_your_less
            } else {
                key::lng_channels_your_more
            };
            bar_link(id, tr(cx, text), cx, move |this, _, _, cx| {
                if let Some(suggestions) = &mut this.suggestions {
                    toggle(suggestions);
                    cx.notify();
                }
            })
        });
        let shown = if is_expanded {
            chats.len()
        } else {
            COLLAPSED_ROWS
        };
        let mut children = vec![bar(tr(cx, title).to_string(), link, cx)];
        children.extend(
            chats
                .into_iter()
                .take(shown)
                .map(|chat| self.peer_row(chat, Counter::Hidden, menu, cx)),
        );
        children
    }
}
