//! The group message menu's first line (`AddWhoReactedAction`): who saw
//! the message and who reacted, opening the reactions list.

use std::rc::Rc;

use gpui::{App, Task, Window};
use model::UserId;
use model::message::{Content, Message, Sender};
use model::reaction::AddedReactions;
use style::Icon;
use style::chat::{WHO_READ_CHECKS, WHO_READ_PLAYED, WHO_READ_REACTIONS};

use super::read_date::{Seen, SeenKind, seen};
use super::service::sender_name;
use super::{HistoryView, menu, reactions_box};
use crate::lang::strings;
use crate::store::spawn;

/// Who reacted, as many as the menu asks for (`kContextReactionsLimit`).
const MENU_REACTORS_LIMIT: i32 = 50;

type OnClick = Rc<dyn Fn(&mut Window, &mut App)>;

/// The line over a message's menu, ready to add.
pub(super) struct WhoLine {
    pub text: String,
    pub icon: Icon,
    pub on_click: Option<OnClick>,
}

impl HistoryView {
    /// Asks who reacted to a group message, when TDLib can tell.
    pub(super) fn reactors(
        &self,
        message: &Message,
        cx: &App,
    ) -> Option<Task<Option<AddedReactions>>> {
        message
            .reactions
            .as_ref()
            .filter(|r| r.can_get_added && !r.are_tags && !r.list.is_empty())?;
        let task = self.store.read(cx).session().added_reactions(
            self.chat,
            message.id,
            None,
            String::new(),
            MENU_REACTORS_LIMIT,
        );
        Some(spawn(async move { menu::known(task.await) }, cx))
    }

    /// "3 Seen", "2/5 Reacted" and the like; `viewers` only when the user
    /// may see them.
    pub(super) fn who_reacted_line(
        &self,
        message: &Message,
        viewers: Option<Vec<UserId>>,
        reactors: Option<AddedReactions>,
        cx: &App,
    ) -> Option<WhoLine> {
        if viewers.is_none() && reactors.is_none() {
            return None;
        }
        let reactions = message
            .reactions
            .as_ref()
            .filter(|reactions| !reactions.are_tags)
            .map_or(0, |reactions| reactions.list.iter().map(|r| r.count).sum());
        let kind = match (&viewers, &message.content) {
            (None, _) => SeenKind::Reacted,
            (Some(_), Content::VideoNote) => SeenKind::Watched,
            (Some(_), Content::Voice { .. }) => SeenKind::Listened,
            (Some(_), _) => SeenKind::Seen,
        };
        let read = viewers.as_ref().map_or(0, Vec::len);
        let reacted = reactors.into_iter().flat_map(|page| page.list);
        let only_saw = viewers.into_iter().flatten().map(Sender::User);
        let mut senders: Vec<(Sender, bool)> = Vec::new();
        for (sender, has_reacted) in reacted
            .map(|r| (r.sender, true))
            .chain(only_saw.map(|s| (s, false)))
        {
            if !senders.iter().any(|(known, _)| *known == sender) {
                senders.push((sender, has_reacted));
            }
        }
        let chats = &self.store.read(cx).state().chats;
        let participants = senders
            .into_iter()
            .map(|(sender, has_reacted)| (sender_name(strings(cx), chats, sender), has_reacted))
            .collect();
        let text = seen(
            strings(cx),
            &Seen {
                participants,
                reactions,
                read: i32::try_from(read).unwrap_or(i32::MAX),
                kind,
            },
        );
        let icon = match kind {
            _ if reactions > 0 => WHO_READ_REACTIONS,
            SeenKind::Seen => WHO_READ_CHECKS,
            _ => WHO_READ_PLAYED,
        };
        Some(WhoLine {
            text,
            icon,
            on_click: (reactions > 0).then(|| self.show_reactions(message)),
        })
    }

    /// Opens the reactions list for the message.
    // shortcut: one reactor's line opens the list too, not their profile; profiles come with M6.
    fn show_reactions(&self, message: &Message) -> OnClick {
        let (store, layers, chat) = (self.store.clone(), self.layers.clone(), self.chat);
        let (id, reactions) = (message.id, message.reactions.clone().unwrap_or_default());
        Rc::new(move |window, cx| {
            reactions_box::show(&store, &layers, chat, id, &reactions, window, cx);
        })
    }
}
