//! Who a message is from, as its bubble names them: the author over a
//! group message or a channel post, and the sender's name, color and
//! userpic.

use model::FileId;
use model::State;
use model::chat::ChatKind;
use model::message::{Message, Sender};

use super::bubble::{Author, OnClick};
use super::headers::{self, ForwardHeader};
use super::{HistoryEvent, HistoryView};
use crate::dialogs::photo_userpic;
use crate::lang::strings;
use gpui::Context;
use std::rc::Rc;

impl HistoryView {
    /// The author of an incoming message in a group, with the userpic file
    /// to download, or the channel of a post, named without a userpic
    /// (`Message::hasFromName`, `displayFromPhoto`).
    pub(super) fn author(
        &self,
        state: &State,
        message: &Message,
    ) -> Option<(Author, Option<FileId>)> {
        let chats = &state.chats;
        let kind = chats.chat(self.chat).map(|chat| &chat.kind);
        let is_channel = matches!(
            kind,
            Some(ChatKind::Supergroup {
                is_channel: true,
                ..
            })
        );
        let is_group = matches!(
            kind,
            Some(
                ChatKind::Group { .. }
                    | ChatKind::Supergroup {
                        is_channel: false,
                        ..
                    }
            )
        );
        if !((is_channel && message.is_channel_post) || (is_group && !message.is_outgoing)) {
            return None;
        }
        let (name, color_index, (userpic, file)) = sender_face(chats, message.sender)?;
        let (userpic, file) = if is_channel {
            (None, None)
        } else {
            (Some(userpic), file)
        };
        Some((
            Author {
                name: name.into(),
                color_index,
                userpic,
                sender: message.sender,
            },
            file,
        ))
    }

    pub(super) fn on_sender(
        &self,
        sender: Sender,
        open_channel: bool,
        cx: &Context<Self>,
    ) -> OnClick {
        let weak = cx.weak_entity();
        Rc::new(move |_, _, cx| {
            let _ = weak.update(cx, |_, cx| match sender {
                Sender::User(user) => cx.emit(HistoryEvent::OpenProfile(model::ChatId(user.0))),
                Sender::Chat(chat) if open_channel => cx.emit(HistoryEvent::OpenChat(chat)),
                Sender::Chat(chat) => cx.emit(HistoryEvent::OpenProfile(chat)),
            });
        })
    }

    pub(super) fn forward_header(
        &self,
        message: &Message,
        state: &State,
        cx: &Context<Self>,
    ) -> Option<(ForwardHeader, Option<FileId>)> {
        let forward = message.forward.as_ref()?;
        let sender = headers::forward_sender(&forward.origin);
        let face = sender.and_then(|sender| sender_face(&state.chats, sender));
        let (avatar, file) =
            face.map_or((None, None), |(_, _, (avatar, file))| (Some(avatar), file));
        Some((
            ForwardHeader {
                text: headers::forwarded(strings(cx), &state.chats, forward),
                hidden: sender.is_none().then(|| {
                    strings(cx)
                        .text(i18n::key::lng_forwarded_hidden)
                        .to_owned()
                        .into()
                }),
                avatar,
                on_click: sender.map(|sender| {
                    self.on_sender(
                        sender,
                        matches!(forward.origin, model::message::Origin::Channel { .. }),
                        cx,
                    )
                }),
            },
            file,
        ))
    }
}

/// The color of whoever sent `message` (`colorIndex`).
pub(super) fn sender_color(state: &State, message: &Message) -> i32 {
    match message.sender {
        Sender::User(id) => state.chats.user(id).map_or(0, |user| user.accent_color),
        Sender::Chat(id) => state.chats.chat(id).map_or(0, |chat| chat.accent_color),
    }
}

/// A sender's name, color and userpic, with the photo file to download.
pub(super) fn sender_face(
    chats: &model::chats::Chats,
    sender: Sender,
) -> Option<(String, i32, (ui::UserpicSource, Option<FileId>))> {
    match sender {
        Sender::User(id) => {
            let user = chats.user(id)?;
            let name = user.name();
            let userpic = photo_userpic(chats, user.photo.as_ref(), user.accent_color, &name);
            Some((name, user.accent_color, userpic))
        }
        Sender::Chat(id) => {
            let chat = chats.chat(id)?;
            let userpic = photo_userpic(chats, chat.photo.as_ref(), chat.accent_color, &chat.title);
            Some((chat.title.clone(), chat.accent_color, userpic))
        }
    }
}
