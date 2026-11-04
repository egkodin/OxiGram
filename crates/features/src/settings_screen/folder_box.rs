//! "New Folder" / "Edit Folder" (`EditFilterBox`): the name, the included
//! chat types and chats, the excluded ones; Create or Save.
// shortcut: no icon picker, tag color, custom emoji in the name, default name from the chosen types, length counter or invite links section; with rows "Create and edit folders" and "Shareable folders".

use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Task, Window, div,
    linear_color_stop, linear_gradient,
};
use i18n::{Key, key};
use model::chat_folders::{ChatType, FolderRules, NAME_MAX};
use model::{ChatId, FolderId};
use style::layers::{BOX_WIDE_WIDTH, DEFAULT_BOX};
use style::settings::{SETTINGS_BUTTON_ACTIVE, SETTINGS_ICON_ADD, SETTINGS_ICON_REMOVE};
use style::widgets::DEFAULT_TOAST;
use style::window::{
    WINDOW_FILTER_NAME_INPUT, WINDOW_FILTER_NAME_INPUT_PADDING, WINDOW_FILTER_SMALL_ITEM,
    WINDOW_FILTER_SMALL_REMOVE, WINDOW_FILTER_SMALL_REMOVE_RIGHT,
};
use style::{ColorId, ColorRef};
use ui::{
    BoxFrame, IconButton, InputEvent, InputField, RoundButton, SettingsButton, Userpic,
    UserpicSource, color, font, font_height,
};

use super::folder_chats_box::{self, Side};
use super::{Env, rows};
use crate::lang::{strings, tr};
use crate::px_of;

/// A chat type's name in the lists (`FilterChatsTypeName`).
pub(super) fn type_name(kind: ChatType) -> Key {
    match kind {
        ChatType::Contacts => key::lng_filters_type_contacts,
        ChatType::NonContacts => key::lng_filters_type_non_contacts,
        ChatType::Groups => key::lng_filters_type_groups,
        ChatType::Channels => key::lng_filters_type_channels,
        ChatType::Bots => key::lng_filters_type_bots,
        ChatType::Muted => key::lng_filters_type_no_muted,
        ChatType::Read => key::lng_filters_type_no_read,
        ChatType::Archived => key::lng_filters_type_no_archived,
    }
}

/// The type's userpic: its icon on a gradient (`PaintFilterChatsTypeIcon`).
pub(super) fn type_userpic(kind: ChatType, size: gpui::Pixels, cx: &App) -> gpui::Div {
    use style::window as icons;
    let (from, to, icon) = match kind {
        ChatType::Contacts => (
            ColorId::HistoryPeer4UserpicBg,
            ColorId::HistoryPeer4UserpicBg2,
            icons::WINDOW_FILTER_TYPE_CONTACTS,
        ),
        ChatType::NonContacts => (
            ColorId::HistoryPeer7UserpicBg,
            ColorId::HistoryPeer7UserpicBg2,
            icons::WINDOW_FILTER_TYPE_NON_CONTACTS,
        ),
        ChatType::Groups => (
            ColorId::HistoryPeer2UserpicBg,
            ColorId::HistoryPeer2UserpicBg2,
            icons::WINDOW_FILTER_TYPE_GROUPS,
        ),
        ChatType::Channels => (
            ColorId::HistoryPeer1UserpicBg,
            ColorId::HistoryPeer1UserpicBg2,
            icons::WINDOW_FILTER_TYPE_CHANNELS,
        ),
        ChatType::Bots => (
            ColorId::HistoryPeer6UserpicBg,
            ColorId::HistoryPeer6UserpicBg2,
            icons::WINDOW_FILTER_TYPE_BOTS,
        ),
        ChatType::Muted => (
            ColorId::HistoryPeer6UserpicBg,
            ColorId::HistoryPeer6UserpicBg2,
            icons::WINDOW_FILTER_TYPE_NO_MUTED,
        ),
        ChatType::Read => (
            ColorId::HistoryPeer7UserpicBg,
            ColorId::HistoryPeer7UserpicBg2,
            icons::WINDOW_FILTER_TYPE_NO_READ,
        ),
        ChatType::Archived => (
            ColorId::HistoryPeer4UserpicBg,
            ColorId::HistoryPeer4UserpicBg2,
            icons::WINDOW_FILTER_TYPE_NO_ARCHIVED,
        ),
    };
    div()
        .size(size)
        .rounded_full()
        .bg(linear_gradient(
            180.0,
            linear_color_stop(color(cx, ColorRef::Palette(from)), 0.0),
            linear_color_stop(color(cx, ColorRef::Palette(to)), 1.0),
        ))
        .flex()
        .items_center()
        .justify_center()
        .child(ui::icon(icon))
}

/// A chat's name and userpic, Saved Messages as such.
pub(super) fn chat_look(env: &Env, chat: ChatId, cx: &App) -> Option<(String, UserpicSource)> {
    let state = env.targets.store.read(cx).state();
    let found = state.chats.chat(chat)?;
    let name = crate::recipients_box::name(state, strings(cx), chat);
    if Some(chat) == crate::recipients_box::saved_messages(state) {
        return Some((name, UserpicSource::SavedMessages));
    }
    let (userpic, _) = crate::dialogs::photo_userpic(
        &state.chats,
        found.photo.as_ref(),
        found.accent_color,
        &name,
    );
    Some((name, userpic))
}

pub(super) struct FolderBox {
    env: Env,
    /// None for a new folder.
    id: Option<FolderId>,
    /// None while TDLib is asked for the folder.
    rules: Option<FolderRules>,
    name: Entity<InputField>,
    _name: Subscription,
    _load: Option<Task<()>>,
}

impl Focusable for FolderBox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.name.focus_handle(cx)
    }
}

pub(super) fn show(env: &Env, id: Option<FolderId>, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx: &mut Context<FolderBox>| {
        let placeholder = tr(cx, key::lng_filters_new_name);
        let name = cx.new(|cx| {
            InputField::new(&WINDOW_FILTER_NAME_INPUT, placeholder, window, cx)
                .with_max_length(NAME_MAX)
        });
        let subscription = cx.subscribe_in(
            &name,
            window,
            |this: &mut FolderBox, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Submitted) {
                    this.save(window, cx);
                }
            },
        );
        let load = id.map(|id| {
            let task =
                crate::store::spawn(env.targets.store.read(cx).session().folder_rules(id), cx);
            cx.spawn_in(window, async move |this, cx| match task.await {
                Ok(rules) => {
                    let _ = this.update(cx, |this, cx| {
                        let text = rules.name().to_owned();
                        this.name
                            .update(cx, |field, cx| field.set_text(&text, None, cx));
                        this.rules = Some(rules);
                        cx.notify();
                    });
                }
                Err(error) => tracing::warn!(%error, "reading the folder failed"),
            })
        });
        FolderBox {
            env: env.clone(),
            id,
            rules: id.is_none().then(FolderRules::new),
            name,
            _name: subscription,
            _load: load,
        }
    });
    let field = view.read(cx).name.clone();
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
    crate::focus(&field, window, cx);
}

impl FolderBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
    }

    fn toast(&self, text: Key, cx: &mut App) {
        let text = tr(cx, text);
        self.env.targets.layers.update(cx, |layers, cx| {
            layers.show_toast(text, &DEFAULT_TOAST, crate::toast::DEFAULT_DURATION, cx);
        });
    }

    /// Checks the folder as tdesktop's `collect` does, then sends it.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut rules) = self.rules.clone() else {
            return;
        };
        let name = self.name.read(cx).text().trim().to_owned();
        if name.is_empty() || name.chars().count() > NAME_MAX {
            self.name.update(cx, |field, cx| field.show_error(cx));
            return;
        }
        if rules.is_empty() {
            self.toast(key::lng_filters_empty, cx);
            return;
        }
        if rules.is_all_chats() {
            self.toast(key::lng_filters_default, cx);
            return;
        }
        rules.set_name(&name);
        let session = self.env.targets.store.read(cx).session().clone();
        match self.id {
            Some(id) => crate::peer_menu::send(session.edit_folder(id, &rules), "edit folder", cx),
            None => {
                let task = crate::store::spawn(session.create_folder(&rules), cx);
                cx.spawn(async move |_, _| {
                    if let Err(error) = task.await {
                        tracing::warn!(%error, "creating a folder failed");
                    }
                })
                .detach();
            }
        }
        self.close(window, cx);
    }

    fn change(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut FolderRules)) {
        if let Some(rules) = self.rules.as_mut() {
            edit(rules);
            cx.notify();
        }
    }

    /// A small row with a cross (`FilterChatsPreview`).
    fn preview_row(
        id: (&'static str, usize),
        userpic: AnyElement,
        name: SharedString,
        on_remove: impl Fn(&mut Window, &mut App) + 'static,
        cx: &App,
    ) -> AnyElement {
        let st = &WINDOW_FILTER_SMALL_ITEM;
        let name_font = st.name_style.font;
        let remove_top = (st.height.0 - WINDOW_FILTER_SMALL_REMOVE.height.0) / 2;
        div()
            .relative()
            .h(px_of(st.height))
            .child(
                div()
                    .absolute()
                    .left(px_of(st.photo_position.x))
                    .top(px_of(st.photo_position.y))
                    .child(userpic),
            )
            .child(
                div()
                    .absolute()
                    .left(px_of(st.name_position.x))
                    .top(px_of(st.name_position.y))
                    .right(px_of(style::Px(60)))
                    .font(font(name_font))
                    .text_size(px_of(name_font.size))
                    .line_height(font_height(name_font))
                    .text_color(color(cx, st.name_fg))
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(name),
            )
            .child(
                div()
                    .absolute()
                    .right(px_of(WINDOW_FILTER_SMALL_REMOVE_RIGHT))
                    .top(px_of(style::Px(remove_top)))
                    .child(
                        IconButton::new(id, &WINDOW_FILTER_SMALL_REMOVE)
                            .on_click(move |_, window, cx| on_remove(window, cx)),
                    ),
            )
            .into_any_element()
    }

    /// The types and chats of one side, each with its cross.
    fn preview(&self, side: Side, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(rules) = &self.rules else {
            return Vec::new();
        };
        let size = px_of(WINDOW_FILTER_SMALL_ITEM.photo_size);
        let types = side.types().iter().copied().filter(|kind| rules.has(*kind));
        let mut items: Vec<AnyElement> = types
            .enumerate()
            .map(|(index, kind)| {
                let weak = cx.weak_entity();
                Self::preview_row(
                    (side.id("type"), index),
                    type_userpic(kind, size, cx).into_any_element(),
                    tr(cx, type_name(kind)),
                    move |_, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.change(cx, |rules| rules.set(kind, false));
                        });
                    },
                    cx,
                )
            })
            .collect();
        let chats = side.chats(rules);
        items.extend(chats.into_iter().enumerate().filter_map(|(index, chat)| {
            let (name, userpic) = chat_look(&self.env, chat, cx)?;
            let weak = cx.weak_entity();
            Some(Self::preview_row(
                (side.id("chat"), index),
                Userpic::new(size, userpic).into_any_element(),
                name.into(),
                move |_, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.change(cx, |rules| rules.remove_chat(chat));
                    });
                },
                cx,
            ))
        }));
        items
    }

    /// "Add Chats" or "Exclude Chats", opening the chooser.
    fn side_button(&self, side: Side, cx: &mut Context<Self>) -> SettingsButton {
        let (text, icon) = match side {
            Side::Include => (key::lng_filters_add_chats, SETTINGS_ICON_ADD),
            Side::Exclude => (key::lng_filters_remove_chats, SETTINGS_ICON_REMOVE),
        };
        let weak = cx.weak_entity();
        let env = self.env.clone();
        SettingsButton::new(side.id("add"), &SETTINGS_BUTTON_ACTIVE, tr(cx, text))
            .icon_round(icon, ColorRef::Palette(ColorId::WindowBgActive))
            .on_click(move |_, window, cx| {
                let Some(rules) = weak.upgrade().and_then(|this| this.read(cx).rules.clone())
                else {
                    return;
                };
                let weak = weak.clone();
                let on_saved: folder_chats_box::OnSaved = Rc::new(move |rules, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.rules = Some(rules);
                        cx.notify();
                    });
                });
                folder_chats_box::show(&env, side, rules, on_saved, window, cx);
            })
    }

    fn content(&self, cx: &mut Context<Self>) -> gpui::Div {
        let padding = WINDOW_FILTER_NAME_INPUT_PADDING;
        let is_shareable = self.rules.as_ref().is_some_and(FolderRules::is_shareable);
        let mut column = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .pl(px_of(padding.right))
                    .pr(px_of(padding.right))
                    .pb(px_of(padding.bottom))
                    .child(self.name.clone()),
            )
            .child(rows::skip())
            .child(rows::divider(cx))
            .child(rows::skip())
            .child(rows::subtitle(tr(cx, key::lng_filters_include), cx))
            .child(self.side_button(Side::Include, cx))
            .children(self.preview(Side::Include, cx))
            .child(rows::skip())
            .child(rows::divider_text(
                tr(cx, key::lng_filters_include_about),
                cx,
            ))
            .child(rows::skip());
        if !is_shareable {
            column = column
                .child(rows::subtitle(tr(cx, key::lng_filters_exclude), cx))
                .child(self.side_button(Side::Exclude, cx))
                .children(self.preview(Side::Exclude, cx))
                .child(rows::skip())
                .child(rows::divider_text(
                    tr(cx, key::lng_filters_exclude_about),
                    cx,
                ))
                .child(rows::skip());
        }
        column
    }
}

impl Render for FolderBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, button) = if self.id.is_some() {
            (key::lng_filters_edit, key::lng_settings_save)
        } else {
            (key::lng_filters_new, key::lng_filters_create_button)
        };
        let (save, cancel) = (cx.weak_entity(), cx.weak_entity());
        BoxFrame::new("folder-box", &DEFAULT_BOX)
            .width(px_of(BOX_WIDE_WIDTH))
            .title(tr(cx, title))
            .row(self.content(cx))
            .button(
                RoundButton::new("folder-save", &DEFAULT_BOX.button, tr(cx, button)).on_click(
                    move |_, window, cx| {
                        let _ = save.update(cx, |this, cx| this.save(window, cx));
                    },
                ),
            )
            .button(
                RoundButton::new(
                    "folder-cancel",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_cancel),
                )
                .on_click(move |_, window, cx| {
                    let _ = cancel.update(cx, |this, cx| this.close(window, cx));
                }),
            )
    }
}
