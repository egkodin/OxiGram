//! The tags under the search field in Saved Messages
//! (`Dialogs::SearchTags` in `ComposeSearch`): each tag with its name and
//! count; picking one finds the messages that carry it. Without Premium a
//! lock pill leads to what tags are.

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement as _, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
};
use i18n::key;
use model::message::Topic;
use model::reaction::ReactionKind;
use model::saved_tags::SavedTag;
use style::chat::{
    REACTION_INLINE_PADDING, REACTION_INLINE_SIZE, REACTION_INLINE_TAG_FONT,
    REACTION_INLINE_TAG_LEFT_RADIUS, REACTION_INLINE_TAG_NAME_POSITION,
    REACTION_INLINE_TAG_PROMO_POSITION, REACTION_INLINE_TAG_RIGHT_RADIUS, REACTION_INLINE_TAG_SKIP,
    SEARCH_IN_CHAT_TAGS_PADDING,
};
use style::dialogs::{
    DIALOGS_SEARCH_TAG_ARROW, DIALOGS_SEARCH_TAG_BOTTOM, DIALOGS_SEARCH_TAG_LOCKED,
    DIALOGS_SEARCH_TAG_PROMO, DIALOGS_SEARCH_TAG_PROMO_LEFT, DIALOGS_SEARCH_TAG_PROMO_RIGHT,
    DIALOGS_SEARCH_TAG_PROMO_SKIP, DIALOGS_SEARCH_TAG_SKIP,
};
use style::{ColorId, ColorRef};
use ui::{color, font, font_height};

use super::HistoryView;
use super::reactions::{emoji, picture};
use crate::lang::tr;
use crate::px_of;
use crate::store::spawn;

/// The tags of the open search, and the one it searches by.
#[derive(Debug, Default)]
pub(super) struct Tags {
    list: Vec<SavedTag>,
    pub(super) selected: Option<ReactionKind>,
}

/// How a pill looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Look {
    Normal,
    Selected,
    /// "Add tags" or "Unlock" with a lock, for a user without Premium.
    Promo,
}

impl Look {
    /// How wide the emoji or lock is, where the text starts in the pill,
    /// and how much narrower than the tag background the pill is around
    /// the text (`SearchTags::layout`).
    fn place(self) -> (i32, i32, i32) {
        let size = REACTION_INLINE_SIZE.0;
        match self {
            Self::Promo => (
                size - DIALOGS_SEARCH_TAG_PROMO_LEFT.0,
                REACTION_INLINE_TAG_PROMO_POSITION.x.0,
                DIALOGS_SEARCH_TAG_PROMO_LEFT.0 + DIALOGS_SEARCH_TAG_PROMO_RIGHT.0,
            ),
            Self::Normal | Self::Selected => (size, REACTION_INLINE_TAG_NAME_POSITION.x.0, 0),
        }
    }

    /// The pill's fill and its text (`SearchTags::bgColor`, `paintText`).
    fn colors(self) -> (ColorId, ColorId) {
        match self {
            Self::Normal => (ColorId::DialogsBgOver, ColorId::WindowSubTextFg),
            Self::Selected => (ColorId::DialogsBgActive, ColorId::DialogsTextFgActive),
            Self::Promo => (ColorId::LightButtonBgOver, ColorId::LightButtonFgOver),
        }
    }
}

impl Tags {
    /// The tags worth a pill: used ones and the one searched by.
    fn shown(&self) -> impl Iterator<Item = &SavedTag> {
        self.list
            .iter()
            .filter(|tag| tag.count > 0 || self.selected.as_ref() == Some(&tag.kind))
    }

    /// Picks `kind`, or drops it when it was picked.
    // shortcut: one tag at a time; TDLib's searchSavedMessages takes one, where tdesktop adds more with Shift.
    fn toggle(&mut self, kind: ReactionKind) {
        self.selected = (self.selected.as_ref() != Some(&kind)).then_some(kind);
    }
}

impl HistoryView {
    pub(super) fn is_saved_messages(&self, cx: &Context<Self>) -> bool {
        let state = self.store.read(cx).state();
        state
            .chats
            .chat(self.chat)
            .is_some_and(|chat| chat.user().is_some() && chat.user() == state.me)
    }

    // shortcut: loaded once per search; `updateSavedMessagesTags` while it is open is not followed.
    pub(super) fn load_search_tags(&mut self, cx: &mut Context<Self>) {
        let topic = match self.topic {
            Some(Topic::Saved(topic)) => Some(topic),
            _ => None,
        };
        let task = spawn(self.store.read(cx).session().saved_messages_tags(topic), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(list) => {
                    if let Some(tags) = this.search.as_mut().and_then(|s| s.tags.as_mut()) {
                        tags.list = list;
                        cx.notify();
                    }
                }
                Err(error) => tracing::warn!(%error, "loading Saved Messages tags failed"),
            });
        })
        .detach();
    }

    fn pick_tag(&mut self, kind: ReactionKind, cx: &mut Context<Self>) {
        let Some(tags) = self.search.as_mut().and_then(|s| s.tags.as_mut()) else {
            return;
        };
        tags.toggle(kind);
        self.search_now(cx);
    }

    /// The pills under the field, wrapping to more rows when they need.
    // shortcut: no right-click menu to rename a tag or show it in the chat; with sending reactions (M4).
    pub(super) fn search_tags(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let kinds: Vec<ReactionKind> = (self.search.as_ref()?.tags.as_ref()?.shown())
            .map(|tag| tag.kind.clone())
            .collect();
        let pictures: Vec<_> = kinds
            .iter()
            .map(|kind| picture(&self.store, kind, cx))
            .collect();
        let tags = self.search.as_ref()?.tags.as_ref()?;
        let state = self.store.read(cx).state();
        let is_premium = state
            .me
            .and_then(|me| state.chats.user(me))
            .is_some_and(|me| me.is_premium);
        let mut pills = Vec::new();
        if !is_premium {
            let text = if tags.shown().next().is_none() {
                key::lng_add_tag_button
            } else {
                key::lng_unlock_tags
            };
            pills.push(self.promo_pill(tr(cx, text), cx));
        }
        for (index, tag) in tags.shown().enumerate() {
            let look = if tags.selected.as_ref() == Some(&tag.kind) {
                Look::Selected
            } else {
                Look::Normal
            };
            let kind = tag.kind.clone();
            let shown = pictures.get(index).cloned().flatten();
            let weak = cx.weak_entity();
            let pill = pill(
                ("history-search-tag", index),
                look,
                emoji(&tag.kind, shown.as_deref()),
                tag.text(),
                cx,
            )
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if is_premium {
                        this.pick_tag(kind.clone(), cx);
                    } else {
                        this.show_tags_promo(window, cx);
                    }
                });
            });
            pills.push(pill.into_any_element());
        }
        let only_promo = !is_premium && pills.len() == 1;
        let skip = DIALOGS_SEARCH_TAG_SKIP;
        let pad = SEARCH_IN_CHAT_TAGS_PADDING;
        Some(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_x(px_of(skip.x))
                .gap_y(px_of(skip.y))
                .pl(px_of(pad.left))
                .pr(px_of(pad.right))
                .pb(px_of(DIALOGS_SEARCH_TAG_BOTTOM))
                .border_b_1()
                .border_color(color(cx, ColorRef::Palette(ColorId::ShadowFg)))
                .children(pills)
                .when(only_promo, |this| this.child(promo_phrase(cx)))
                .into_any_element(),
        )
    }

    fn promo_pill(&self, text: SharedString, cx: &Context<Self>) -> AnyElement {
        let weak = cx.weak_entity();
        let lock = div()
            .flex()
            .justify_center()
            .child(ui::icon(DIALOGS_SEARCH_TAG_LOCKED));
        pill("history-search-tags-promo", Look::Promo, lock, text, cx)
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| this.show_tags_promo(window, cx));
            })
            .into_any_element()
    }

    fn show_tags_promo(&self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let feature = (
            key::lng_premium_summary_subtitle_tags_for_messages,
            key::lng_premium_summary_about_tags_for_messages,
        );
        self.show_premium_feature(feature, window, cx);
    }
}

/// A tag's shape with the emoji or lock and the text after it: round on
/// the left, nearly square where it points.
// shortcut: the right end is squared off without the arrow point and the hole (`PaintTagShape`); with custom paths.
fn pill(
    id: impl Into<gpui::ElementId>,
    look: Look,
    lead: gpui::Div,
    text: impl Into<SharedString>,
    cx: &Context<HistoryView>,
) -> gpui::Stateful<gpui::Div> {
    let (bg, fg) = look.colors();
    let pad = REACTION_INLINE_PADDING;
    let (lead_width, text_x, narrower) = look.place();
    let tag_width = pad.left.0 + REACTION_INLINE_SIZE.0 + REACTION_INLINE_TAG_SKIP.0 + pad.right.0;
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .h(px_of(pad.top) + px_of(REACTION_INLINE_SIZE) + px_of(pad.bottom))
        .pl(px_of(pad.left))
        .pr(gpui::px((tag_width - narrower - text_x) as f32))
        .rounded_l(px_of(REACTION_INLINE_TAG_LEFT_RADIUS))
        .rounded_r(px_of(REACTION_INLINE_TAG_RIGHT_RADIUS))
        .bg(color(cx, ColorRef::Palette(bg)))
        .text_color(color(cx, ColorRef::Palette(fg)))
        .cursor_pointer()
        .child(lead.flex_none().w(gpui::px(lead_width as f32)))
        .child(
            div()
                .flex_none()
                .ml(gpui::px((text_x - pad.left.0 - lead_width) as f32))
                .font(font(REACTION_INLINE_TAG_FONT))
                .text_size(px_of(REACTION_INLINE_TAG_FONT.size))
                .line_height(font_height(REACTION_INLINE_TAG_FONT))
                .child(text.into()),
        )
}

/// "to your Saved Messages ›" after the lone lock pill.
// shortcut: always the long phrase, cut with an ellipsis when narrow; tdesktop falls back to "to messages" and then to none.
fn promo_phrase(cx: &Context<HistoryView>) -> AnyElement {
    let phrase = tr(cx, key::lng_add_tag_phrase_long);
    let before = phrase
        .split("{arrow}")
        .next()
        .unwrap_or_default()
        .to_owned();
    let style = &DIALOGS_SEARCH_TAG_PROMO;
    let arrow = DIALOGS_SEARCH_TAG_ARROW;
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .ml(px_of(DIALOGS_SEARCH_TAG_PROMO_SKIP) - px_of(DIALOGS_SEARCH_TAG_SKIP.x))
        .font(font(style.font))
        .text_size(px_of(style.font.size))
        .line_height(font_height(style.font))
        .text_color(color(cx, ColorRef::Palette(ColorId::WindowSubTextFg)))
        .child(div().min_w_0().truncate().child(before))
        .child(
            div()
                .flex_none()
                .ml(px_of(arrow.padding.left))
                .mt(px_of(arrow.padding.top))
                .child(ui::icon(arrow.icon)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(emoji: &str, count: i32) -> SavedTag {
        SavedTag {
            kind: ReactionKind::Emoji(emoji.to_owned()),
            label: String::new(),
            count,
        }
    }

    #[test]
    fn unused_tags_hide_unless_searched_by() {
        let mut tags = Tags {
            list: vec![tag("👍", 2), tag("🔥", 0)],
            selected: None,
        };
        assert_eq!(tags.shown().count(), 1);
        tags.toggle(ReactionKind::Emoji("🔥".to_owned()));
        assert_eq!(tags.shown().count(), 2, "the picked tag stays visible");
    }

    #[test]
    fn picking_a_tag_again_drops_it_and_another_replaces_it() {
        let mut tags = Tags::default();
        let thumbs = ReactionKind::Emoji("👍".to_owned());
        let fire = ReactionKind::Emoji("🔥".to_owned());
        tags.toggle(thumbs.clone());
        tags.toggle(fire.clone());
        assert_eq!(tags.selected, Some(fire.clone()));
        tags.toggle(fire);
        assert_eq!(tags.selected, None);
    }
}
