//! Reactions under a message's text (`Reactions::InlineList` in a bubble):
//! each emoji with its count, or with who chose it when that is few.

use gpui::{
    AnyElement, App, Context, Div, IntoElement as _, ParentElement as _, Styled as _, div,
    prelude::FluentBuilder as _, px,
};
use model::chat::ChatKind;
use model::message::{Message, Sender};
use model::reaction::{Reaction, ReactionKind, Reactions};
use style::basic::SEMIBOLD_FONT;
use style::chat::{
    MEDIA_IN_BUBBLE_SKIP, MSG_DATE_DELTA, MSG_DATE_SPACE, REACTION_INLINE_BETWEEN,
    REACTION_INLINE_EMPTY_SKIP, REACTION_INLINE_IN_BUBBLE_LEFT, REACTION_INLINE_PADDING,
    REACTION_INLINE_SIZE, REACTION_INLINE_SKIP, REACTION_INLINE_USERPICS,
    REACTION_INLINE_USERPICS_PADDING,
};
use style::{ColorId, ColorRef};
use ui::{UserpicSource, color, font, font_height};

use super::userpics_row::userpics_row;
use super::{HistoryView, layout, sender_face};
use crate::px_of;

/// Unchosen reactions fill their pill this faintly.
const IN_NON_CHOSEN_OPACITY: f32 = 0.12;
const OUT_NON_CHOSEN_OPACITY: f32 = 0.18;
/// Userpics replace the counts up to this many reactions in all
/// (`kMaxRecentUserpics`).
const MAX_RECENT_USERPICS: i32 = 3;
/// The emoji glyph's size inside its square.
const EMOJI_FONT_SIZE: f32 = 15.0;

/// One reaction, ready to draw.
pub(super) struct Button {
    kind: ReactionKind,
    label: Label,
    is_chosen: bool,
}

enum Label {
    /// The count, empty for Saved Messages tags.
    Count(String),
    Userpics(Vec<UserpicSource>),
}

impl HistoryView {
    pub(super) fn reaction_buttons(
        &mut self,
        message: &Message,
        cx: &mut Context<Self>,
    ) -> Vec<Button> {
        let Some(reactions) = message.reactions.as_ref().filter(|r| !r.list.is_empty()) else {
            return Vec::new();
        };
        let state = self.store.read(cx).state();
        let private_user = state
            .chats
            .chat(self.chat)
            .and_then(|chat| match chat.kind {
                ChatKind::Private { user } | ChatKind::Secret { user } => Some(user),
                ChatKind::Group { .. } | ChatKind::Supergroup { .. } => None,
            });
        let me = state.me;
        let has_userpics = private_user.is_some() || shows_userpics(reactions);
        let mut files = Vec::new();
        let buttons = sorted(reactions)
            .into_iter()
            .map(|reaction| {
                let label = if reactions.are_tags {
                    Label::Count(String::new())
                } else if has_userpics {
                    let senders = match (private_user, me) {
                        (Some(user), Some(me)) => private_senders(reaction, user, me),
                        _ => reaction.recent.clone(),
                    };
                    let faces = senders
                        .into_iter()
                        .filter_map(|sender| sender_face(&state.chats, sender))
                        .map(|(_, _, (userpic, file))| {
                            files.extend(file);
                            userpic
                        });
                    Label::Userpics(faces.collect())
                } else {
                    Label::Count(layout::short_count(reaction.count))
                };
                Button {
                    kind: reaction.kind.clone(),
                    label,
                    is_chosen: reaction.is_chosen,
                }
            })
            .collect();
        for file in files {
            self.request_file(file, cx);
        }
        buttons
    }
}

/// Paid first, then the most chosen by others; ties keep TDLib's order.
fn sorted(reactions: &Reactions) -> Vec<&Reaction> {
    let mut sorted: Vec<&Reaction> = reactions.list.iter().collect();
    if !reactions.are_tags {
        sorted.sort_by_key(|reaction| {
            let others = reaction.count - i32::from(reaction.is_chosen);
            (
                reaction.kind != ReactionKind::Paid,
                std::cmp::Reverse(others),
            )
        });
    }
    sorted
}

/// Whether TDLib named everyone who reacted, and they are few.
fn shows_userpics(reactions: &Reactions) -> bool {
    let total: i32 = reactions.list.iter().map(|reaction| reaction.count).sum();
    total <= MAX_RECENT_USERPICS
        && reactions.list.iter().all(|reaction| {
            usize::try_from(reaction.count).is_ok_and(|count| count == reaction.recent.len())
        })
}

/// In a private chat only the two of them can react.
fn private_senders(reaction: &Reaction, user: model::UserId, me: model::UserId) -> Vec<Sender> {
    let mut senders = Vec::with_capacity(2);
    if !reaction.is_chosen || reaction.count > 1 {
        senders.push(Sender::User(user));
    }
    if reaction.is_chosen {
        senders.push(Sender::User(me));
    }
    senders
}

/// The reactions under the text; `room`, an unseen copy of the time,
/// keeps its place on the last row (`skipBlock`).
// shortcut: custom emoji and paid reactions show a placeholder; with stickers and custom emoji (M5) and Stars (M13).
pub(super) fn row(buttons: Vec<Button>, out: bool, is_selected: bool, room: Div, cx: &App) -> Div {
    div()
        .flex()
        .flex_wrap()
        .gap(px_of(REACTION_INLINE_BETWEEN))
        .mt(px_of(MEDIA_IN_BUBBLE_SKIP))
        .ml(px_of(REACTION_INLINE_IN_BUBBLE_LEFT))
        .children(
            buttons
                .into_iter()
                .map(|button| element(button, out, is_selected, cx)),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .h(height())
                .pl(px_of(MSG_DATE_SPACE) - px_of(MSG_DATE_DELTA.x))
                .invisible()
                .child(room),
        )
}

fn height() -> gpui::Pixels {
    px_of(REACTION_INLINE_PADDING.top)
        + px_of(REACTION_INLINE_SIZE)
        + px_of(REACTION_INLINE_PADDING.bottom)
}

fn element(button: Button, out: bool, is_selected: bool, cx: &App) -> AnyElement {
    let palette = |id| color(cx, ColorRef::Palette(id));
    let (bg, fg) = colors(button.is_chosen, out, is_selected);
    let opacity = match (button.is_chosen, out) {
        (true, _) => 1.0,
        (false, true) => OUT_NON_CHOSEN_OPACITY,
        (false, false) => IN_NON_CHOSEN_OPACITY,
    };
    let pad = REACTION_INLINE_PADDING;
    let right = match &button.label {
        Label::Userpics(_) => px_of(REACTION_INLINE_USERPICS_PADDING.right),
        Label::Count(count) if count.is_empty() => {
            px_of(pad.right) - px_of(REACTION_INLINE_EMPTY_SKIP)
        }
        Label::Count(_) => px_of(pad.right),
    };
    let fg = palette(fg);
    let ring = palette(if button.is_chosen {
        bg
    } else {
        bubble_bg(out, is_selected)
    });
    div()
        .flex_none()
        .flex()
        .items_center()
        .h(height())
        .pl(px_of(pad.left))
        .pr(right)
        .rounded_full()
        .bg(palette(bg).opacity(opacity))
        .child(emoji(&button.kind))
        .map(|this| match button.label {
            Label::Count(count) if count.is_empty() => this,
            Label::Count(count) => this.child(
                div()
                    .ml(px_of(REACTION_INLINE_SKIP))
                    .font(font(SEMIBOLD_FONT))
                    .text_size(px_of(SEMIBOLD_FONT.size))
                    .line_height(font_height(SEMIBOLD_FONT))
                    .text_color(fg)
                    .child(count),
            ),
            Label::Userpics(userpics) => this.child(
                div()
                    .ml(px_of(REACTION_INLINE_USERPICS_PADDING.left))
                    .child(userpics_row(userpics, &REACTION_INLINE_USERPICS, ring)),
            ),
        })
        .into_any_element()
}

/// The pill's fill and the text over it.
fn colors(is_chosen: bool, out: bool, is_selected: bool) -> (ColorId, ColorId) {
    match (is_chosen, out, is_selected) {
        (false, false, false) => (ColorId::MsgFileInBg, ColorId::MsgInServiceFg),
        (false, false, true) => (
            ColorId::MsgFileInBgSelected,
            ColorId::MsgInServiceFgSelected,
        ),
        (false, true, false) => (ColorId::MsgFileOutBg, ColorId::MsgOutServiceFg),
        (false, true, true) => (
            ColorId::MsgFileOutBgSelected,
            ColorId::MsgOutServiceFgSelected,
        ),
        (true, false, false) => (ColorId::MsgFileInBg, ColorId::HistoryFileInIconFg),
        (true, false, true) => (
            ColorId::MsgFileInBgSelected,
            ColorId::HistoryFileInIconFgSelected,
        ),
        (true, true, false) => (ColorId::MsgFileOutBg, ColorId::HistoryFileOutIconFg),
        (true, true, true) => (
            ColorId::MsgFileOutBgSelected,
            ColorId::HistoryFileOutIconFgSelected,
        ),
    }
}

/// Rings around userpics on a faint pill take the bubble's color; on a
/// chosen one, the pill's.
fn bubble_bg(out: bool, is_selected: bool) -> ColorId {
    match (out, is_selected) {
        (false, false) => ColorId::MsgInBg,
        (false, true) => ColorId::MsgInBgSelected,
        (true, false) => ColorId::MsgOutBg,
        (true, true) => ColorId::MsgOutBgSelected,
    }
}

pub(super) fn emoji(kind: &ReactionKind) -> Div {
    let glyph = match kind {
        ReactionKind::Emoji(emoji) => emoji_presentation(emoji),
        ReactionKind::CustomEmoji(_) => "\u{2B50}".to_owned(),
        ReactionKind::Paid => "\u{2B50}".to_owned(),
    };
    div()
        .flex_none()
        .size(px_of(REACTION_INLINE_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(EMOJI_FONT_SIZE))
        .line_height(px_of(REACTION_INLINE_SIZE))
        .child(glyph)
}

/// Telegram sends "❤" without the selector that asks for its emoji look;
/// a lone code point gets it so it does not draw as a black glyph.
fn emoji_presentation(emoji: &str) -> String {
    let mut chars = emoji.chars();
    match (chars.next(), chars.next()) {
        (Some(only), None) => format!("{only}\u{FE0F}"),
        _ => emoji.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::UserId;

    fn reaction(emoji: &str, count: i32, is_chosen: bool, recent: &[i64]) -> Reaction {
        Reaction {
            kind: ReactionKind::Emoji(emoji.to_owned()),
            count,
            is_chosen,
            recent: recent.iter().map(|&id| Sender::User(UserId(id))).collect(),
        }
    }

    fn reactions(list: Vec<Reaction>) -> Reactions {
        Reactions {
            list,
            ..Reactions::default()
        }
    }

    #[test]
    fn the_most_chosen_by_others_come_first_and_paid_before_all() {
        let mut list = vec![
            reaction("👍", 3, true, &[]),
            reaction("🔥", 3, false, &[]),
            reaction("❤", 2, false, &[]),
        ];
        list.push(Reaction {
            kind: ReactionKind::Paid,
            ..reaction("", 1, false, &[])
        });
        let all = reactions(list);
        let order: Vec<_> = sorted(&all).iter().map(|r| r.kind.clone()).collect();
        assert_eq!(
            order,
            [
                ReactionKind::Paid,
                ReactionKind::Emoji("🔥".to_owned()),
                ReactionKind::Emoji("👍".to_owned()),
                ReactionKind::Emoji("❤".to_owned()),
            ],
            "my own reaction does not count toward the order"
        );
    }

    #[test]
    fn userpics_show_only_when_everyone_is_named_and_few() {
        assert!(shows_userpics(&reactions(vec![
            reaction("👍", 2, false, &[2, 3]),
            reaction("🔥", 1, false, &[4]),
        ])));
        assert!(!shows_userpics(&reactions(vec![reaction(
            "👍",
            4,
            false,
            &[2, 3, 4, 5]
        )])));
        assert!(
            !shows_userpics(&reactions(vec![reaction("👍", 2, false, &[2])])),
            "one who reacted is not named"
        );
    }

    #[test]
    fn lone_code_points_ask_for_the_emoji_look() {
        assert_eq!(emoji_presentation("\u{2764}"), "\u{2764}\u{FE0F}");
        assert_eq!(emoji_presentation("👍"), "👍\u{FE0F}");
        assert_eq!(emoji_presentation("\u{2764}\u{FE0F}"), "\u{2764}\u{FE0F}");
        assert_eq!(
            emoji_presentation("👍🏻"),
            "👍🏻",
            "a skin tone needs no selector"
        );
    }

    #[test]
    fn a_private_chat_names_both_sides() {
        let (user, me) = (UserId(2), UserId(1));
        let senders =
            |count, is_chosen| private_senders(&reaction("👍", count, is_chosen, &[]), user, me);
        assert_eq!(senders(1, false), [Sender::User(user)]);
        assert_eq!(senders(1, true), [Sender::User(me)]);
        assert_eq!(senders(2, true), [Sender::User(user), Sender::User(me)]);
    }
}
