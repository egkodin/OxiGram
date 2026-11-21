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
            let task = spawn(task, cx);
            let targets = targets.clone();
            window
                .spawn(cx, async move |cx| match task.await {
                    Ok(chat) => {
                        let _ =
                            cx.update(|window, cx| crate::info::show(targets, chat, window, cx));
                    }
                    Err(error) => {
                        tracing::warn!(%error, "opening a mentioned profile failed");
                        targets.layers.update(cx, |layers, cx| {
                            layers.show_toast(
                                not_found,
                                &style::widgets::DEFAULT_TOAST,
                                TOAST_DURATION,
                                cx,
                            );
                        });
                    }
                })
                .detach();
        }
        Link::Profile(user) => {
            crate::info::show(targets.clone(), model::ChatId(user.0), window, cx)
        }
        Link::User(user) => {
            let task = targets.store.read(cx).session().create_private_chat(user);
            open_chat_when(spawn(task, cx), targets, cx);
        }
        // Spoilers reveal in the message itself; phones, cards and dates
        // open their menus there, and code copies there.
        Link::Spoiler
        | Link::CustomEmoji(_)
        | Link::Phone(_)
        | Link::BankCard(_)
        | Link::Date(_)
        | Link::Code(_) => {}
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

/// Opens the chat returned by TDLib.
fn open_chat_when(
    task: gpui::Task<Result<model::ChatId, model::RequestError>>,
    targets: &Targets,
    cx: &mut App,
) {
    let dialogs = targets.dialogs.downgrade();
    cx.spawn(async move |cx| match task.await {
        // The views are gone once the user signed out meanwhile.
        Ok(chat) => {
            let _ = dialogs.update(cx, |dialogs, cx| dialogs.select(chat, cx));
        }
        Err(error) => {
            tracing::warn!(%error, "opening a mentioned chat failed");
        }
    })
    .detach();
}
