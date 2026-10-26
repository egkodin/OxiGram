//! The chat list's search results (`Dialogs::InnerWidget` while filtering):
//! known chats, then global results and found messages, each under a bar.

use std::time::Duration;

use gpui::{
    AnyElement, Context, IntoElement, ListAlignment, ListState, ParentElement, Styled, Task, div,
    list, px,
};
use i18n::key;
use model::message::Message;
use model::search::FoundMessages;
use model::{ChatId, RequestError};
use style::dialogs::{SEARCHED_BAR_FONT, SEARCHED_BAR_HEIGHT, SEARCHED_BAR_POSITION};
use style::{ColorId, ColorRef};
use ui::{color, font, font_height};

use super::{Dialogs, PRELOAD_ROWS, RowKey};
use crate::lang::{tr, tr_plural};
use crate::px_of;
use crate::store::spawn;

/// Server searches wait for typing to pause (`AutoSearchTimeout`).
pub(super) const SERVER_DELAY: Duration = Duration::from_millis(900);
pub(super) const CHATS_LIMIT: i32 = 50;
pub(super) const MESSAGES_PAGE: i32 = 50;
/// Rows drawn beyond the view so short scrolls show no gap.
const OVERDRAW: f32 = 200.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Entry {
    Chat(ChatId),
    GlobalBar,
    Global(ChatId),
    MessagesBar,
    /// An index into [`Search::messages`].
    Message(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Messages {
    Waiting,
    Requested,
    HasMore,
    Complete,
}

pub(super) struct Search {
    pub query: String,
    chats: Vec<ChatId>,
    global: Vec<ChatId>,
    pub messages: Vec<Message>,
    pub total_messages: i32,
    pub next_offset: String,
    pub messages_state: Messages,
    pub entries: Vec<Entry>,
    pub list: ListState,
}

impl Search {
    pub fn new(query: String) -> Self {
        Self {
            query,
            chats: Vec::new(),
            global: Vec::new(),
            messages: Vec::new(),
            total_messages: 0,
            next_offset: String::new(),
            messages_state: Messages::Waiting,
            entries: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, px(OVERDRAW)),
        }
    }

    /// Known chats, local ones first; repeats are dropped.
    pub fn add_chats(&mut self, found: Vec<ChatId>) {
        for chat in found {
            if !self.chats.contains(&chat) {
                self.chats.push(chat);
            }
        }
        self.global.retain(|chat| !self.chats.contains(chat));
        self.rebuild();
    }

    pub fn set_global(&mut self, found: Vec<ChatId>) {
        self.global = found
            .into_iter()
            .filter(|chat| !self.chats.contains(chat))
            .collect();
        self.rebuild();
    }

    pub fn add_messages(&mut self, page: FoundMessages) {
        self.total_messages = page.total_count;
        self.messages.extend(page.messages);
        match page.next_offset {
            Some(offset) => {
                self.next_offset = offset;
                self.messages_state = Messages::HasMore;
            }
            None => self.messages_state = Messages::Complete,
        }
        self.rebuild();
    }

    /// Recomputes the entries and tells the list which ones changed, so the
    /// scroll position survives results arriving.
    fn rebuild(&mut self) {
        let entries = entries(
            &self.chats,
            &self.global,
            self.messages.len(),
            self.messages_state == Messages::Complete,
        );
        let same = self
            .entries
            .iter()
            .zip(&entries)
            .take_while(|(was, now)| was == now)
            .count();
        self.list
            .splice(same..self.entries.len(), entries.len() - same);
        self.entries = entries;
    }
}

impl Dialogs {
    /// Shows the results for `query`, or the chat list again when it is
    /// blank. Known chats come at once, server results after a pause.
    pub(crate) fn set_query(&mut self, query: &str, cx: &mut Context<Self>) {
        let query = query.trim();
        if query.is_empty() {
            self.search = None;
            cx.notify();
            return;
        }
        if self
            .search
            .as_ref()
            .is_some_and(|search| search.query == query)
        {
            return;
        }
        let search = Search::new(query.to_owned());
        let weak = cx.entity().downgrade();
        search.list.set_scroll_handler(move |event, _, cx| {
            if event.visible_range.end + PRELOAD_ROWS >= event.count {
                weak.update(cx, |this, cx| this.load_messages(cx)).ok();
            }
        });
        self.search = Some(search);
        self.hovered = None;
        let local = spawn(
            self.store
                .read(cx)
                .session()
                .search_chats(query, CHATS_LIMIT),
            cx,
        );
        self.when_found(local, Search::add_chats, cx);
        let query = query.to_owned();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SERVER_DELAY).await;
            this.update(cx, |this, cx| this.search_server(&query, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }

    fn search_server(&mut self, query: &str, cx: &mut Context<Self>) {
        if self
            .search
            .as_ref()
            .is_none_or(|search| search.query != query)
        {
            return;
        }
        let session = self.store.read(cx).session().clone();
        let mine = spawn(session.search_chats_on_server(query, CHATS_LIMIT), cx);
        self.when_found(mine, Search::add_chats, cx);
        let global = spawn(session.search_public_chats(query), cx);
        self.when_found(global, Search::set_global, cx);
        self.load_messages(cx);
    }

    fn load_messages(&mut self, cx: &mut Context<Self>) {
        let Some(search) = &mut self.search else {
            return;
        };
        if search.messages_state == Messages::Requested
            || search.messages_state == Messages::Complete
        {
            return;
        }
        search.messages_state = Messages::Requested;
        let task = spawn(
            self.store.read(cx).session().search_messages(
                &search.query,
                &search.next_offset,
                MESSAGES_PAGE,
            ),
            cx,
        );
        self.when_found(task, Search::add_messages, cx);
    }

    /// Hands `task`'s result to the search it was made for, unless the
    /// query changed meanwhile.
    fn when_found<T: 'static>(
        &self,
        task: Task<Result<T, RequestError>>,
        apply: fn(&mut Search, T),
        cx: &mut Context<Self>,
    ) {
        let Some(query) = self.search.as_ref().map(|search| search.query.clone()) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let Some(search) = this.search.as_mut().filter(|search| search.query == query)
                else {
                    return;
                };
                match result {
                    Ok(found) => apply(search, found),
                    Err(error) => {
                        tracing::warn!(%error, "chat list search failed");
                        if search.messages_state == Messages::Requested {
                            search.messages_state = Messages::HasMore;
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn search_list(&self, search: &Search, cx: &mut Context<Self>) -> AnyElement {
        let weak = cx.entity().downgrade();
        list(search.list.clone(), move |index, _, cx| {
            weak.update(cx, |this, cx| this.entry_element(index, cx))
                .unwrap_or_else(|_| div().into_any_element())
        })
        .size_full()
        .into_any_element()
    }

    fn entry_element(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(entry) = self
            .search
            .as_ref()
            .and_then(|search| search.entries.get(index).copied())
        else {
            return div().into_any_element();
        };
        match entry {
            Entry::Chat(chat) => self.row_element(RowKey::Chat(chat), chat, None, cx),
            Entry::Global(chat) => self.row_element(RowKey::Global(chat), chat, None, cx),
            Entry::Message(found) => {
                let chat = self
                    .search
                    .as_ref()
                    .and_then(|search| search.messages.get(found))
                    .map(|message| message.chat);
                match chat {
                    Some(chat) => self.row_element(RowKey::Found(found), chat, Some(found), cx),
                    None => div().into_any_element(),
                }
            }
            Entry::GlobalBar => bar(tr(cx, key::lng_search_global_results).to_string(), cx),
            Entry::MessagesBar => {
                let total = self
                    .search
                    .as_ref()
                    .map_or(0, |search| search.total_messages);
                let text = if total > 0 {
                    tr_plural(cx, key::lng_search_found_results, i64::from(total), &[])
                } else {
                    tr(cx, key::lng_search_no_results).to_string()
                };
                bar(text, cx)
            }
        }
    }
}

/// A section title (`searchedBar`).
fn bar(text: String, cx: &gpui::App) -> AnyElement {
    div()
        .h(px_of(SEARCHED_BAR_HEIGHT))
        .pl(px_of(SEARCHED_BAR_POSITION.x))
        .pt(px_of(SEARCHED_BAR_POSITION.y))
        .bg(color(cx, ColorRef::Palette(ColorId::SearchedBarBg)))
        .text_color(color(cx, ColorRef::Palette(ColorId::SearchedBarFg)))
        .font(font(SEARCHED_BAR_FONT))
        .text_size(px_of(SEARCHED_BAR_FONT.size))
        .line_height(font_height(SEARCHED_BAR_FONT))
        .child(text)
        .into_any_element()
}

/// The rows in order: the messages bar shows once there are messages or
/// the search for them found none.
fn entries(
    chats: &[ChatId],
    global: &[ChatId],
    messages: usize,
    messages_complete: bool,
) -> Vec<Entry> {
    let mut entries: Vec<Entry> = chats.iter().copied().map(Entry::Chat).collect();
    if !global.is_empty() {
        entries.push(Entry::GlobalBar);
        entries.extend(global.iter().copied().map(Entry::Global));
    }
    if messages > 0 || messages_complete {
        entries.push(Entry::MessagesBar);
        entries.extend((0..messages).map(Entry::Message));
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puts_bars_before_global_results_and_messages() {
        let chats = [ChatId(1)];
        let global = [ChatId(2)];
        assert_eq!(
            entries(&chats, &global, 1, false),
            [
                Entry::Chat(ChatId(1)),
                Entry::GlobalBar,
                Entry::Global(ChatId(2)),
                Entry::MessagesBar,
                Entry::Message(0),
            ]
        );
        assert_eq!(entries(&chats, &[], 0, false), [Entry::Chat(ChatId(1))]);
        assert_eq!(entries(&[], &[], 0, true), [Entry::MessagesBar]);
    }

    #[test]
    fn known_chats_leave_the_global_results() {
        let mut search = Search::new("a".to_owned());
        search.set_global(vec![ChatId(1), ChatId(2)]);
        search.add_chats(vec![ChatId(2), ChatId(2)]);
        assert_eq!(
            search.entries,
            [
                Entry::Chat(ChatId(2)),
                Entry::GlobalBar,
                Entry::Global(ChatId(1)),
            ]
        );
        assert_eq!(search.list.item_count(), 3);
    }
}
