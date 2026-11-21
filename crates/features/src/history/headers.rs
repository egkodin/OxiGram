//! The lines over a message's text: what it replies to (`Reply`) and where
//! it was forwarded from (`HistoryMessageForwarded`).

use gpui::{
    AnyElement, App, Div, ElementId, InteractiveElement as _, IntoElement, ParentElement,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled, div,
    prelude::FluentBuilder as _,
};
use i18n::{Strings, key};
use model::UserId;
use model::chats::Chats;
use model::message::{External, Forward, Message, Origin, ReplyTo, Sender};
use style::basic::NORMAL_FONT;
use style::chat::{
    HISTORY_REPLY_BOTTOM, HISTORY_REPLY_PADDING, HISTORY_REPLY_TOP, MSG_SERVICE_NAME_FONT,
};
use style::{ColorRef, Font, Margins};
use ui::{Userpic, UserpicSource, color, font, font_height};

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
    match (replied, reply_to.external.as_deref()) {
        (Replied::Loaded(message), _) => {
            let text = reply_to.quote.as_ref().map_or_else(
                || {
                    content_preview(strings, &message.content)
                        .0
                        .hiding_spoilers()
                },
                |quote| quote.text.clone(),
            );
            Reply {
                name: from(strings, chats, message).into(),
                text: first_line(&text).into(),
                color_index: Some(color_of(message)),
            }
        }
        (_, Some(external)) => external_reply(strings, chats, reply_to, external),
        (Replied::Loading, None) => Reply {
            name: SharedString::default(),
            text: strings.text(key::lng_profile_loading).to_owned().into(),
            color_index: None,
        },
        (Replied::Deleted, None) => Reply {
            name: SharedString::default(),
            text: strings.text(key::lng_deleted_message).to_owned().into(),
            color_index: None,
        },
    }
}

/// A reply to a message the user cannot load or to a story, from what
/// TDLib sent with it (`Reply::update` with an external sender).
// shortcut: no peer emoji before the name and no media thumbnail; with reply previews in M5.
fn external_reply(
    strings: &Strings,
    chats: &Chats,
    reply_to: &ReplyTo,
    external: &External,
) -> Reply {
    let accent = |chat| chats.chat(chat).map(|chat| chat.accent_color);
    let (name, color_index, text) = match external {
        External::Message { origin, media } => {
            let text = reply_to
                .quote
                .as_ref()
                .map(|quote| quote.text.clone())
                .or_else(|| {
                    let media = media.as_ref()?.without_caption();
                    Some(content_preview(strings, &media).0.hiding_spoilers())
                })
                .unwrap_or_default();
            let color = match origin {
                Origin::User(user) => chats.user(*user).map(|user| user.accent_color),
                Origin::HiddenUser(_) => None,
                Origin::Chat { chat, .. } | Origin::Channel { chat, .. } => accent(*chat),
            };
            (origin_name(strings, chats, origin), color, text)
        }
        External::Story { poster } => (
            chat_title(chats, *poster),
            accent(*poster),
            strings.text(key::lng_in_dlg_story).to_owned(),
        ),
    };
    Reply {
        name: name.into(),
        text: first_line(&text).into(),
        color_index,
    }
}

fn chat_title(chats: &Chats, chat: model::ChatId) -> String {
    chats
        .chat(chat)
        .map(|chat| chat.title.clone())
        .unwrap_or_default()
}

/// Who sent a message elsewhere, by name.
pub(super) fn origin_name(strings: &Strings, chats: &Chats, origin: &Origin) -> String {
    match origin {
        Origin::User(user) => user_name(strings, chats, *user),
        Origin::HiddenUser(name) => name.clone(),
        Origin::Chat { chat, .. } | Origin::Channel { chat, .. } => chat_title(chats, *chat),
    }
}

pub(super) fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

/// "via @bot" for messages sent through an inline bot; none while the
/// bot is unknown or has no username.
pub(super) fn via(strings: &Strings, chats: &Chats, bot: UserId) -> Option<String> {
    let username = &chats.user(bot)?.username;
    (!username.is_empty()).then(|| {
        i18n::fill(
            strings.text(key::lng_inline_bot_via),
            &[("inline_bot", &format!("@{username}"))],
        )
    })
}

/// "Forwarded from Ann", "Forwarded from News (Ann)".
pub(super) fn forwarded(strings: &Strings, chats: &Chats, forward: &Forward) -> String {
    let title = |chat| chat_title(chats, chat);
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

pub(super) fn forward_sender(origin: &Origin) -> Option<Sender> {
    match origin {
        Origin::User(user) => Some(Sender::User(*user)),
        Origin::Chat { chat, .. } | Origin::Channel { chat, .. } => Some(Sender::Chat(*chat)),
        Origin::HiddenUser(_) => None,
    }
}

pub(super) fn line(text: SharedString, line_font: Font, fg: gpui::Hsla) -> AnyElement {
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
    quote_frame(id, color(cx, tint), HISTORY_REPLY_PADDING)
        .when_some(on_click, |this, on_click| {
            this.cursor_pointer()
                .on_click(move |event, window, cx| on_click(event, window, cx))
        })
        .mt(px_of(HISTORY_REPLY_TOP))
        .mb(px_of(HISTORY_REPLY_BOTTOM))
        .child(line(reply.name, MSG_SERVICE_NAME_FONT, color(cx, name_fg)))
        .child(line(reply.text, NORMAL_FONT, color(cx, text_fg)))
        .into_any_element()
}

/// A box tinted in `tint` with a line on its left, as replies and link
/// previews draw (`Ui::Text::QuotePaintCache`).
pub(super) fn quote_frame(
    id: impl Into<ElementId>,
    tint: gpui::Hsla,
    padding: Margins,
) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
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
}

#[derive(Clone)]
pub(super) struct ForwardHeader {
    pub text: String,
    pub hidden: Option<SharedString>,
    pub avatar: Option<UserpicSource>,
    pub on_click: Option<OnClick>,
}

pub(super) fn forward_element(header: ForwardHeader, fg: ColorRef, cx: &App) -> AnyElement {
    let fg = color(cx, fg);
    let origin = div()
        .id("message-forward-origin")
        .flex()
        .items_center()
        .gap(gpui::px(4.0))
        .when_some(header.on_click, |div, click| {
            div.cursor_pointer()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    click(event, window, cx);
                })
        })
        .children(
            header
                .avatar
                .map(|avatar| Userpic::new(gpui::px(16.0), avatar)),
        )
        .child(line(header.text.into(), NORMAL_FONT, fg));
    div()
        .child(origin)
        .children(header.hidden.map(|hidden| {
            div()
                .font(font(NORMAL_FONT))
                .text_size(px_of(NORMAL_FONT.size))
                .text_color(fg)
                .child(hidden)
        }))
        .into_any_element()
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
        assert_eq!(forward_sender(&Origin::HiddenUser("Ann".to_owned())), None);
        assert_eq!(
            forward_sender(&Origin::User(UserId(7))),
            Some(Sender::User(UserId(7)))
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

    #[test]
    fn via_names_the_bot_by_its_username() {
        let strings = Strings::english();
        let mut chats = Chats::default();
        let with_username = |change, username: &str| match change {
            Change::User(mut user) => {
                user.username = username.to_owned();
                Change::User(user)
            }
            other => other,
        };
        chats.apply(with_username(
            super::super::service::tests::user(7, "GIF"),
            "gif",
        ));
        chats.apply(super::super::service::tests::user(8, "No Name"));
        assert_eq!(
            via(&strings, &chats, UserId(7)).as_deref(),
            Some("via @gif")
        );
        assert_eq!(via(&strings, &chats, UserId(8)), None);
        assert_eq!(via(&strings, &chats, UserId(9)), None);
    }

    #[test]
    fn replies_elsewhere_name_their_origin_until_the_message_loads() {
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
        let reply_to = |external| ReplyTo {
            chat: ChatId(-100),
            message: MessageId(5),
            quote: None,
            external: Some(Box::new(external)),
        };
        let post = reply_to(External::Message {
            origin: Origin::Channel {
                chat: ChatId(-100),
                message: MessageId(5),
                signature: String::new(),
            },
            media: Some(Box::new(model::message::Content::Photo(
                model::message::Media {
                    caption: model::text::FormattedText::plain("Sunrise"),
                    thumbnail: None,
                    dimensions: None,
                    preview: None,
                    duration: 0,
                    file: None,
                    has_spoiler: false,
                    start_timestamp: 0,
                },
            ))),
        });
        let header = reply(&strings, &chats, &post, Replied::Deleted, |_| 0);
        assert_eq!(
            (header.name.as_ref(), header.text.as_ref()),
            ("News", "Photo"),
            "the caption is left out"
        );
        let story = reply_to(External::Story {
            poster: ChatId(-100),
        });
        let header = reply(&strings, &chats, &story, Replied::Loading, |_| 0);
        assert_eq!(
            (header.name.as_ref(), header.text.as_ref()),
            ("News", "Story")
        );
    }
}
