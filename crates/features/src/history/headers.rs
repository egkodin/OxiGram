//! The lines over a message's text: what it replies to (`Reply`) and where
//! it was forwarded from (`HistoryMessageForwarded`).

use gpui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, div, prelude::FluentBuilder as _,
};
use i18n::{Strings, key};
use model::chats::Chats;
use model::message::{Forward, Message, Origin, ReplyTo};
use style::basic::NORMAL_FONT;
use style::chat::{
    HISTORY_REPLY_BOTTOM, HISTORY_REPLY_PADDING, HISTORY_REPLY_TOP, MSG_SERVICE_NAME_FONT,
};
use style::{ColorRef, Font};
use ui::{color, font, font_height};

use super::bubble::OnClick;
use super::service::{from, user_name};
use crate::dialogs::content_preview;
use crate::px_of;

/// tdesktop's reply background (`kDefaultBgOpacity`).
const BG_OPACITY: f32 = 0.12;
/// And its left line (`kDefaultOutline1Opacity`).
const OUTLINE_OPACITY: f32 = 0.9;
/// `historyQuoteStyle`: the reply box's line and corners.
const OUTLINE: f32 = 3.0;
const RADIUS: f32 = 5.0;

/// The replied message as far as it is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Replied<'a> {
    Loaded(&'a Message),
    Loading,
    Deleted,
}

/// A reply header, ready to draw.
#[derive(Debug, Clone)]
pub(super) struct Reply {
    pub name: SharedString,
    pub text: SharedString,
    /// The replied sender's color; `None` while unknown.
    pub color_index: Option<i32>,
}

pub(super) fn reply(
    strings: &Strings,
    chats: &Chats,
    reply_to: &ReplyTo,
    replied: Replied<'_>,
    color_of: impl Fn(&Message) -> i32,
) -> Reply {
    match replied {
        Replied::Loaded(message) => {
            let text = reply_to.quote.as_ref().map_or_else(
                || content_preview(strings, &message.content).0.text,
                |quote| quote.text.clone(),
            );
            Reply {
                name: from(strings, chats, message).into(),
                text: first_line(&text).into(),
                color_index: Some(color_of(message)),
            }
        }
        Replied::Loading => Reply {
            name: SharedString::default(),
            text: strings.text(key::lng_profile_loading).to_owned().into(),
            color_index: None,
        },
        Replied::Deleted => Reply {
            name: SharedString::default(),
            text: strings.text(key::lng_deleted_message).to_owned().into(),
            color_index: None,
        },
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

/// "Forwarded from Ann", "Forwarded from News (Ann)".
pub(super) fn forwarded(strings: &Strings, chats: &Chats, forward: &Forward) -> String {
    let title = |chat| {
        chats
            .chat(chat)
            .map(|chat| chat.title.clone())
            .unwrap_or_default()
    };
    let signed = |title: String, signature: &str| {
        if signature.is_empty() {
            title
        } else {
            i18n::fill(
                strings.text(key::lng_forwarded_signed),
                &[("channel", &title), ("user", signature)],
            )
        }
    };
    let (key, name) = match &forward.origin {
        Origin::User(user) => (key::lng_forwarded, user_name(strings, chats, *user)),
        Origin::HiddenUser(name) => (key::lng_forwarded, name.clone()),
        Origin::Chat { chat, signature } => (key::lng_forwarded, signed(title(*chat), signature)),
        Origin::Channel {
            chat, signature, ..
        } => (key::lng_forwarded_channel, signed(title(*chat), signature)),
    };
    i18n::fill(strings.text(key), &[("user", &name), ("channel", &name)])
}

fn line(text: SharedString, line_font: Font, fg: gpui::Hsla) -> AnyElement {
    div()
        .truncate()
        .font(font(line_font))
        .text_size(px_of(line_font.size))
        .line_height(font_height(line_font))
        .text_color(fg)
        .child(text)
        .into_any_element()
}

/// The reply box: tinted in the replied sender's color with its name and
/// one line of its text.
/// Its colors are the tint, the name's and the text's.
// shortcut: no reply preview thumbnails; they come with media (M5).
pub(super) fn reply_element(
    reply: Reply,
    id: impl Into<ElementId>,
    [tint, name_fg, text_fg]: [ColorRef; 3],
    on_click: Option<OnClick>,
    cx: &App,
) -> AnyElement {
    let tint = color(cx, tint);
    let padding = HISTORY_REPLY_PADDING;
    div()
        .id(id)
        .relative()
        .when_some(on_click, |this, on_click| {
            this.cursor_pointer()
                .on_click(move |event, window, cx| on_click(event, window, cx))
        })
        .mt(px_of(HISTORY_REPLY_TOP))
        .mb(px_of(HISTORY_REPLY_BOTTOM))
        .rounded(gpui::px(RADIUS))
        .bg(tint.opacity(BG_OPACITY))
        .pl(px_of(padding.left))
        .pr(px_of(padding.right))
        .pt(px_of(padding.top))
        .pb(px_of(padding.bottom))
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(gpui::px(OUTLINE))
                .rounded_l(gpui::px(RADIUS))
                .bg(tint.opacity(OUTLINE_OPACITY)),
        )
        .child(line(reply.name, MSG_SERVICE_NAME_FONT, color(cx, name_fg)))
        .child(line(reply.text, NORMAL_FONT, color(cx, text_fg)))
        .into_any_element()
}

/// The forward line in the service color of the bubble.
pub(super) fn forward_element(text: String, fg: ColorRef, cx: &App) -> AnyElement {
    line(text.into(), NORMAL_FONT, color(cx, fg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::ChatId;
    use model::chat::{Chat, ChatKind};
    use model::chats::Change;
    use model::{MessageId, SupergroupId};

    #[test]
    fn forwards_name_the_origin_and_its_signature() {
        let strings = Strings::english();
        let mut chats = Chats::default();
        chats.apply(Change::Chat(Box::new(Chat::new(
            ChatId(-100),
            ChatKind::Supergroup {
                supergroup: SupergroupId(100),
                is_channel: true,
            },
            "News",
        ))));
        let forward = |origin| Forward { origin, date: 0 };
        assert_eq!(
            forwarded(
                &strings,
                &chats,
                &forward(Origin::HiddenUser("Ann".to_owned()))
            ),
            "Forwarded from Ann"
        );
        assert_eq!(
            forwarded(
                &strings,
                &chats,
                &forward(Origin::Channel {
                    chat: ChatId(-100),
                    message: MessageId(1),
                    signature: "Ann".to_owned(),
                })
            ),
            "Forwarded from News (Ann)"
        );
    }
}
