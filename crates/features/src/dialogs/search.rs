//! The chat list's search results (`Dialogs::InnerWidget` while filtering):
//! known chats, then global results and found messages, each under a bar.

use std::time::Duration;

use gpui::{
    AnyElement, App, ClickEvent, Context, InteractiveElement, IntoElement, ListAlignment,
    ListState, ParentElement, StatefulInteractiveElement, Styled, Task, Window, div, list, px,
};
use i18n::key;
use model::message::Message;
use model::search::{ArchiveScope, ChatTypeFilter, FoundMessages, MessageFilter};
use model::{ChatId, RequestError};
use style::dialogs::{
    DIALOGS_SEARCH_IN_CHECK, SEARCHED_BAR_FONT, SEARCHED_BAR_HEIGHT, SEARCHED_BAR_POSITION,
};
use style::{ColorId, ColorRef};
use ui::PopupMenu;
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
    /// The message filter the found messages passed.
    pub filter: MessageFilter,
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
    pub fn new(query: String, filter: MessageFilter) -> Self {
        Self {
            query,
            filter,
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

    /// Drops the found messages to search again with `filter`.
    pub fn refilter(&mut self, filter: MessageFilter) {
        self.filter = filter;
        self.messages.clear();
        self.total_messages = 0;
        self.next_offset.clear();
        self.messages_state = Messages::Waiting;
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
        let search = Search::new(query.to_owned(), self.message_filter);
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
                search.filter,
                &search.next_offset,
                MESSAGES_PAGE,
            ),
            cx,
        );
        self.when_found(task, Search::add_messages, cx);
    }

    /// Searches messages again once the user picks another filter.
    fn set_message_filter(&mut self, filter: MessageFilter, cx: &mut Context<Self>) {
        if self.message_filter == filter {
            return;
        }
        self.message_filter = filter;
        if let Some(search) = &mut self.search {
            search.refilter(filter);
            self.load_messages(cx);
        }
        cx.notify();
    }

    fn show_filter_menu(
        &mut self,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self.message_filter;
        let weak = cx.entity().downgrade();
        let choose = move |filter: MessageFilter| {
            let weak = weak.clone();
            move |_: &mut Window, cx: &mut App| {
                weak.update(cx, |this, cx| this.set_message_filter(filter, cx))
                    .ok();
            }
        };
        self.open_menu(
            event.position(),
            window,
            cx,
            |_, menu: &mut PopupMenu, cx| {
                let mark = |is_checked: bool| is_checked.then_some(DIALOGS_SEARCH_IN_CHECK);
                let from_archive = current.archive == ArchiveScope::Included;
                menu.option(
                    tr(cx, key::lng_search_filter_from_archive),
                    mark(from_archive),
                    choose(MessageFilter {
                        archive: if from_archive {
                            ArchiveScope::Excluded
                        } else {
                            ArchiveScope::Included
                        },
                        ..current
                    }),
                );
                for chat_type in [
                    ChatTypeFilter::All,
                    ChatTypeFilter::Private,
                    ChatTypeFilter::Groups,
                    ChatTypeFilter::Channels,
                ] {
                    menu.option(
                        tr(cx, chat_type_label(chat_type)),
                        mark(chat_type == current.chat_type),
                        choose(MessageFilter {
                            chat_type,
                            ..current
                        }),
                    );
                }
            },
        );
    }

    /// Hands `task`'s result to the search it was made for, unless the
    /// query or the filter changed meanwhile.
    fn when_found<T: 'static>(
        &self,
        task: Task<Result<T, RequestError>>,
        apply: fn(&mut Search, T),
        cx: &mut Context<Self>,
    ) {
        let Some((query, filter)) = self
            .search
            .as_ref()
            .map(|search| (search.query.clone(), search.filter))
        else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let Some(search) = this
                    .search
                    .as_mut()
                    .filter(|search| search.query == query && search.filter == filter)
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
            Entry::GlobalBar => bar(tr(cx, key::lng_search_global_results).to_string(), None, cx),
            Entry::MessagesBar => {
                let (total, found) = self.search.as_ref().map_or((0, 0), |search| {
                    (search.total_messages, search.messages.len())
                });
                let text = if total > 0 {
                    tr_plural(cx, key::lng_search_found_results, i64::from(total), &[])
                } else {
                    tr(cx, key::lng_search_no_results).to_string()
                };
                // tdesktop hides the filter with no results; a narrowed
                // search keeps it so the filter can be undone.
                let filter = (found > 0 || self.message_filter != MessageFilter::default())
                    .then(|| self.filter_link(cx));
                bar(text, filter, cx)
            }
        }
    }
}

impl Dialogs {
    /// The current filter at the bar's right, opening the filter menu.
    fn filter_link(&self, cx: &mut Context<Self>) -> AnyElement {
        let label = match self.message_filter {
            MessageFilter {
                chat_type: ChatTypeFilter::All,
                archive: ArchiveScope::Excluded,
            } => key::lng_search_filter_non_archived,
            filter => chat_type_label(filter.chat_type),
        };
        div()
            .id("search-filter")
            .cursor_pointer()
            .hover(|style| style.underline())
            .child(tr(cx, label))
            .on_click(cx.listener(|this, event, window, cx| {
                this.show_filter_menu(event, window, cx);
            }))
            .into_any_element()
    }
}

fn chat_type_label(filter: ChatTypeFilter) -> i18n::Key {
    match filter {
        ChatTypeFilter::All => key::lng_search_filter_all,
        ChatTypeFilter::Private => key::lng_search_filter_private,
        ChatTypeFilter::Groups => key::lng_search_filter_group,
        ChatTypeFilter::Channels => key::lng_search_filter_channel,
    }
}

/// A section title (`searchedBar`), with `filter` at its right.
fn bar(text: String, filter: Option<AnyElement>, cx: &App) -> AnyElement {
    div()
        .h(px_of(SEARCHED_BAR_HEIGHT))
        .flex()
        .items_start()
        .justify_between()
        .pl(px_of(SEARCHED_BAR_POSITION.x))
        .pr(px_of(SEARCHED_BAR_POSITION.x))
        .pt(px_of(SEARCHED_BAR_POSITION.y))
        .bg(color(cx, ColorRef::Palette(ColorId::SearchedBarBg)))
        .text_color(color(cx, ColorRef::Palette(ColorId::SearchedBarFg)))
        .font(font(SEARCHED_BAR_FONT))
        .text_size(px_of(SEARCHED_BAR_FONT.size))
        .line_height(font_height(SEARCHED_BAR_FONT))
        .child(text)
        .children(filter)
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
        let mut search = Search::new("a".to_owned(), MessageFilter::default());
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

    #[test]
    fn refiltering_drops_found_messages_and_keeps_chats() {
        let mut search = Search::new("a".to_owned(), MessageFilter::default());
        search.add_chats(vec![ChatId(1)]);
        search.add_messages(FoundMessages {
            total_count: 0,
            messages: Vec::new(),
            next_offset: None,
        });
        assert_eq!(search.entries, [Entry::Chat(ChatId(1)), Entry::MessagesBar]);
        let channels = MessageFilter {
            chat_type: ChatTypeFilter::Channels,
            archive: ArchiveScope::Included,
        };
        search.refilter(channels);
        assert_eq!(search.filter, channels);
        assert_eq!(search.messages_state, Messages::Waiting);
        assert_eq!(search.entries, [Entry::Chat(ChatId(1))]);
        assert_eq!(search.list.item_count(), 1);
    }
}
