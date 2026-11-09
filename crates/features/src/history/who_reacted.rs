//! The group message menu's first line (`AddWhoReactedAction`): who saw
//! the message and who reacted, opening the reactions list.

use std::rc::Rc;

use chrono::Local;
use gpui::{App, Task, Window};
use model::message::{Content, Message, Sender};
use model::message_actions::Abilities;
use model::reaction::AddedReactions;
use model::{RequestError, UserId};
use style::Icon;
use style::chat::{WHO_READ_CHECKS, WHO_READ_PLAYED, WHO_READ_REACTIONS};

use super::read_date::{self, Seen, SeenKind, WhoRead, seen};
use super::service::sender_name;
use super::{HistoryView, reactions_box};
use crate::lang::strings;
use crate::store::spawn;

/// Who reacted, as many as the menu asks for (`kContextReactionsLimit`).
const MENU_REACTORS_LIMIT: i32 = 50;

/// A who-read answer, or none when TDLib could not tell.
pub(super) fn known<T>(result: Result<T, RequestError>) -> Option<T> {
    result
        .inspect_err(|error| tracing::warn!(%error, "who read the message unknown"))
        .ok()
}

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
        Some(spawn(async move { known(task.await) }, cx))
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
            (Some(_), Content::VideoNote { .. }) => SeenKind::Watched,
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

    /// Asks who saw an outgoing message alongside its abilities: the read
    /// date in private chats, the viewers in groups.
    pub(super) fn who_read(&self, message: &Message, cx: &App) -> Task<Option<WhoRead>> {
        let session = self.store.read(cx).session();
        if self.is_private(cx) {
            let task = session.message_read_date(self.chat, message.id);
            spawn(async move { known(task.await).map(WhoRead::Date) }, cx)
        } else {
            let task = session.message_viewers(self.chat, message.id);
            spawn(async move { known(task.await).map(WhoRead::Viewers) }, cx)
        }
    }

    /// The menu's first line, when the abilities allow asking.
    pub(super) fn who_read_line(
        &self,
        message: &Message,
        who_read: Option<WhoRead>,
        reactors: Option<AddedReactions>,
        abilities: Option<&Abilities>,
        cx: &App,
    ) -> Option<WhoLine> {
        let abilities = abilities?;
        let viewers = match who_read {
            Some(WhoRead::Date(date)) if abilities.can_get_read_date => {
                return read_date::text(strings(cx), date, &Local::now()).map(|text| WhoLine {
                    text,
                    icon: WHO_READ_CHECKS,
                    on_click: None,
                });
            }
            Some(WhoRead::Viewers(viewers)) if abilities.can_get_viewers => Some(viewers),
            _ => None,
        };
        self.who_reacted_line(message, viewers, reactors, cx)
    }

    /// Opens the reactions list for the message.
    // shortcut: one reactor's line opens the list too, not their profile; profiles come with M6.
    fn show_reactions(&self, message: &Message) -> OnClick {
        let (store, layers, chat) = (self.store.clone(), self.layers.clone(), self.chat);
        let (id, reactions) = (message.id, message.reactions.clone().unwrap_or_default());
        Rc::new(move |window, cx| {
            reactions_box::show(&store, &layers, (chat, id), (&reactions, None), window, cx);
        })
    }
}
