//! What a focused search shows before a query (`Dialogs::Suggestions`):
//! tabs of chats, channels and apps.

mod channels;
mod chats;
mod row;

use gpui::{
    AnyElement, App, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Task, Window, div,
};
use i18n::key;
use model::search::TopChats;
use model::{ChatId, RequestError};
use style::{ColorId, ColorRef};
use ui::color;

use super::Dialogs;
use super::menu::send;
use super::tabs::{Tab, tab_strip};
use crate::lang::tr;
use crate::store::spawn;

/// TDLib returns at most this many top chats.
const TOP_LIMIT: i32 = 30;
/// tdesktop keeps this many recent search picks (`RecentPeers` `kLimit`).
const RECENT_LIMIT: i32 = 48;

/// The suggestions' tabs (`Suggestions::Tab`).
// shortcut: no posts, media or downloads tabs; public posts come in M13, global media search with shared media in M6.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum SuggestionsTab {
    #[default]
    Chats,
    Channels,
    Apps,
}

const TABS: [SuggestionsTab; 3] = [
    SuggestionsTab::Chats,
    SuggestionsTab::Channels,
    SuggestionsTab::Apps,
];

#[derive(Debug, Default)]
pub(super) struct Suggestions {
    tab: SuggestionsTab,
    top: Vec<ChatId>,
    recent: Vec<ChatId>,
    /// `None` until the channels tab first opens.
    recommended: Option<Vec<ChatId>>,
    /// `None` until the apps tab first opens.
    apps: Option<Vec<ChatId>>,
    channels_expanded: bool,
    apps_expanded: bool,
}

impl Dialogs {
    /// Shows the suggestions while the search field is focused and empty.
    pub(crate) fn show_suggestions(&mut self, cx: &mut Context<Self>) {
        if self.suggestions.is_some() {
            return;
        }
        self.suggestions = Some(Suggestions::default());
        let session = self.store.read(cx).session().clone();
        self.when_suggested(
            spawn(session.top_chats(TopChats::Users, TOP_LIMIT), cx),
            |suggestions, top| suggestions.top = top,
            cx,
        );
        self.when_suggested(
            spawn(session.recently_found_chats(RECENT_LIMIT), cx),
            |suggestions, recent| suggestions.recent = recent,
            cx,
        );
        cx.notify();
    }

    pub(crate) fn hide_suggestions(&mut self, cx: &mut Context<Self>) {
        if self.suggestions.take().is_some() {
            cx.notify();
        }
    }

    /// Opens `tab`, loading its lists the first time.
    fn open_suggestions_tab(&mut self, tab: SuggestionsTab, cx: &mut Context<Self>) {
        let Some(suggestions) = &mut self.suggestions else {
            return;
        };
        suggestions.tab = tab;
        let session = self.store.read(cx).session().clone();
        match tab {
            SuggestionsTab::Channels if suggestions.recommended.is_none() => {
                suggestions.recommended = Some(Vec::new());
                self.when_suggested(
                    spawn(session.recommended_channels(), cx),
                    |suggestions, found| suggestions.recommended = Some(found),
                    cx,
                );
            }
            SuggestionsTab::Apps if suggestions.apps.is_none() => {
                suggestions.apps = Some(Vec::new());
                self.when_suggested(
                    spawn(session.top_chats(TopChats::WebApps, TOP_LIMIT), cx),
                    |suggestions, found| suggestions.apps = Some(found),
                    cx,
                );
            }
            SuggestionsTab::Chats | SuggestionsTab::Channels | SuggestionsTab::Apps => {}
        }
        cx.notify();
    }

    fn when_suggested(
        &self,
        task: Task<Result<Vec<ChatId>, RequestError>>,
        apply: fn(&mut Suggestions, Vec<ChatId>),
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| match result {
                Ok(chats) => {
                    if let Some(suggestions) = &mut this.suggestions {
                        apply(suggestions, chats);
                        cx.notify();
                    }
                }
                Err(error) => tracing::warn!(%error, "loading search suggestions failed"),
            })
            .ok();
        })
        .detach();
    }

    /// Puts `chat` first in the recent searches (`RecentPeers::bump`).
    pub(super) fn remember_found(&self, chat: ChatId, cx: &mut App) {
        let task = self.store.read(cx).session().add_recently_found_chat(chat);
        send(task, "adding a recent search", cx);
    }

    fn choose_suggested(&mut self, chat: ChatId, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_found(chat, cx);
        self.select(chat, cx);
        // Leaving the field hides the suggestions, as opening a chat does.
        self.main_focus.focus(window, cx);
    }

    pub(super) fn suggestions_view(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let suggestions = self.suggestions.as_ref()?;
        let active = suggestions.tab;
        let (top, recent) = (suggestions.top.clone(), suggestions.recent.clone());
        let tabs: Vec<Tab> = TABS
            .iter()
            .map(|&tab| Tab {
                label: tr(
                    cx,
                    match tab {
                        SuggestionsTab::Chats => key::lng_recent_chats,
                        SuggestionsTab::Channels => key::lng_recent_channels,
                        SuggestionsTab::Apps => key::lng_recent_apps,
                    },
                ),
                counter: None,
                is_active: tab == active,
                is_movable: false,
            })
            .collect();
        let strip = tab_strip("suggestions-tab", tabs, None, cx, |this, index, cx| {
            if let Some(tab) = TABS.get(index) {
                this.open_suggestions_tab(*tab, cx);
            }
        });
        let content = match active {
            SuggestionsTab::Chats => self.chats_tab(top, recent, cx),
            SuggestionsTab::Channels => self.channels_tab(cx),
            SuggestionsTab::Apps => self.apps_tab(cx),
        };
        Some(
            div()
                .size_full()
                .flex()
                .flex_col()
                .bg(color(cx, ColorRef::Palette(ColorId::DialogsBg)))
                // Clicks here keep the search focused, so the suggestions stay.
                .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                .on_mouse_down(MouseButton::Right, |_, window, _| window.prevent_default())
                .child(strip)
                .child(
                    div()
                        .id(("suggestions", active as usize))
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .children(content),
                )
                .into_any_element(),
        )
    }
}
