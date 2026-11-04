//! "Connected websites" (`Settings::Websites`): disconnecting all, the
//! websites logged in with Telegram, each one's details.
// shortcut: the list loads once per opening, tdesktop polls it every 30 s while open; bot userpics are round, tdesktop's are the forum shape. With row "Connected websites".

use std::rc::Rc;

use chrono::Local;
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement, Render, SharedString, Styled, Task, Window, div,
};
use i18n::key;
use model::websites::{Website, WebsiteId};
use style::info::{INFO_BLOCK_BUTTON, INFO_ICON_BLOCK};
use style::layers::{
    ATTENTION_BOX_BUTTON, BOX_LABEL, BOX_MEDIUM_SKIP, BOX_PADDING, DEFAULT_BOX,
    DEFAULT_BOX_CHECKBOX,
};
use style::menu_icons::{MENU_ICON_ADDRESS, MENU_ICON_DEVICES, MENU_ICON_IP_ADDRESS};
use style::settings::{SESSION_SUBTITLE_SKIP, WEBSITE_LIST_ITEM, WEBSITE_LOCATION_TOP};
use style::widgets::DEFAULT_CHECK;
use ui::{
    BoxFrame, Checkbox, Checked, FlatLabel, Markup, RoundButton, SettingsButton, UserpicSource,
};

use super::session_box::{self, Cover, Details};
use super::sessions::ListRow;
use super::{Env, rows};
use crate::boxes::Callback;
use crate::dialogs::photo_userpic;
use crate::lang::{strings, tr, tr_with};
use crate::px_of;

/// `JoinNonEmpty`.
fn join_non_empty(parts: &[&str]) -> String {
    parts
        .iter()
        .filter(|part| !part.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(", ")
}

/// The bot's name and userpic.
fn bot_look(env: &Env, site: &Website, cx: &App) -> (String, UserpicSource) {
    let state = env.targets.store.read(cx).state();
    let Some(bot) = state.chats.user(site.bot) else {
        let source = UserpicSource::Empty {
            color_index: 0,
            name: site.domain.clone().into(),
        };
        return (site.domain.clone(), source);
    };
    let name = bot.name();
    let (source, _) = photo_userpic(&state.chats, bot.photo.as_ref(), bot.accent_color, &name);
    (name, source)
}

pub(super) struct WebsitesPage {
    env: Env,
    /// None while Telegram is asked.
    sites: Option<Vec<Website>>,
    _load: Option<Task<()>>,
}

impl WebsitesPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            env,
            sites: None,
            _load: None,
        };
        page.reload(cx);
        page
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let task = crate::store::spawn(
            self.env
                .targets
                .store
                .read(cx)
                .session()
                .connected_websites(),
            cx,
        );
        self._load = Some(cx.spawn(async move |this, cx| match task.await {
            Ok(sites) => {
                let _ = this.update(cx, |this, cx| {
                    this.sites = Some(sites);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "connected websites failed"),
        }));
    }

    /// Runs the disconnect, then asks for the list again.
    fn disconnect(
        &mut self,
        one: Option<(WebsiteId, model::UserId)>,
        block: bool,
        cx: &mut Context<Self>,
    ) {
        let session = self.env.targets.store.read(cx).session().clone();
        let task = match one {
            Some((id, _)) => crate::store::spawn(session.disconnect_website(id), cx),
            None => crate::store::spawn(session.disconnect_all_websites(), cx),
        };
        let block = one
            .filter(|_| block)
            .map(|(_, bot)| crate::store::spawn(session.block_user(bot), cx));
        cx.spawn(async move |this, cx| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "disconnecting websites failed");
            }
            if let Some(Err(error)) = match block {
                Some(task) => Some(task.await),
                None => None,
            } {
                tracing::warn!(%error, "blocking a website's bot failed");
            }
            let _ = this.update(cx, |this, cx| this.reload(cx));
        })
        .detach();
    }

    /// `terminateOne` and `terminateAll`: the box, then the request.
    fn confirm(&self, one: Option<&Website>, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        let target = one.map(|site| (site.id, site.bot));
        let on_confirm: OnConfirm = Rc::new(move |block, cx| {
            let _ = weak.update(cx, |this, cx| this.disconnect(target, block, cx));
        });
        let (title, text, block) = match one {
            Some(site) => {
                let (name, _) = bot_look(&self.env, site, cx);
                (
                    tr(cx, key::lng_settings_disconnect_title),
                    tr_with(
                        cx,
                        key::lng_settings_disconnect_sure,
                        &[("domain", &site.domain)],
                    ),
                    Some(tr_with(
                        cx,
                        key::lng_settings_disconnect_block,
                        &[("name", &name)],
                    )),
                )
            }
            None => (
                tr(cx, key::lng_settings_disconnect_all_title),
                tr(cx, key::lng_settings_disconnect_all_sure).to_string(),
                None,
            ),
        };
        let view = cx.new(|cx| DisconnectBox {
            env: self.env.clone(),
            title,
            text,
            block: block.map(|text| (text, Checked::No)),
            on_confirm,
            focus: cx.focus_handle(),
        });
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.show_above(view, window, cx));
    }

    fn open(&self, site: &Website, window: &mut Window, cx: &mut Context<Self>) {
        let (name, source) = bot_look(&self.env, site, cx);
        let (weak, target) = (cx.weak_entity(), site.clone());
        let on_disconnect: Callback = Rc::new(move |window, cx| {
            let _ = weak.update(cx, |this, cx| this.confirm(Some(&target), window, cx));
        });
        let details = Details {
            cover: Cover::Userpic(source),
            name,
            subtitle: site.domain.clone(),
            rows: vec![
                (
                    key::lng_sessions_browser,
                    join_non_empty(&[&site.browser, &site.platform]),
                    MENU_ICON_DEVICES,
                ),
                (key::lng_sessions_ip, site.ip.clone(), MENU_ICON_IP_ADDRESS),
                (
                    key::lng_sessions_location,
                    site.location.clone(),
                    MENU_ICON_ADDRESS,
                ),
            ],
            has_location: !site.location.is_empty(),
            terminate: Some((key::lng_settings_disconnect, on_disconnect)),
        };
        session_box::show_details(&self.env, details, window, cx);
    }

    fn row(&self, site: &Website, cx: &mut Context<Self>) -> AnyElement {
        let st = &WEBSITE_LIST_ITEM;
        let (name, source) = bot_look(&self.env, site, cx);
        let place = if site.location.is_empty() {
            &site.ip
        } else {
            &site.location
        };
        let date = crate::dialogs::date_text(strings(cx), site.last_active, &Local::now());
        let (open, disconnect) = (cx.weak_entity(), cx.weak_entity());
        let (opened, disconnected) = (site.clone(), site.clone());
        ListRow {
            id: ("website-row", site.id.0 as u64),
            st,
            location_top: WEBSITE_LOCATION_TOP,
            cover: ui::Userpic::new(px_of(st.photo_size), source).into_any_element(),
            name: name.into(),
            status: join_non_empty(&[&site.domain, &site.browser, &site.platform]).into(),
            location: format!("{place} \u{2022} {date}").into(),
            on_open: Rc::new(move |window, cx| {
                let _ = open.update(cx, |this, cx| this.open(&opened, window, cx));
            }),
            on_terminate: Some(Rc::new(move |window, cx| {
                let _ = disconnect.update(cx, |this, cx| {
                    this.confirm(Some(&disconnected), window, cx);
                });
            })),
        }
        .render(cx)
    }
}

impl Render for WebsitesPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(sites) = self.sites.clone() else {
            return div().child(rows::divider_text(tr(cx, key::lng_contacts_loading), cx));
        };
        let about = tr(cx, key::lng_settings_logged_in_description);
        if sites.is_empty() {
            return div().child(rows::divider_text(about, cx));
        }
        let weak = cx.weak_entity();
        let disconnect_all = SettingsButton::new(
            "websites-disconnect-all",
            &INFO_BLOCK_BUTTON,
            tr(cx, key::lng_settings_disconnect_all),
        )
        .icon(INFO_ICON_BLOCK)
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |this, cx| this.confirm(None, window, cx));
        });
        let list: Vec<_> = sites.iter().map(|site| self.row(site, cx)).collect();
        div()
            .flex()
            .flex_col()
            .child(disconnect_all)
            .child(rows::skip())
            .child(rows::divider_text(about, cx))
            .child(rows::skip_by(SESSION_SUBTITLE_SKIP))
            .child(rows::subtitle(
                tr(cx, key::lng_settings_logged_in_title),
                cx,
            ))
            .children(list)
            .child(rows::skip())
    }
}

/// What Disconnect does; the flag is "Block the bot".
type OnConfirm = Rc<dyn Fn(bool, &mut App)>;

/// `Content::terminate`'s box: the question, for one website the "Block"
/// checkbox, Disconnect and Cancel.
struct DisconnectBox {
    env: Env,
    title: SharedString,
    text: String,
    block: Option<(String, Checked)>,
    on_confirm: OnConfirm,
    focus: FocusHandle,
}

impl Focusable for DisconnectBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl DisconnectBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
    }

    fn confirm(&self, window: &mut Window, cx: &mut App) {
        let block = matches!(self.block, Some((_, Checked::Yes)));
        self.close(window, cx);
        (self.on_confirm)(block, cx);
    }
}

impl Render for DisconnectBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let padding = BOX_PADDING;
        let checkbox = self.block.as_ref().map(|(text, checked)| {
            let weak = cx.weak_entity();
            let checkbox = Checkbox::new(
                "disconnect-block",
                &DEFAULT_BOX_CHECKBOX,
                &DEFAULT_CHECK,
                text.clone(),
                *checked,
            )
            .on_toggle(move |_, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if let Some((_, checked)) = &mut this.block {
                        *checked = if *checked == Checked::Yes {
                            Checked::No
                        } else {
                            Checked::Yes
                        };
                    }
                    cx.notify();
                });
            });
            div().pt(px_of(BOX_MEDIUM_SKIP)).child(checkbox)
        });
        let content = div()
            .flex()
            .flex_col()
            .pl(px_of(padding.left))
            .pr(px_of(padding.right))
            .pt(px_of(padding.top))
            .pb(px_of(padding.bottom))
            .child(FlatLabel::new(
                "disconnect-text",
                &BOX_LABEL,
                Markup::plain(self.text.clone()),
            ))
            .children(checkbox);
        let (confirm, cancel) = (cx.weak_entity(), cx.weak_entity());
        div().track_focus(&self.focus).child(
            BoxFrame::new("disconnect-box", &DEFAULT_BOX)
                .title(self.title.clone())
                .row(content)
                .button(
                    RoundButton::new(
                        "disconnect-confirm",
                        &ATTENTION_BOX_BUTTON,
                        tr(cx, key::lng_settings_disconnect),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = confirm.update(cx, |this, cx| this.confirm(window, cx));
                    }),
                )
                .button(
                    RoundButton::new(
                        "disconnect-cancel",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_cancel),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = cancel.update(cx, |this, cx| this.close(window, cx));
                    }),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_parts_are_left_out() {
        assert_eq!(join_non_empty(&["a.com", "", "macOS"]), "a.com, macOS");
        assert_eq!(join_non_empty(&["", ""]), "");
    }
}
