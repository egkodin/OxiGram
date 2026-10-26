//! What clicking a link in a message does (`UrlClickHandler`,
//! `HiddenUrlClickHandler`, `MentionClickHandler`, `HashtagClickHandler`).

use std::rc::Rc;
use std::time::Duration;

use gpui::{App, Entity, Window};
use i18n::key;
use ui::{InputField, Markup};

use crate::boxes::{ConfirmArgs, confirm};
use crate::dialogs::Dialogs;
use crate::history::Link;
use crate::lang::tr;
use crate::store::spawn;
use crate::{Layers, Store};

/// `Ui::Toast::kDefaultDuration`.
const TOAST_DURATION: Duration = Duration::from_millis(1500);

/// The views a link can reach.
#[derive(Debug, Clone)]
pub(crate) struct Targets {
    pub store: Entity<Store>,
    pub layers: Entity<Layers>,
    pub dialogs: Entity<Dialogs>,
    pub search: Entity<InputField>,
}

pub(crate) fn open(link: Link, targets: &Targets, window: &mut Window, cx: &mut App) {
    match link {
        Link::Url(url) => cx.open_url(&url),
        Link::Email(address) => cx.open_url(&format!("mailto:{address}")),
        Link::HiddenUrl(url) => confirm_hidden(url, &targets.layers, window, cx),
        Link::Hashtag(tag) => targets
            .search
            .update(cx, |search, cx| search.set_text(&tag, None, cx)),
        Link::Username(username) => {
            let task = targets
                .store
                .read(cx)
                .session()
                .search_public_chat(&username);
            let not_found = tr(cx, key::lng_username_not_found);
            open_chat_when(spawn(task, cx), targets, Some(not_found), cx);
        }
        Link::User(user) => {
            let task = targets.store.read(cx).session().create_private_chat(user);
            open_chat_when(spawn(task, cx), targets, None, cx);
        }
        // Spoilers reveal in the message itself.
        Link::Spoiler => {}
    }
}

/// A link whose text is not its address asks first, showing the address.
fn confirm_hidden(url: String, layers: &Entity<Layers>, window: &mut Window, cx: &mut App) {
    let text = format!("{}\n\n{url}", tr(cx, key::lng_open_this_link));
    let mut args = ConfirmArgs::new(Markup::plain(text));
    args.confirm_text = Some(tr(cx, key::lng_open_link));
    args.cancel_text = Some(tr(cx, key::lng_cancel));
    args.confirmed = Some(Rc::new(move |_, cx| cx.open_url(&url)));
    confirm(layers, args, window, cx);
}

/// Opens the chat `task` finds; `not_found` is the toast when it finds none.
fn open_chat_when(
    task: gpui::Task<Result<model::ChatId, model::RequestError>>,
    targets: &Targets,
    not_found: Option<gpui::SharedString>,
    cx: &mut App,
) {
    let dialogs = targets.dialogs.downgrade();
    let layers = targets.layers.downgrade();
    cx.spawn(async move |cx| match task.await {
        // The views are gone once the user signed out meanwhile.
        Ok(chat) => {
            let _ = dialogs.update(cx, |dialogs, cx| dialogs.select(chat, cx));
        }
        Err(error) => {
            tracing::warn!(%error, "opening a mentioned chat failed");
            if let Some(text) = not_found {
                let _ = layers.update(cx, |layers, cx| {
                    layers.show_toast(text, &style::widgets::DEFAULT_TOAST, TOAST_DURATION, cx);
                });
            }
        }
    })
    .detach();
}
