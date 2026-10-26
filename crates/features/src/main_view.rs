//! The signed-in window (`MainWidget`): the chat list column on the left and
//! the chat on the right.

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding,
    ParentElement, Render, Styled, Subscription, Window, actions, div, prelude::*, px,
};
use i18n::key;
use model::auth;
use model::chat::ChatList;
use style::chat::{HISTORY_SAVED_FONT, MSG_PADDING, MSG_SERVICE_FONT, MSG_SERVICE_PADDING};
use style::dialogs::{
    DIALOGS_CANCEL_SEARCH, DIALOGS_FILTER, DIALOGS_FILTER_PADDING, DIALOGS_FILTER_SKIP,
    DIALOGS_MENU_TOGGLE,
};
use style::info::{HISTORY_TOP_BAR_BACK, TOP_BAR_HEIGHT};
use style::window::{COLUMN_MAXIMAL_WIDTH_LEFT, COLUMN_MINIMAL_WIDTH_LEFT};
use style::window_main_menu::MAIN_MENU_WIDTH;
use style::{ColorId, ColorRef};
use ui::{CrossButton, IconButton, InputEvent, InputField, color, font, font_height};

use crate::boxes;
use crate::dialogs::Dialogs;
use crate::lang::tr;
use crate::main_menu::{MainMenu, MenuAction};
use crate::{Layers, Store, px_of};

/// tdesktop's default share of the window for the chat list
/// (`kDefaultDialogsWidthRatio`).
const DIALOGS_WIDTH_RATIO: f32 = 5.0 / 14.0;

actions!(main_view, [CancelSearch]);

const CONTEXT: &str = "MainView";

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CancelSearch, Some(CONTEXT))]);
}

pub struct MainView {
    store: Entity<Store>,
    layers: Entity<Layers>,
    dialogs: Entity<Dialogs>,
    search: Entity<InputField>,
    focus: FocusHandle,
    _subscriptions: [Subscription; 2],
}

impl std::fmt::Debug for MainView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainView").finish_non_exhaustive()
    }
}

impl MainView {
    pub fn new(
        store: Entity<Store>,
        layers: Entity<Layers>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let dialogs = cx.new(|cx| Dialogs::new(store.clone(), layers.clone(), cx));
        let search =
            cx.new(|cx| InputField::new(&DIALOGS_FILTER, tr(cx, key::lng_dlg_filter), window, cx));
        let subscription = cx.subscribe(&search, |this, search, event, cx| {
            if *event == InputEvent::Changed {
                let query = search.read(cx).text().to_owned();
                this.dialogs
                    .update(cx, |dialogs, cx| dialogs.set_query(&query, cx));
                cx.notify();
            }
        });
        let observe_dialogs = cx.observe(&dialogs, |_, _, cx| cx.notify());
        Self {
            store,
            layers,
            dialogs,
            search,
            focus: cx.focus_handle(),
            _subscriptions: [subscription, observe_dialogs],
        }
    }

    /// Escape: clears the search, else leaves the archive.
    fn cancel_search(&mut self, _: &CancelSearch, _: &mut Window, cx: &mut Context<Self>) {
        if self.search.read(cx).text().is_empty() {
            self.dialogs
                .update(cx, |dialogs, cx| dialogs.open_list(ChatList::Main, cx));
        } else {
            self.clear_search(cx);
        }
    }

    fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.search
            .update(cx, |search, cx| search.set_text("", None, cx));
        self.dialogs
            .update(cx, |dialogs, cx| dialogs.set_query("", cx));
        cx.notify();
    }

    fn open_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let menu = cx.new(|cx| MainMenu::new(self.store.clone(), cx));
        cx.subscribe_in(&menu, window, |this, _, action, window, cx| {
            this.layers
                .update(cx, |layers, cx| layers.close(window, cx));
            match action {
                MenuAction::Close => {}
                MenuAction::SavedMessages => {
                    let me = this.store.read(cx).state().me;
                    if let Some(me) = me {
                        // A private chat's id is its user's id.
                        let chat = model::ChatId(me.0);
                        this.dialogs
                            .update(cx, |dialogs, cx| dialogs.select(chat, cx));
                    }
                }
                // shortcut: Settings asks to log out until the settings screen lands in M7.
                MenuAction::Settings => this.confirm_logout(window, cx),
            }
        })
        .detach();
        self.layers.update(cx, |layers, cx| {
            layers.show_panel(menu, px_of(MAIN_MENU_WIDTH), window, cx);
        });
    }

    fn confirm_logout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let action: boxes::Callback = Rc::new(move |_, cx| {
            let task = store.read(cx).auth(auth::Action::LogOut, cx);
            cx.spawn(async move |_| {
                if let Err(error) = task.await {
                    tracing::warn!(%error, "log out failed");
                }
            })
            .detach();
        });
        boxes::confirm_attention(
            &self.layers,
            tr(cx, key::lng_sure_logout).to_string(),
            tr(cx, key::lng_settings_logout),
            action,
            window,
            cx,
        );
    }

    /// A folder's back button and name in place of the search
    /// (`TopBarWidget` over a folder).
    // shortcut: no search inside the archive yet; it comes with search in one chat list later in M2.
    fn folder_bar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let dialogs = self.dialogs.clone();
        div()
            .flex_none()
            .flex()
            .items_center()
            .h(px_of(TOP_BAR_HEIGHT))
            .child(
                IconButton::new("folder-back", &HISTORY_TOP_BAR_BACK).on_click(move |_, _, cx| {
                    dialogs.update(cx, |dialogs, cx| dialogs.open_list(ChatList::Main, cx));
                }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font(font(HISTORY_SAVED_FONT))
                    .text_size(px_of(HISTORY_SAVED_FONT.size))
                    .line_height(font_height(HISTORY_SAVED_FONT))
                    .text_color(color(cx, ColorRef::Palette(ColorId::DialogsNameFg)))
                    .child(tr(cx, key::lng_archived_name)),
            )
            .into_any_element()
    }

    /// The menu button and the search field (`Dialogs::Widget` top).
    fn top_bar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if self.dialogs.read(cx).list() == ChatList::Archive {
            return self.folder_bar(cx);
        }
        let entity = cx.entity();
        let padding = px_of(DIALOGS_FILTER_PADDING.x);
        let field_top = (px_of(TOP_BAR_HEIGHT) - px_of(DIALOGS_FILTER.height_min)) / 2.0;
        let is_searching = !self.search.read(cx).text().is_empty();
        let clear = cx.entity();
        div()
            .flex_none()
            .flex()
            .h(px_of(TOP_BAR_HEIGHT))
            .pl(padding)
            .pr(px_of(DIALOGS_FILTER_SKIP) + padding)
            .child(div().pt(px_of(DIALOGS_FILTER_PADDING.y)).child(
                IconButton::new("main-menu", &DIALOGS_MENU_TOGGLE).on_click(
                    move |_, window, cx| {
                        entity.update(cx, |this, cx| this.open_menu(window, cx));
                    },
                ),
            ))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .ml(padding)
                    .mt(field_top)
                    .child(self.search.clone())
                    .when(is_searching, |this| {
                        this.child(div().absolute().top_0().right_0().child(
                            CrossButton::new("cancel-search", &DIALOGS_CANCEL_SEARCH).on_click(
                                move |_, _, cx| {
                                    clear.update(cx, Self::clear_search);
                                },
                            ),
                        ))
                    }),
            )
            .into_any_element()
    }

    /// "Select a chat to start messaging" in a service bubble.
    fn empty_history(cx: &App) -> impl IntoElement {
        let height = font_height(MSG_SERVICE_FONT)
            + px_of(MSG_SERVICE_PADDING.top)
            + px_of(MSG_SERVICE_PADDING.bottom);
        // shortcut: a plain color instead of the chat wallpaper, which comes with the history in M3.
        div()
            .flex_1()
            .h_full()
            .bg(color(cx, ColorRef::Palette(ColorId::WindowBgOver)))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .h(height)
                    .pl(px_of(MSG_PADDING.left))
                    .pr(px_of(MSG_PADDING.right))
                    .pt(px_of(MSG_SERVICE_PADDING.top))
                    .rounded(height / 2.0)
                    .bg(color(cx, ColorRef::Palette(ColorId::MsgServiceBg)))
                    .font(font(MSG_SERVICE_FONT))
                    .text_size(px_of(MSG_SERVICE_FONT.size))
                    .line_height(font_height(MSG_SERVICE_FONT))
                    .text_color(color(cx, ColorRef::Palette(ColorId::MsgServiceFg)))
                    .child(tr(cx, key::lng_willbe_history)),
            )
    }
}

impl Focusable for MainView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = window.viewport_size().width;
        let column = (width * DIALOGS_WIDTH_RATIO)
            .max(px_of(COLUMN_MINIMAL_WIDTH_LEFT))
            .min(px_of(COLUMN_MAXIMAL_WIDTH_LEFT));
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::cancel_search))
            .size_full()
            .flex()
            .child(
                div()
                    .w(column)
                    .h_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .bg(color(cx, ColorRef::Palette(ColorId::DialogsBg)))
                    .child(self.top_bar(cx))
                    .child(div().flex_1().min_h_0().child(self.dialogs.clone())),
            )
            .child(
                div()
                    .w(px(1.0))
                    .h_full()
                    .bg(color(cx, ColorRef::Palette(ColorId::ShadowFg))),
            )
            .child(Self::empty_history(cx))
    }
}
