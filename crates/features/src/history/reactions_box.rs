//! Who reacted to a message, a tab per reaction (`Info::ReactionsList`).
// shortcut: a layer box, as tdesktop shows the section in a narrow window; moves to the info column with the profile panel (M6).

use std::rc::Rc;

use chrono::Local;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, WeakEntity, Window, div, prelude::*,
    px, uniform_list,
};
use i18n::key;
use model::message::Sender;
use model::reaction::{AddedReaction, ReactionKind, Reactions};
use model::{ChatId, FileId, MessageId};
use style::basic::{EMOJI_SIZE, SEMIBOLD_FONT};
use style::boxes::CONTACTS_STATUS_FONT;
use style::chat::{
    REACTIONS_TAB_ALL, REACTIONS_TAB_ALL_SELECTED, REACTIONS_TAB_ICON_SKIP, REACTIONS_TABS,
};
use style::layers::{BOX_MAX_LIST_HEIGHT, DEFAULT_BOX};
use style::widgets::DEFAULT_PEER_LIST;
use ui::{BoxFrame, RoundButton, Userpic, color, font, font_height};

use super::reactions::{emoji, picture};
use super::sender_face;
use crate::Store;
use crate::lang::{group_digits, strings, tr};
use crate::layers::{Dismiss, Layers};
use crate::px_of;
use crate::status::status;
use crate::store::spawn;

/// Who reacted, per request (`kPerPageFirst`).
const PAGE: i32 = 50;
/// Rows left below the shown ones when the next page is asked for.
const PRELOAD_ROWS: usize = 10;
/// Room the box leaves around itself in a short window.
const WINDOW_MARGIN: f32 = 160.0;
/// Userpics wait behind what the user opened.
const USERPIC_PRIORITY: i32 = 8;

/// A tab: all reactions, or one of them, with how many there are.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Tab {
    kind: Option<ReactionKind>,
    count: i32,
}

pub(super) struct ReactionsBox {
    store: Entity<Store>,
    layers: WeakEntity<Layers>,
    focus: FocusHandle,
    chat: ChatId,
    message: MessageId,
    tabs: Vec<Tab>,
    selected: usize,
    rows: Vec<AddedReaction>,
    /// Where the next page starts; none once the last one came.
    next: Option<String>,
    is_loading: bool,
    /// Drops pages asked for before the tab changed.
    generation: u64,
    _store_changed: Subscription,
}

impl Focusable for ReactionsBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Opens the list on the "All" tab and asks for its first page.
pub(super) fn show(
    store: &Entity<Store>,
    layers: &Entity<Layers>,
    chat: ChatId,
    message: MessageId,
    reactions: &Reactions,
    window: &mut Window,
    cx: &mut App,
) {
    let weak = layers.downgrade();
    let view = cx.new(|cx| {
        // Names and userpics arrive as user updates.
        let store_changed = cx.observe(store, |_, _, cx| cx.notify());
        let mut view = ReactionsBox {
            store: store.clone(),
            layers: weak,
            focus: cx.focus_handle(),
            chat,
            message,
            tabs: tabs(reactions),
            selected: 0,
            rows: Vec::new(),
            next: Some(String::new()),
            is_loading: false,
            generation: 0,
            _store_changed: store_changed,
        };
        view.load(cx);
        view
    });
    layers.update(cx, |layers, cx| {
        layers.show(view, Dismiss::Allowed, None, window, cx)
    });
}

/// "All" with every reaction, then each reaction, the most used first.
// shortcut: no tab of who only saw the message (`WhoReadList`); with the info column (M6).
fn tabs(reactions: &Reactions) -> Vec<Tab> {
    // TDLib cannot list paid reactions by their own type.
    let mut each: Vec<Tab> = reactions
        .list
        .iter()
        .filter(|reaction| reaction.kind != ReactionKind::Paid)
        .map(|reaction| Tab {
            kind: Some(reaction.kind.clone()),
            count: reaction.count,
        })
        .collect();
    each.sort_by_key(|tab| std::cmp::Reverse(tab.count));
    let all = Tab {
        kind: None,
        count: each.iter().map(|tab| tab.count).sum(),
    };
    std::iter::once(all).chain(each).collect()
}

impl ReactionsBox {
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if index == self.selected {
            return;
        }
        self.selected = index;
        self.rows.clear();
        self.next = Some(String::new());
        self.is_loading = false;
        self.generation += 1;
        self.load(cx);
        cx.notify();
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let Some(offset) = self.next.clone().filter(|_| !self.is_loading) else {
            return;
        };
        self.is_loading = true;
        let kind = self
            .tabs
            .get(self.selected)
            .and_then(|tab| tab.kind.clone());
        let task = spawn(
            self.store.read(cx).session().added_reactions(
                self.chat,
                self.message,
                kind.as_ref(),
                offset,
                PAGE,
            ),
            cx,
        );
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.is_loading = false;
                match result {
                    Ok(page) => {
                        this.rows.extend(page.list);
                        this.next = Some(page.next_offset).filter(|next| !next.is_empty());
                    }
                    Err(error) => {
                        tracing::warn!(%error, "loading who reacted failed");
                        this.next = None;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn download(&mut self, file: FileId, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.load_userpic(file, USERPIC_PRIORITY, cx)
        });
    }

    fn tab(&self, index: usize, tab: &Tab, cx: &mut Context<Self>) -> AnyElement {
        let st = &REACTIONS_TABS.item;
        let is_selected = index == self.selected;
        let (bg, fg) = if is_selected {
            (st.text_active_bg, st.text_active_fg)
        } else {
            (st.text_bg, st.text_fg)
        };
        let icon = match &tab.kind {
            Some(kind) => emoji(kind, picture(&self.store, kind, cx).as_deref()),
            None if is_selected => div().child(ui::icon(REACTIONS_TAB_ALL_SELECTED)),
            None => div().child(ui::icon(REACTIONS_TAB_ALL)),
        };
        div()
            .id(("reactions-tab", index))
            .flex()
            .items_center()
            .h(px_of(st.height))
            .rounded_full()
            .bg(color(cx, bg))
            .pl(px_of(REACTIONS_TAB_ICON_SKIP))
            .pr(px_of(st.padding.right))
            .cursor_pointer()
            .child(
                div()
                    .size(px_of(st.height))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon),
            )
            .child(
                div()
                    .ml(px_of(st.padding.left) - px_of(REACTIONS_TAB_ICON_SKIP))
                    .font(font(SEMIBOLD_FONT))
                    .text_size(px_of(SEMIBOLD_FONT.size))
                    .line_height(font_height(SEMIBOLD_FONT))
                    .text_color(color(cx, fg))
                    .child(group_digits(i64::from(tab.count))),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.select(index, cx)))
            .into_any_element()
    }

    // shortcut: rows open nothing; tdesktop opens the person's profile, which comes with M6.
    fn row(&mut self, added: &AddedReaction, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let st = &DEFAULT_PEER_LIST.item;
        let shown = picture(&self.store, &added.kind, cx);
        let state = self.store.read(cx).state();
        let face = sender_face(&state.chats, added.sender);
        let line = match added.sender {
            Sender::User(user) => state
                .chats
                .user(user)
                .map(|user| status(strings(cx), user, &Local::now())),
            Sender::Chat(_) => None,
        };
        let (name, source) = match face {
            Some((name, _, (source, file))) => {
                if let Some(file) = file {
                    self.download(file, cx);
                }
                (name, Some(source))
            }
            None => (String::new(), None),
        };
        let status_fg = match &line {
            Some(line) if line.is_online => st.status_fg_active,
            _ => st.status_fg,
        };
        let name_font = st.name_style.font;
        let status_font = CONTACTS_STATUS_FONT;
        let emoji_top = (px_of(st.height) - px_of(EMOJI_SIZE)) / 2.0;
        div()
            .id(("reactor", index))
            .relative()
            .h(px_of(st.height))
            .w_full()
            .bg(color(cx, DEFAULT_PEER_LIST.bg))
            .when_some(source, |this, source| {
                this.child(
                    div()
                        .absolute()
                        .left(px_of(st.photo_position.x))
                        .top(px_of(st.photo_position.y))
                        .child(Userpic::new(px_of(st.photo_size), source)),
                )
            })
            .child(
                div()
                    .absolute()
                    .left(px_of(st.name_position.x))
                    .top(px_of(st.name_position.y))
                    .right(px_of(EMOJI_SIZE) * 3.0)
                    .font(font(name_font))
                    .text_size(px_of(name_font.size))
                    .line_height(font_height(name_font))
                    .text_color(color(cx, st.name_fg))
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(SharedString::from(name)),
            )
            .when_some(line, |this, line| {
                this.child(
                    div()
                        .absolute()
                        .left(px_of(st.status_position.x))
                        .top(px_of(st.status_position.y))
                        .font(font(status_font))
                        .text_size(px_of(status_font.size))
                        .line_height(font_height(status_font))
                        .text_color(color(cx, status_fg))
                        .whitespace_nowrap()
                        .child(SharedString::from(line.text)),
                )
            })
            .child(
                div()
                    .absolute()
                    .top(emoji_top)
                    .right(px_of(EMOJI_SIZE) * 1.5)
                    .child(emoji(&added.kind, shown.as_deref())),
            )
            .into_any_element()
    }
}

impl Render for ReactionsBox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let st = &REACTIONS_TABS;
        let tabs: Vec<_> = self
            .tabs
            .clone()
            .iter()
            .enumerate()
            .map(|(index, tab)| self.tab(index, tab, cx))
            .collect();
        let tabs = div()
            .flex()
            .flex_wrap()
            .gap(px_of(st.item_skip))
            .pl(px_of(st.padding.left))
            .pr(px_of(st.padding.right))
            .pt(px_of(st.padding.top))
            .pb(px_of(st.padding.bottom))
            .children(tabs);
        let item_height = px_of(DEFAULT_PEER_LIST.item.height);
        let expected = self
            .tabs
            .get(self.selected)
            .map_or(0, |tab| usize::try_from(tab.count).unwrap_or(0))
            .max(self.rows.len());
        let window_room = window.viewport_size().height - px(WINDOW_MARGIN);
        let list_height = (item_height * expected as f32)
            .min(px_of(BOX_MAX_LIST_HEIGHT))
            .min(window_room)
            .max(item_height);
        let rows: Rc<Vec<AddedReaction>> = Rc::new(self.rows.clone());
        let list = uniform_list(
            "reactors",
            rows.len(),
            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                if range.end + PRELOAD_ROWS >= rows.len() {
                    this.load(cx);
                }
                range
                    .map(|index| this.row(&rows[index], index, cx))
                    .collect::<Vec<_>>()
            }),
        )
        .h(list_height);
        let entity = cx.entity();
        BoxFrame::new("reactions-box", &DEFAULT_BOX)
            .title(tr(cx, key::lng_manage_peer_reactions))
            .row(tabs)
            .row(list)
            .button(
                RoundButton::new(
                    "reactions-close",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_close),
                )
                .on_click(move |_, window, cx| {
                    entity.update(cx, |this, cx| this.close(window, cx));
                }),
            )
    }
}

#[cfg(test)]
mod tests {
    use model::reaction::Reaction;

    use super::*;

    fn reaction(kind: ReactionKind, count: i32) -> Reaction {
        Reaction {
            kind,
            count,
            is_chosen: false,
            recent: Vec::new(),
        }
    }

    #[test]
    fn tabs_start_with_all_then_the_most_used_reaction() {
        let heart = ReactionKind::Emoji("❤".to_owned());
        let fire = ReactionKind::Emoji("🔥".to_owned());
        let reactions = Reactions {
            list: vec![
                reaction(ReactionKind::Paid, 9),
                reaction(heart.clone(), 3),
                reaction(fire.clone(), 48),
            ],
            ..Reactions::default()
        };
        let tabs = tabs(&reactions);
        let shown: Vec<_> = tabs
            .iter()
            .map(|tab| (tab.kind.clone(), tab.count))
            .collect();
        assert_eq!(shown, [(None, 51), (Some(fire), 48), (Some(heart), 3)]);
    }
}
