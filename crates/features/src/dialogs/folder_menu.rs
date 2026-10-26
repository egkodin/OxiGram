//! The folder tabs' context menu (`ChatsFiltersTabs` `ShowMenu`) and the
//! mark-as-read item it shares with the archive (`MarkAsReadMenu`).

use std::rc::Rc;

use gpui::{App, Context, Pixels, Point, Window};
use i18n::key;
use model::chat::ChatList;
use model::{ChatId, FolderId, MessageId};
use style::menu_icons::{MENU_ICON_DELETE_ATTENTION, MENU_ICON_MARK_READ};
use ui::{Markup, MenuTone, PopupMenu};

use super::Dialogs;
use crate::boxes::{self, Callback, ConfirmArgs};
use crate::lang::tr;
use crate::peer_menu::send;

/// Marking more unread messages than this read asks first
/// (`kMaxUnreadWithoutConfirmation`).
const MAX_UNREAD_WITHOUT_CONFIRMATION: i64 = 1000;

impl Dialogs {
    /// The menu of the tab showing `list`.
    // shortcut: no Edit folder / Edit folders items until folder settings exist (M6).
    pub(super) fn show_folder_menu(
        &mut self,
        list: ChatList,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_menu(position, window, cx, move |this, menu, cx| {
            this.add_mark_read(menu, list, cx);
            if let ChatList::Folder(folder) = list {
                this.add_remove_folder(menu, folder, cx);
            }
        });
    }

    fn add_remove_folder(&self, menu: &mut PopupMenu, folder: FolderId, cx: &App) {
        let has_links = self
            .store
            .read(cx)
            .state()
            .chats
            .folders()
            .iter()
            .any(|known| known.id == folder && known.has_my_invite_links);
        let (text, button) = if has_links {
            (key::lng_filters_delete_sure, key::lng_box_delete)
        } else {
            (key::lng_filters_remove_sure, key::lng_filters_remove_yes)
        };
        let session = self.store.read(cx).session().clone();
        let layers = self.layers.clone();
        menu.action(
            tr(cx, key::lng_filters_context_remove),
            MENU_ICON_DELETE_ATTENTION,
            MenuTone::Attention,
            move |window, cx| {
                let session = session.clone();
                let remove: Callback = Rc::new(move |_, cx| {
                    send(session.delete_folder(folder), "remove folder", cx);
                });
                boxes::confirm_attention(
                    &layers,
                    tr(cx, text).to_string(),
                    tr(cx, button),
                    remove,
                    window,
                    cx,
                );
            },
        );
    }

    /// "Mark as read" for every unread chat of `list`, asking first past
    /// [`MAX_UNREAD_WITHOUT_CONFIRMATION`] messages; nothing when all are
    /// read (`MarkAsReadMenu::AddChatListAction`).
    pub(super) fn add_mark_read(&self, menu: &mut PopupMenu, list: ChatList, cx: &App) {
        let state = self.store.read(cx).state();
        let unread: Vec<(ChatId, Option<MessageId>, i32)> = state
            .chats
            .list(list)
            .filter_map(|id| state.chats.chat(id))
            .filter(|chat| chat.unread_count > 0 || chat.is_marked_as_unread)
            .map(|chat| {
                let last = chat.last_message.as_ref().map(|message| message.id);
                (chat.id, last, chat.unread_count)
            })
            .collect();
        if unread.is_empty() {
            return;
        }
        let (text, sure) = match list {
            ChatList::Main => (
                key::lng_context_mark_read_all,
                key::lng_context_mark_read_all_sure,
            ),
            ChatList::Archive => (
                key::lng_context_mark_read,
                key::lng_context_mark_read_archive_sure,
            ),
            ChatList::Folder(_) => (key::lng_context_mark_read, key::lng_context_mark_read_sure),
        };
        let messages: i64 = unread.iter().map(|(_, _, count)| i64::from(*count)).sum();
        let session = self.store.read(cx).session().clone();
        let layers = self.layers.clone();
        let read_all: Callback = Rc::new(move |_, cx| {
            for (chat, last, _) in &unread {
                send(session.read_chat(*chat, *last), "mark list read", cx);
            }
        });
        menu.action(
            tr(cx, text),
            MENU_ICON_MARK_READ,
            MenuTone::Normal,
            move |window, cx| {
                if messages <= MAX_UNREAD_WITHOUT_CONFIRMATION {
                    read_all(window, cx);
                    return;
                }
                let mut args = ConfirmArgs::new(Markup::plain(tr(cx, sure).to_string()));
                args.confirm_text = Some(tr(cx, key::lng_box_ok));
                args.cancel_text = Some(tr(cx, key::lng_cancel));
                args.confirmed = Some(read_all.clone());
                boxes::confirm(&layers, args, window, cx);
            },
        );
    }
}
