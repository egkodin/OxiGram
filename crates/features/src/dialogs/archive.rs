//! The archive's row in the main list (`Data::Folder`): the names of its
//! latest chats, the unread ones highlighted.

use chrono::{DateTime, Local};
use gpui::{AnyElement, Context, ParentElement, SharedString, Styled, div};
use i18n::{Strings, key};
use model::State;
use model::chat::{Chat, ChatList};
use model::chats::Chats;
use style::{ColorId, ColorRef};
use ui::{UserpicSource, color};

use super::preview::{Preview, Span, SpanKind};
use super::row::{self, Badges, Lit, NarrowRow, Row, RowText};
use super::{Dialogs, RowKey, date_text};
use crate::lang::strings;

/// Chat names the row lists (`kShowChatNamesCount`).
const NAMES: usize = 8;

/// The chats whose names the row shows: the latest by last message,
/// looking past pinned chats (`Folder::reorderLastHistories`).
fn last_chats(chats: &Chats) -> Vec<&Chat> {
    let date = |chat: &Chat| chat.last_message.as_ref().map_or(0, |message| message.date);
    let mut last: Vec<&Chat> = Vec::with_capacity(NAMES + 1);
    let mut unpinned = 0;
    for chat in chats
        .list(ChatList::Archive)
        .filter_map(|id| chats.chat(id))
    {
        let at = last.partition_point(|shown| date(shown) >= date(chat));
        if last.len() < NAMES || at < last.len() {
            last.insert(at, chat);
        }
        last.truncate(NAMES);
        let is_pinned = chat
            .position(ChatList::Archive)
            .is_some_and(|position| position.is_pinned);
        if !is_pinned {
            unpinned += 1;
            if unpinned >= NAMES {
                break;
            }
        }
    }
    last
}

/// `ComposeFolderListEntryText`: "A, B, C and 5 more chats".
pub(super) fn preview(strings: &Strings, chats: &Chats) -> Preview {
    let last = last_chats(chats);
    if last.is_empty() {
        return Preview::default();
    }
    let count = last.len().max(chats.list_len(ChatList::Archive));
    // "and 1 more chat" reads worse than one more name.
    let shown = if last.len() > 1 && count == last.len() + 1 {
        last.len() - 1
    } else {
        last.len()
    };
    let name = |chat: &Chat| {
        let text = chat.title.clone();
        let spans = if chat.unread_count > 0 || chat.is_marked_as_unread {
            vec![
                Span {
                    range: 0..text.len(),
                    kind: SpanKind::Semibold,
                },
                Span {
                    range: 0..text.len(),
                    kind: SpanKind::Colorized,
                },
            ]
        } else {
            Vec::new()
        };
        Preview { text, spans }
    };
    let mut names = last[..shown].iter().map(|chat| name(chat));
    let Some(first) = names.next() else {
        return Preview::default();
    };
    let accumulated = names.fold(first, |accumulated, chat| {
        fill(
            strings.text(key::lng_archived_last_list),
            &[("accumulated", accumulated), ("chat", chat)],
        )
    });
    if shown < count {
        let more = count - shown;
        let template = strings
            .plural(
                key::lng_archived_last,
                i64::try_from(more).unwrap_or(i64::MAX),
            )
            .replace("{count}", &more.to_string());
        fill(&template, &[("chats", accumulated)])
    } else {
        accumulated
    }
}

/// `template` with each `{name}` replaced by its preview, spans shifted.
fn fill(template: &str, args: &[(&str, Preview)]) -> Preview {
    let mut out = Preview::default();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}').map(|close| open + close) else {
            break;
        };
        let name = &rest[open + 1..close];
        let Some((_, value)) = args.iter().find(|(key, _)| *key == name) else {
            out.text.push_str(&rest[..=close]);
            rest = &rest[close + 1..];
            continue;
        };
        out.text.push_str(&rest[..open]);
        let shift = out.text.len();
        out.spans.extend(value.spans.iter().map(|span| Span {
            range: span.range.start + shift..span.range.end + shift,
            kind: span.kind,
        }));
        out.text.push_str(&value.text);
        rest = &rest[close + 1..];
    }
    out.text.push_str(rest);
    out
}

/// The archive row's date: the latest of its chats' messages.
pub(super) fn date(chats: &Chats) -> Option<i32> {
    last_chats(chats)
        .first()
        .and_then(|chat| chat.last_message.as_ref())
        .map(|message| message.date)
}

impl Dialogs {
    pub(super) fn archive_element(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let lit = if self.hovered == Some(RowKey::Archive) {
            Lit::Over
        } else {
            Lit::Normal
        };
        let row = archive_row(self.store.read(cx).state(), strings(cx), lit, &Local::now());
        let open = |this: &mut Self, cx: &mut Context<Self>| this.open_list(ChatList::Archive, cx);
        if self.narrow {
            return self.wrap_row(RowKey::Archive, NarrowRow(row), cx, open);
        }
        self.wrap_row(RowKey::Archive, row, cx, open)
    }

    /// The collapsed archive (`PaintCollapsedRow`): its name and a muted
    /// counter of unread chats.
    pub(super) fn archive_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let is_hovered = self.hovered == Some(RowKey::Archive);
        let unread = self
            .store
            .read(cx)
            .state()
            .chats
            .unread(ChatList::Archive)
            .chats;
        let bg = if is_hovered {
            ColorId::DialogsBgOver
        } else {
            ColorId::DialogsBg
        };
        if self.narrow {
            // The archive's userpic, small and centered.
            let size = crate::px_of(style::dialogs::DIALOGS_UNREAD_HEIGHT);
            let bar = div()
                .h(crate::px_of(style::dialogs::DIALOGS_IMPORTANT_BAR_HEIGHT))
                .w_full()
                .bg(color(cx, ColorRef::Palette(bg)))
                .flex()
                .items_center()
                .justify_center()
                .child(ui::Userpic::new(size, UserpicSource::Archive));
            return self.wrap_row(RowKey::Archive, bar, cx, |this, cx| {
                this.open_list(ChatList::Archive, cx);
            });
        }
        let bar =
            div()
                .h(crate::px_of(style::dialogs::DIALOGS_IMPORTANT_BAR_HEIGHT))
                .w_full()
                .bg(color(cx, ColorRef::Palette(bg)))
                .flex()
                .items_center()
                .pl(crate::px_of(style::dialogs::DIALOGS_TOP_BAR_LEFT_PADDING))
                .pr(crate::px_of(
                    style::dialogs::DEFAULT_DIALOG_ROW.padding.right,
                ))
                .child(
                    div()
                        .flex_1()
                        .font(ui::font(style::basic::SEMIBOLD_FONT))
                        .text_size(crate::px_of(style::basic::SEMIBOLD_FONT.size))
                        .text_color(color(cx, ColorRef::Palette(ColorId::DialogsNameFg)))
                        .child(crate::lang::tr(cx, key::lng_archived_name)),
                )
                .children((unread > 0).then(|| {
                    row::unread_badge(unread.to_string().into(), true, row::Lit::Normal, cx)
                }));
        self.wrap_row(RowKey::Archive, bar, cx, |this, cx| {
            this.open_list(ChatList::Archive, cx);
        })
    }
}

/// The archive folder's row: its latest chats' names, and how many of
/// them are unread in a muted badge.
fn archive_row(state: &State, strings: &Strings, lit: Lit, now: &DateTime<Local>) -> Row {
    let chats = &state.chats;
    let unread = chats.unread(ChatList::Archive).chats;
    Row {
        userpic: UserpicSource::Archive,
        is_online: false,
        name: strings.text(key::lng_archived_name).into(),
        type_icon: None,
        name_badge: None,
        scam_text: SharedString::default(),
        date: date(chats).map(|date| date_text(strings, date, now)),
        send_state: None,
        text: RowText::Message {
            preview: preview(strings, chats),
            thumbnail: None,
        },
        badges: Badges {
            unread: (unread > 0).then(|| unread.to_string().into()),
            is_muted: true,
            has_mention: false,
            has_reaction: false,
        },
        is_pinned: false,
        lit,
    }
}

#[cfg(test)]
mod tests {
    use model::ChatId;

    use super::*;

    fn ids(chats: &[&Chat]) -> Vec<ChatId> {
        chats.iter().map(|chat| chat.id).collect()
    }

    fn plain(text: &str) -> Preview {
        Preview {
            text: text.to_owned(),
            spans: Vec::new(),
        }
    }

    #[test]
    fn fills_placeholders_and_shifts_spans() {
        let bold = Preview {
            text: "Bob".to_owned(),
            spans: vec![Span {
                range: 0..3,
                kind: SpanKind::Semibold,
            }],
        };
        let filled = fill(
            "{accumulated}, {chat}",
            &[("accumulated", plain("Ann")), ("chat", bold)],
        );
        assert_eq!(filled.text, "Ann, Bob");
        assert_eq!(filled.spans[0].range, 5..8);
        assert_eq!(fill("{x} {unknown}", &[]).text, "{x} {unknown}");
    }

    #[test]
    fn last_chats_are_empty_without_an_archive() {
        let chats = Chats::default();
        assert_eq!(ids(&last_chats(&chats)), Vec::<ChatId>::new());
        assert_eq!(preview(&Strings::default(), &chats), Preview::default());
    }
}
