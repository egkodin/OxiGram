//! "Active sessions" (`Settings::Sessions`): this device, terminating the
//! others, incomplete logins, the other devices and how long an unused
//! session lives.
// shortcut: a renamed device shows its new name after a restart, when TDLib reads it; tdesktop re-sends it at once.

use std::rc::Rc;

use chrono::Local;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Render, SharedString, Styled, Task,
    Window, div, linear_color_stop, linear_gradient, prelude::*,
};
use i18n::key;
use model::sessions::{ActiveSession, ActiveSessions, Device, SessionId};
use style::info::{INFO_BLOCK_BUTTON, INFO_ICON_BLOCK};
use style::layers::DEFAULT_SUBSECTION_TITLE_PADDING;
use style::settings::{
    SESSION_INFO_FG, SESSION_LIST_ITEM, SESSION_SUBTITLE_SKIP, SESSION_TERMINATE,
    SESSION_TERMINATE_SKIP, SESSION_TERMINATE_TOP,
};
use style::settings_active_sessions as icons;
use style::widgets::DEFAULT_LINK_BUTTON;
use style::widgets::PeerListItem;
use style::{ColorId, ColorRef};
use ui::{IconButton, LinkButton, SettingsButton, color, font, font_height};

use super::{Env, rows, self_destruct_box, session_box};
use crate::boxes::Callback;
use crate::lang::{strings, tr};
use crate::px_of;

/// `sessionLocationTop`.
const LOCATION_TOP: style::Px = style::Px(54);
/// `sessionCurrentSkip`.
const CURRENT_SKIP: style::Px = style::Px(8);

/// `TypeIcon`.
pub(super) fn device_icon(device: Device) -> style::Icon {
    match device {
        Device::Windows => icons::SESSION_ICON_WINDOWS,
        Device::Mac => icons::SESSION_ICON_MAC,
        Device::Ubuntu => icons::SESSION_ICON_UBUNTU,
        Device::Linux => icons::SESSION_ICON_LINUX,
        Device::IPhone => style::settings::SESSION_ICONI_PHONE,
        Device::IPad => icons::SESSION_ICONI_PAD,
        Device::Android => style::settings::SESSION_ICON_ANDROID,
        Device::Web => icons::SESSION_ICON_WEB,
        Device::Chrome => icons::SESSION_ICON_CHROME,
        Device::Edge => icons::SESSION_ICON_EDGE,
        Device::Firefox => icons::SESSION_ICON_FIREFOX,
        Device::Safari => icons::SESSION_ICON_SAFARI,
        Device::Other => icons::SESSION_ICON_OTHER,
    }
}

/// `TypeColors`: the circle's gradient.
pub(super) fn device_colors(device: Device) -> (ColorId, ColorId) {
    match device {
        Device::Windows | Device::Mac | Device::Other => (
            ColorId::HistoryPeer4UserpicBg,
            ColorId::HistoryPeer4UserpicBg2,
        ),
        Device::Ubuntu => (
            ColorId::HistoryPeer8UserpicBg,
            ColorId::HistoryPeer8UserpicBg2,
        ),
        Device::Linux => (
            ColorId::HistoryPeer5UserpicBg,
            ColorId::HistoryPeer5UserpicBg2,
        ),
        Device::IPhone | Device::IPad => (
            ColorId::HistoryPeer7UserpicBg,
            ColorId::HistoryPeer7UserpicBg2,
        ),
        Device::Android => (
            ColorId::HistoryPeer2UserpicBg,
            ColorId::HistoryPeer2UserpicBg2,
        ),
        Device::Web | Device::Chrome | Device::Edge | Device::Firefox | Device::Safari => (
            ColorId::HistoryPeer6UserpicBg,
            ColorId::HistoryPeer6UserpicBg2,
        ),
    }
}

/// A row of the sessions' or websites' list: the cover, a name, a status,
/// the place and date, and the terminate button (`Row::elementsPaint`).
pub(super) struct ListRow {
    pub(super) id: (&'static str, u64),
    pub(super) st: &'static PeerListItem,
    pub(super) location_top: style::Px,
    pub(super) cover: AnyElement,
    pub(super) name: SharedString,
    pub(super) status: SharedString,
    pub(super) location: SharedString,
    pub(super) on_open: Callback,
    pub(super) on_terminate: Option<Callback>,
}

impl ListRow {
    pub(super) fn render(self, cx: &App) -> AnyElement {
        let st = self.st;
        let line = |top: style::Px, text: SharedString, fg: ColorRef, text_font: style::Font| {
            div()
                .absolute()
                .left(px_of(st.name_position.x))
                .top(px_of(top))
                .right(px_of(SESSION_TERMINATE.width) + px_of(SESSION_TERMINATE_SKIP))
                .font(font(text_font))
                .text_size(px_of(text_font.size))
                .line_height(font_height(text_font))
                .text_color(color(cx, fg))
                .truncate()
                .child(text)
        };
        let info_font = st.button.font;
        let on_open = self.on_open;
        div()
            .id(self.id)
            .relative()
            .h(px_of(st.height))
            .cursor_pointer()
            .hover(|this| this.bg(color(cx, st.button.text_bg_over)))
            .on_click(move |_, window, cx| on_open(window, cx))
            .child(
                div()
                    .absolute()
                    .left(px_of(st.photo_position.x))
                    .top(px_of(st.photo_position.y))
                    .child(self.cover),
            )
            .child(line(
                st.name_position.y,
                self.name,
                st.name_fg,
                st.name_style.font,
            ))
            .child(line(
                st.status_position.y,
                self.status,
                st.status_fg,
                info_font,
            ))
            .child(line(
                self.location_top,
                self.location,
                SESSION_INFO_FG,
                info_font,
            ))
            .children(self.on_terminate.map(|on_terminate| {
                div()
                    .absolute()
                    .right(px_of(SESSION_TERMINATE_SKIP))
                    .top(px_of(SESSION_TERMINATE_TOP))
                    .child(
                        IconButton::new((self.id.0, self.id.1), &SESSION_TERMINATE).on_click(
                            move |_, window, cx| {
                                cx.stop_propagation();
                                on_terminate(window, cx);
                            },
                        ),
                    )
            }))
            .into_any_element()
    }
}

pub(super) struct SessionsPage {
    env: Env,
    /// None while Telegram is asked.
    sessions: Option<ActiveSessions>,
    _load: Option<Task<()>>,
}

impl SessionsPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            env,
            sessions: None,
            _load: None,
        };
        page.reload(cx);
        page
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let task = crate::store::spawn(
            self.env.targets.store.read(cx).session().active_sessions(),
            cx,
        );
        self._load = Some(cx.spawn(async move |this, cx| match task.await {
            Ok(sessions) => {
                let _ = this.update(cx, |this, cx| {
                    this.sessions = Some(sessions);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "active sessions failed"),
        }));
    }

    /// Asks first, then terminates and reloads (`terminateOne`, `terminateAll`).
    fn confirm_terminate(
        &self,
        one: Option<SessionId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let weak = cx.weak_entity();
        let confirmed: crate::boxes::Callback = Rc::new(move |_, cx| {
            let _ = weak.update(cx, |this, cx| {
                let session = this.env.targets.store.read(cx).session().clone();
                let task = match one {
                    Some(id) => crate::store::spawn(session.terminate_session(id), cx),
                    None => crate::store::spawn(session.terminate_other_sessions(), cx),
                };
                cx.spawn(async move |this, cx| {
                    if let Err(error) = task.await {
                        tracing::warn!(%error, "terminating sessions failed");
                    }
                    let _ = this.update(cx, |this, cx| this.reload(cx));
                })
                .detach();
            });
        });
        let text = if one.is_some() {
            key::lng_settings_reset_one_sure
        } else {
            key::lng_settings_reset_sure
        };
        crate::boxes::confirm_attention(
            &self.env.targets.layers,
            tr(cx, text).to_string(),
            tr(cx, key::lng_settings_reset_button),
            confirmed,
            window,
            cx,
        );
    }

    fn row(&self, session: &ActiveSession, cx: &mut Context<Self>) -> AnyElement {
        let st = &SESSION_LIST_ITEM;
        let (from, to) = device_colors(session.device);
        let gradient = linear_gradient(
            180.0,
            linear_color_stop(color(cx, ColorRef::Palette(from)), 0.0),
            linear_color_stop(color(cx, ColorRef::Palette(to)), 1.0),
        );
        let place = if session.location.is_empty() {
            &session.ip
        } else {
            &session.location
        };
        let location = if session.is_current {
            place.clone()
        } else {
            let date = crate::dialogs::date_text(strings(cx), session.last_active, &Local::now());
            format!("{place} \u{2022} {date}")
        };
        let id = session.id;
        let (open, details) = (cx.weak_entity(), session.clone());
        let on_terminate = (!session.is_current).then(|| {
            let weak = cx.weak_entity();
            let on_terminate: Callback = Rc::new(move |window, cx| {
                let _ = weak.update(cx, |this, cx| this.confirm_terminate(Some(id), window, cx));
            });
            on_terminate
        });
        ListRow {
            id: ("session-row", id.0 as u64),
            st,
            location_top: LOCATION_TOP,
            cover: div()
                .size(px_of(st.photo_size))
                .rounded_full()
                .bg(gradient)
                .flex()
                .items_center()
                .justify_center()
                .child(ui::icon(device_icon(session.device)))
                .into_any_element(),
            name: session.device_model.clone().into(),
            status: session.application.clone().into(),
            location: location.into(),
            on_open: Rc::new(move |window, cx| {
                let _ = open.update(cx, |this, cx| this.open(details.clone(), window, cx));
            }),
            on_terminate,
        }
        .render(cx)
    }

    /// The details box; its Terminate asks first, as the row's button does.
    fn open(&self, session: ActiveSession, window: &mut Window, cx: &mut Context<Self>) {
        let on_terminate = (!session.is_current).then(|| {
            let (weak, id) = (cx.weak_entity(), session.id);
            let on_terminate: session_box::OnTerminate = Rc::new(move |window, cx| {
                let _ = weak.update(cx, |this, cx| this.confirm_terminate(Some(id), window, cx));
            });
            on_terminate
        });
        session_box::show(&self.env, session, on_terminate, window, cx);
    }

    /// "This device" with "Rename" at the right.
    // shortcut: the field's placeholder is the name TDLib has now, which is the custom one once renamed; keep the system's name in the store if that confuses.
    fn current_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let system_name = self
            .sessions
            .as_ref()
            .and_then(|sessions| sessions.sessions.iter().find(|s| s.is_current))
            .map(|session| session.device_model.clone())
            .unwrap_or_default();
        let env = self.env.clone();
        let right = px_of(SESSION_TERMINATE_SKIP) + px_of(SESSION_TERMINATE.icon_position.x);
        div()
            .relative()
            .child(rows::subtitle(tr(cx, key::lng_sessions_header), cx))
            .child(
                div()
                    .absolute()
                    .right(right)
                    .top(px_of(DEFAULT_SUBSECTION_TITLE_PADDING.top))
                    .child(
                        LinkButton::new(
                            "sessions-rename",
                            &DEFAULT_LINK_BUTTON,
                            tr(cx, key::lng_settings_rename_device),
                        )
                        .on_click(move |_, window, cx| {
                            session_box::show_rename(&env, &system_name, window, cx);
                        }),
                    ),
            )
            .into_any_element()
    }

    fn ttl_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let days = self
            .sessions
            .as_ref()
            .map_or(0, |sessions| sessions.inactive_ttl_days);
        let (env, weak) = (self.env.clone(), cx.weak_entity());
        rows::plain("sessions/ttl", tr(cx, key::lng_settings_terminate_if))
            .label(self_destruct_box::days_label(days, cx))
            .on_click(move |_, window, cx| {
                let weak = weak.clone();
                let on_saved: self_destruct_box::OnSaved = Rc::new(move |days, cx| {
                    let _ = weak.update(cx, |this: &mut SessionsPage, cx| {
                        if let Some(sessions) = &mut this.sessions {
                            sessions.inactive_ttl_days = days;
                        }
                        cx.notify();
                    });
                });
                self_destruct_box::show(
                    &env,
                    self_destruct_box::Kind::Sessions,
                    days,
                    on_saved,
                    window,
                    cx,
                );
            })
            .into_any_element()
    }
}

/// A group: the title, its rows, the gray text under them.
fn group(
    title: SharedString,
    rows_: Vec<AnyElement>,
    about: SharedString,
    cx: &App,
) -> Vec<AnyElement> {
    let mut items = vec![
        rows::skip_by(SESSION_SUBTITLE_SKIP),
        rows::subtitle(title, cx),
    ];
    items.extend(rows_);
    items.push(rows::skip());
    items.push(rows::divider_text(about, cx));
    items
}

impl Render for SessionsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(sessions) = self.sessions.clone() else {
            return div().child(rows::divider_text(tr(cx, key::lng_contacts_loading), cx));
        };
        let current: Vec<_> = sessions
            .sessions
            .iter()
            .filter(|s| s.is_current)
            .map(|s| self.row(s, cx))
            .collect();
        let incomplete: Vec<_> = sessions
            .sessions
            .iter()
            .filter(|s| !s.is_current && s.is_password_pending)
            .map(|s| self.row(s, cx))
            .collect();
        let others: Vec<_> = sessions
            .sessions
            .iter()
            .filter(|s| !s.is_current && !s.is_password_pending)
            .map(|s| self.row(s, cx))
            .collect();
        let has_others = !others.is_empty() || !incomplete.is_empty();
        let weak = cx.weak_entity();
        let terminate_all = SettingsButton::new(
            "sessions-terminate-all",
            &INFO_BLOCK_BUTTON,
            tr(cx, key::lng_sessions_terminate_all),
        )
        .icon(INFO_ICON_BLOCK)
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |this, cx| this.confirm_terminate(None, window, cx));
        });
        let mut items = vec![self.current_header(cx)];
        items.extend(current);
        items.push(rows::skip_by(CURRENT_SKIP));
        if has_others {
            items.push(terminate_all.into_any_element());
            items.push(rows::skip());
            items.push(rows::divider_text(
                tr(cx, key::lng_sessions_terminate_all_about),
                cx,
            ));
        }
        if !incomplete.is_empty() {
            let (title, about) = (
                tr(cx, key::lng_sessions_incomplete),
                tr(cx, key::lng_sessions_incomplete_about),
            );
            items.extend(group(title, incomplete, about, cx));
        }
        if !others.is_empty() {
            let (title, about) = (
                tr(cx, key::lng_sessions_other_header),
                tr(cx, key::lng_sessions_about_apps),
            );
            items.extend(group(title, others, about, cx));
        }
        items.push(rows::skip_by(SESSION_SUBTITLE_SKIP));
        items.push(rows::subtitle(
            tr(cx, key::lng_settings_terminate_title),
            cx,
        ));
        items.push(self.ttl_row(cx));
        items.push(rows::skip());
        div().flex().flex_col().children(items)
    }
}
