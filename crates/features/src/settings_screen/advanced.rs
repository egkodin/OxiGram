//! "Advanced" (`Settings::Advanced`): connection type, local storage and
//! downloads.
// shortcut: no auto-download, download path, window title, tray / Dock, spellchecker, data export, OpenGL or updates rows; with each one's step.

use gpui::{
    App, Context, IntoElement, ParentElement, Render, Styled, Subscription, Task, Window, div,
};
use i18n::key;
use model::ConnectionState;
use model::proxies::{ProxyKind, SavedProxy};
use style::menu_icons::{MENU_ICON_DOWNLOAD, MENU_ICON_NETWORK, MENU_ICON_STORAGE};

use super::{Env, PageId, proxies_box, rows};
use crate::dialogs::OpenDownloads;
use crate::lang::{tr, tr_with};

pub(super) struct AdvancedPage {
    env: Env,
    /// The proxy in use, None for a direct connection.
    proxy: Option<ProxyKind>,
    /// What the label last saw, to read the proxies again when it changes.
    seen: ConnectionState,
    _load: Task<()>,
    _store: Subscription,
}

impl AdvancedPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        // Turning a proxy on or off reconnects, so a new state means a new list.
        let store = cx.observe(&env.targets.store, |this, store, cx| {
            let state = store.read(cx).state().connection;
            if state != this.seen {
                this.seen = state;
                this.load(cx);
            }
            cx.notify();
        });
        let seen = env.targets.store.read(cx).state().connection;
        let mut this = Self {
            env,
            proxy: None,
            seen,
            _load: Task::ready(()),
            _store: store,
        };
        this.load(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let task = crate::store::spawn(self.env.targets.store.read(cx).session().proxies(), cx);
        self._load = cx.spawn(async move |this, cx| match task.await {
            Ok(proxies) => {
                let _ = this.update(cx, |this, cx| {
                    this.proxy = enabled(proxies);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "reading the proxies failed"),
        });
    }

    /// The "Connection type" value (`ConnectionTypeLabel`).
    fn connection(&self, cx: &App) -> gpui::SharedString {
        let state = self.env.targets.store.read(cx).state().connection;
        let is_connected = matches!(state, ConnectionState::Ready | ConnectionState::Updating);
        let transport = match self.proxy {
            Some(ProxyKind::Http { .. }) => "HTTP",
            _ => "TCP",
        };
        match (&self.proxy, is_connected) {
            (None, true) => {
                tr_with(cx, key::lng_connection_auto, &[("transport", transport)]).into()
            }
            (None, false) => tr(cx, key::lng_connection_auto_connecting),
            (Some(_), true) => {
                tr_with(cx, key::lng_connection_proxy, &[("transport", transport)]).into()
            }
            (Some(_), false) => tr(cx, key::lng_connection_proxy_connecting),
        }
    }
}

fn enabled(proxies: Vec<SavedProxy>) -> Option<ProxyKind> {
    proxies
        .into_iter()
        .find(|saved| saved.is_enabled)
        .map(|saved| saved.proxy.kind)
}

impl Render for AdvancedPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let env = self.env.clone();
        let connection = rows::button(
            "advanced-connection",
            tr(cx, key::lng_settings_connection_type),
            MENU_ICON_NETWORK,
        )
        .label(self.connection(cx))
        .on_click(move |_, window, cx| proxies_box::show(&env, window, cx));
        let env = self.env.clone();
        let storage = rows::button(
            "advanced-storage",
            tr(cx, key::lng_settings_manage_local_storage),
            MENU_ICON_STORAGE,
        )
        .on_click(move |_, window, cx| env.open(PageId::Storage, window, cx));
        let env = self.env.clone();
        let downloads = rows::button(
            "advanced-downloads",
            tr(cx, key::lng_downloads_section),
            MENU_ICON_DOWNLOAD,
        )
        .on_click(move |_, window, cx| {
            env.targets
                .layers
                .update(cx, |layers, cx| layers.close_all(window, cx));
            env.targets
                .dialogs
                .update(cx, |_, cx| cx.emit(OpenDownloads));
        });
        div()
            .flex()
            .flex_col()
            .child(rows::skip())
            .child(rows::subtitle(tr(cx, key::lng_settings_data_storage), cx))
            .child(connection)
            .child(storage)
            .child(downloads)
            .child(rows::skip())
    }
}
