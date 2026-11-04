//! "Proxy settings" (`ProxiesBox`): IPv6, no proxy or a saved one, the
//! list with each proxy's state, adding, editing and deleting.
// shortcut: no "Use system proxy settings" (TDLib has none), no proxy for calls or auto-switch, no import from the clipboard, sharing or "Delete all"; deleting is at once, without tdesktop's restore. With row "Proxy settings".

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, DismissEvent, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement, Pixels, Point, Render,
    StatefulInteractiveElement as _, Styled, Subscription, Task, Window, anchored, deferred, div,
    px,
};
use i18n::key;
use model::ConnectionState;
use model::proxies::{Proxy, ProxyId, SavedProxy};
use style::connection_box::{
    PROXY_ROW_ICON_SKIP, PROXY_ROW_PADDING, PROXY_ROW_SKIP, PROXY_ROW_STATUS_FG,
    PROXY_ROW_STATUS_FG_AVAILABLE, PROXY_ROW_STATUS_FG_OFFLINE, PROXY_ROW_STATUS_FG_ONLINE,
    PROXY_ROW_TITLE_FG, PROXY_TRY_IPV6_PADDING, PROXY_USE_PADDING,
};
use style::info::TOP_BAR_MENU_TOGGLE;
use style::layers::{BOX_WIDE_WIDTH, DEFAULT_BOX, DEFAULT_BOX_CHECKBOX};
use style::menu_icons::{MENU_ICON_DELETE, MENU_ICON_EDIT};
use style::widgets::{DEFAULT_CHECK, DEFAULT_RADIO, POPUP_MENU_WITH_ICONS};
use style::{ColorId, ColorRef, Font, FontFlags};
use ui::{
    BoxFrame, Checkbox, Checked, IconButton, MenuTone, PopupMenu, RoundButton, color, font,
    font_height,
};

use super::{Env, proxy_box, rows};
use crate::lang::{tr, tr_with};
use crate::px_of;

/// TDLib's switch for trying IPv6 first.
const PREFER_IPV6: &str = "prefer_ipv6";
const MENU_EDGE: f32 = 8.0;
const SEMIBOLD: Font = Font::new(13, FontFlags::BOLD, "");
const NORMAL: Font = Font::new(13, FontFlags::NONE, "");

#[derive(Debug, Clone, Copy, PartialEq)]
enum Ping {
    Checking,
    Available(u32),
    Unavailable,
}

pub(super) struct ProxiesBox {
    env: Env,
    /// None while TDLib is asked.
    proxies: Option<Vec<SavedProxy>>,
    pings: HashMap<ProxyId, Ping>,
    prefer_ipv6: Option<bool>,
    menu: Option<(Point<Pixels>, Entity<PopupMenu>, Subscription)>,
    focus: FocusHandle,
    _tasks: Vec<Task<()>>,
    _store: Subscription,
}

impl Focusable for ProxiesBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub(super) fn show(env: &Env, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx: &mut Context<ProxiesBox>| {
        let session = env.targets.store.read(cx).session().clone();
        let ipv6 = crate::store::spawn(session.bool_option(PREFER_IPV6), cx);
        let ipv6 = cx.spawn(async move |this, cx| match ipv6.await {
            Ok(value) => {
                let _ = this.update(cx, |this, cx| {
                    this.prefer_ipv6 = Some(value);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "reading prefer_ipv6 failed"),
        });
        let store = cx.observe(&env.targets.store, |_, _, cx| cx.notify());
        let mut this = ProxiesBox {
            env: env.clone(),
            proxies: None,
            pings: HashMap::new(),
            prefer_ipv6: None,
            menu: None,
            focus: cx.focus_handle(),
            _tasks: vec![ipv6],
            _store: store,
        };
        this.reload(cx);
        this
    });
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
}

impl ProxiesBox {
    fn session(&self, cx: &App) -> model::Session {
        self.env.targets.store.read(cx).session().clone()
    }

    /// Asks TDLib for the list again, then pings each proxy.
    fn reload(&mut self, cx: &mut Context<Self>) {
        let task = crate::store::spawn(self.session(cx).proxies(), cx);
        let load = cx.spawn(async move |this, cx| match task.await {
            Ok(proxies) => {
                let _ = this.update(cx, |this, cx| {
                    for saved in &proxies {
                        this.ping(saved.id, &saved.proxy, cx);
                    }
                    this.proxies = Some(proxies);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "reading the proxies failed"),
        });
        self._tasks.push(load);
    }

    fn ping(&mut self, id: ProxyId, proxy: &Proxy, cx: &mut Context<Self>) {
        self.pings.insert(id, Ping::Checking);
        let task = crate::store::spawn(self.session(cx).ping_proxy(proxy), cx);
        let ping = cx.spawn(async move |this, cx| {
            let result = match task.await {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a ping is a few seconds"
                )]
                Ok(seconds) => Ping::Available((seconds * 1000.0).round() as u32),
                Err(_) => Ping::Unavailable,
            };
            let _ = this.update(cx, |this, cx| {
                this.pings.insert(id, result);
                cx.notify();
            });
        });
        self._tasks.push(ping);
    }

    /// Runs `request`, then reloads the list.
    fn apply<F>(&mut self, request: F, what: &'static str, cx: &mut Context<Self>)
    where
        F: std::future::Future<Output = Result<(), model::RequestError>> + Send + 'static,
    {
        let task = crate::store::spawn(request, cx);
        let done = cx.spawn(async move |this, cx| {
            if let Err(error) = task.await {
                tracing::warn!(%error, what, "proxy change failed");
            }
            let _ = this.update(cx, |this, cx| this.reload(cx));
        });
        self._tasks.push(done);
    }

    fn edit(&self, saved: Option<SavedProxy>, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        let id = saved.as_ref().map(|saved| saved.id);
        let on_saved: proxy_box::OnSaved = Rc::new(move |proxy, _, cx| {
            let _ = weak.update(cx, |this, cx| {
                let session = this.session(cx);
                match id {
                    Some(id) => this.apply(session.edit_proxy(id, &proxy), "edit", cx),
                    None => this.apply(session.add_proxy(&proxy), "add", cx),
                }
            });
        });
        proxy_box::show(
            &self.env,
            saved.map(|saved| saved.proxy),
            on_saved,
            window,
            cx,
        );
    }

    /// "Use custom proxy": the first saved one, or a new one.
    fn use_custom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let first = self
            .proxies
            .as_ref()
            .and_then(|list| list.first())
            .map(|saved| saved.id);
        match first {
            Some(id) => {
                let task = self.session(cx).enable_proxy(id);
                self.apply(task, "enable", cx);
            }
            None => self.edit(None, window, cx),
        }
    }

    fn show_menu(&mut self, saved: SavedProxy, window: &mut Window, cx: &mut Context<Self>) {
        let (edit, delete) = (cx.weak_entity(), cx.weak_entity());
        let id = saved.id;
        let menu = cx.new(|cx| {
            let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
            menu.action(
                tr(cx, key::lng_proxy_menu_edit),
                MENU_ICON_EDIT,
                MenuTone::Normal,
                move |window, cx| {
                    let saved = saved.clone();
                    let _ = edit.update(cx, |this, cx| this.edit(Some(saved), window, cx));
                },
            );
            menu.action(
                tr(cx, key::lng_proxy_menu_delete),
                MENU_ICON_DELETE,
                MenuTone::Normal,
                move |_, cx| {
                    let _ = delete.update(cx, |this, cx| {
                        let task = this.session(cx).remove_proxy(id);
                        this.apply(task, "remove", cx);
                    });
                },
            );
            menu
        });
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.menu = None;
            cx.notify();
        });
        menu.read(cx).focus_handle(cx).focus(window, cx);
        self.menu = Some((window.mouse_position(), menu, subscription));
        cx.notify();
    }

    fn status(&self, saved: &SavedProxy, cx: &App) -> (gpui::SharedString, ColorRef) {
        if saved.is_enabled {
            let state = self.env.targets.store.read(cx).state().connection;
            return if state == ConnectionState::Ready || state == ConnectionState::Updating {
                (tr(cx, key::lng_proxy_online), PROXY_ROW_STATUS_FG_ONLINE)
            } else {
                (tr(cx, key::lng_proxy_connecting), PROXY_ROW_STATUS_FG)
            };
        }
        match self.pings.get(&saved.id) {
            Some(Ping::Available(ms)) => (
                tr_with(cx, key::lng_proxy_available, &[("ping", &ms.to_string())]).into(),
                PROXY_ROW_STATUS_FG_AVAILABLE,
            ),
            Some(Ping::Unavailable) => (
                tr(cx, key::lng_proxy_unavailable),
                PROXY_ROW_STATUS_FG_OFFLINE,
            ),
            Some(Ping::Checking) | None => (tr(cx, key::lng_proxy_checking), PROXY_ROW_STATUS_FG),
        }
    }

    /// A proxy's row (`ProxyRow`): the radio, address, state and menu.
    fn row(&self, index: usize, saved: &SavedProxy, cx: &mut Context<Self>) -> AnyElement {
        let padding = PROXY_ROW_PADDING;
        let (status, status_fg) = self.status(saved, cx);
        let title = format!("{}:{}", saved.proxy.server, saved.proxy.port);
        let checked = if saved.is_enabled {
            Checked::Yes
        } else {
            Checked::No
        };
        let id = saved.id;
        let (pick, menu) = (cx.weak_entity(), cx.weak_entity());
        let for_menu = saved.clone();
        let line = |text: gpui::SharedString, font_style: Font, fg: ColorRef| {
            div()
                .font(font(font_style))
                .text_size(px_of(font_style.size))
                .line_height(font_height(font_style))
                .text_color(color(cx, fg))
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(text)
        };
        div()
            .id(("proxy-row", index))
            .relative()
            .pl(px_of(padding.left))
            .pr(px_of(padding.right))
            .pt(px_of(padding.top))
            .pb(px_of(padding.bottom))
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|this| this.bg(color(cx, ColorRef::Palette(ColorId::WindowBgOver))))
            .on_click(move |_, _, cx| {
                let _ = pick.update(cx, |this, cx| {
                    let task = this.session(cx).enable_proxy(id);
                    this.apply(task, "enable", cx);
                });
            })
            .child(div().w(px_of(PROXY_ROW_ICON_SKIP)).child(Checkbox::radio(
                ("proxy-radio", index),
                &DEFAULT_BOX_CHECKBOX,
                &DEFAULT_RADIO,
                "",
                checked,
            )))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px_of(PROXY_ROW_SKIP))
                    .child(line(title.into(), SEMIBOLD, PROXY_ROW_TITLE_FG))
                    .child(line(status, NORMAL, status_fg)),
            )
            .child(
                IconButton::new(("proxy-menu", index), &TOP_BAR_MENU_TOGGLE).on_click(
                    move |_, window, cx| {
                        cx.stop_propagation();
                        let saved = for_menu.clone();
                        let _ = menu.update(cx, |this, cx| this.show_menu(saved, window, cx));
                    },
                ),
            )
            .into_any_element()
    }

    fn content(&self, cx: &mut Context<Self>) -> gpui::Div {
        let ipv6 = PROXY_TRY_IPV6_PADDING;
        let using = PROXY_USE_PADDING;
        let is_custom = self
            .proxies
            .as_ref()
            .is_some_and(|list| list.iter().any(|saved| saved.is_enabled));
        let ipv6_on = self.prefer_ipv6.unwrap_or(false);
        let toggle_ipv6 = cx.weak_entity();
        let padded = |margins: style::Margins| {
            div()
                .pl(px_of(margins.left))
                .pr(px_of(margins.right))
                .pt(px_of(margins.top))
                .pb(px_of(margins.bottom))
        };
        let radio = |id: usize, text: i18n::Key, is_on: bool, cx: &mut Context<Self>| {
            let weak = cx.weak_entity();
            padded(using).child(
                Checkbox::radio(
                    ("proxy-use", id),
                    &DEFAULT_BOX_CHECKBOX,
                    &DEFAULT_RADIO,
                    tr(cx, text),
                    if is_on { Checked::Yes } else { Checked::No },
                )
                .on_toggle(move |window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if id == 0 {
                            let task = this.session(cx).disable_proxy();
                            this.apply(task, "disable", cx);
                        } else {
                            this.use_custom(window, cx);
                        }
                    });
                }),
            )
        };
        let rows_list: Vec<_> = self
            .proxies
            .clone()
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(index, saved)| self.row(index, saved, cx))
            .collect();
        div()
            .flex()
            .flex_col()
            .child(
                padded(ipv6).child(
                    Checkbox::new(
                        "proxy-ipv6",
                        &DEFAULT_BOX_CHECKBOX,
                        &DEFAULT_CHECK,
                        tr(cx, key::lng_connection_try_ipv6),
                        if ipv6_on { Checked::Yes } else { Checked::No },
                    )
                    .on_toggle(move |_, cx| {
                        let _ = toggle_ipv6.update(cx, |this, cx| {
                            this.prefer_ipv6 = Some(!ipv6_on);
                            let task = this.session(cx).set_bool_option(PREFER_IPV6, !ipv6_on);
                            crate::peer_menu::send(task, "prefer IPv6", cx);
                            cx.notify();
                        });
                    }),
                ),
            )
            .child(radio(0, key::lng_proxy_disable, !is_custom, cx))
            .child(radio(1, key::lng_proxy_use_custom, is_custom, cx))
            .child(rows::skip())
            .child(rows::divider_text(tr(cx, key::lng_proxy_about), cx))
            .children(rows_list)
            .child(rows::skip())
    }
}

impl Render for ProxiesBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (add, close) = (cx.weak_entity(), cx.weak_entity());
        let menu = self.menu.as_ref().map(|(position, menu, _)| {
            deferred(
                anchored()
                    .anchor(gpui::Anchor::TopRight)
                    .position(*position)
                    .snap_to_window_with_margin(px(MENU_EDGE))
                    .child(div().child(menu.clone())),
            )
            .with_priority(1)
        });
        let frame = BoxFrame::new("proxies-box", &DEFAULT_BOX)
            .width(px_of(BOX_WIDE_WIDTH))
            .title(tr(cx, key::lng_proxy_settings))
            .row(self.content(cx))
            .button(
                RoundButton::new(
                    "proxies-add",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_proxy_add),
                )
                .on_click(move |_, window, cx| {
                    let _ = add.update(cx, |this, cx| this.edit(None, window, cx));
                }),
            )
            .button(
                RoundButton::new("proxies-close", &DEFAULT_BOX.button, tr(cx, key::lng_close))
                    .on_click(move |_, window, cx| {
                        let _ = close.update(cx, |this, cx| {
                            this.env
                                .targets
                                .layers
                                .update(cx, |layers, cx| layers.close(window, cx));
                        });
                    }),
            );
        div().track_focus(&self.focus).child(frame).children(menu)
    }
}
