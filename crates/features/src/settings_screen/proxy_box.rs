//! "Edit proxy" (`ProxyBox`): the type, the socket address and the
//! credentials of a proxy; Save hands it back to the proxies box.
// shortcut: no WEB type, no Share link or QR, no "host:port" split while typing; with row "Proxy settings".

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, Styled, Window, div,
};
use i18n::key;
use model::proxies::{Proxy, ProxyKind};
use style::connection_box::{
    CONNECTION_HOST_INPUT_FIELD, CONNECTION_PASSWORD_INPUT_FIELD, CONNECTION_PORT_INPUT_FIELD,
    CONNECTION_USER_INPUT_FIELD, PROXY_ABOUT_SPONSOR_PADDING, PROXY_EDIT_INPUT_PADDING,
    PROXY_EDIT_SKIP, PROXY_EDIT_TITLE, PROXY_EDIT_TITLE_PADDING, PROXY_EDIT_TYPE_PADDING,
};
use style::layers::{BOX_WIDE_WIDTH, DEFAULT_BOX, DEFAULT_BOX_CHECKBOX};
use style::widgets::{DEFAULT_DIVIDER_LABEL, DEFAULT_RADIO};
use ui::{
    BoxFrame, Checkbox, Checked, InputField, InputMode, RoundButton, color, font, font_height,
};

use super::{Env, rows};
use crate::lang::tr;
use crate::px_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Type {
    Mtproto,
    Socks5,
    Http,
}

impl Type {
    const ALL: [(Self, &'static str); 3] = [
        (Self::Mtproto, "MTPROTO"),
        (Self::Socks5, "SOCKS5"),
        (Self::Http, "HTTP"),
    ];
}

pub(super) type OnSaved = Rc<dyn Fn(Proxy, &mut Window, &mut App)>;

/// What the fields hold, before checking them (`collectData`).
#[derive(Debug, PartialEq, Eq)]
enum Wrong {
    Host,
    Port,
    User,
    Secret,
}

fn collect(
    kind: Type,
    host: &str,
    port: &str,
    user: &str,
    password: &str,
    secret: &str,
) -> Result<Proxy, Wrong> {
    let host = host.trim();
    if host.is_empty() {
        return Err(Wrong::Host);
    }
    let port = port
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or(Wrong::Port)?;
    let kind = match kind {
        Type::Mtproto => {
            let secret = secret.trim();
            let is_valid = secret.len() >= 32
                && secret.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'+' | b'/' | b'=')
                });
            if !is_valid {
                return Err(Wrong::Secret);
            }
            ProxyKind::Mtproto {
                secret: secret.to_owned(),
            }
        }
        Type::Socks5 | Type::Http => {
            if !password.is_empty() && user.is_empty() {
                return Err(Wrong::User);
            }
            let (username, password) = (user.to_owned(), password.to_owned());
            if kind == Type::Socks5 {
                ProxyKind::Socks5 { username, password }
            } else {
                ProxyKind::Http { username, password }
            }
        }
    };
    Ok(Proxy {
        server: host.to_owned(),
        port,
        kind,
    })
}

pub(super) struct ProxyBox {
    env: Env,
    kind: Type,
    host: Entity<InputField>,
    port: Entity<InputField>,
    user: Entity<InputField>,
    password: Entity<InputField>,
    secret: Entity<InputField>,
    on_saved: OnSaved,
}

impl Focusable for ProxyBox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.host.focus_handle(cx)
    }
}

pub(super) fn show(
    env: &Env,
    current: Option<Proxy>,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    let view = cx.new(|cx: &mut Context<ProxyBox>| {
        let mut field = |style, placeholder, value: &str, mode: InputMode| {
            let placeholder = tr(cx, placeholder);
            let value = value.to_owned();
            cx.new(|cx| {
                let mut field = InputField::new(style, placeholder, window, cx).with_mode(mode);
                field.set_text(&value, None, cx);
                field
            })
        };
        let (kind, host, port, user, password, secret) = match &current {
            None => (Type::Mtproto, "", String::new(), "", "", ""),
            Some(proxy) => {
                let port = proxy.port.to_string();
                match &proxy.kind {
                    ProxyKind::Mtproto { secret } => (
                        Type::Mtproto,
                        proxy.server.as_str(),
                        port,
                        "",
                        "",
                        secret.as_str(),
                    ),
                    ProxyKind::Socks5 { username, password } => (
                        Type::Socks5,
                        proxy.server.as_str(),
                        port,
                        username.as_str(),
                        password.as_str(),
                        "",
                    ),
                    ProxyKind::Http { username, password } => (
                        Type::Http,
                        proxy.server.as_str(),
                        port,
                        username.as_str(),
                        password.as_str(),
                        "",
                    ),
                }
            }
        };
        ProxyBox {
            env: env.clone(),
            kind,
            host: field(
                &CONNECTION_HOST_INPUT_FIELD,
                key::lng_connection_host_ph,
                host,
                InputMode::Text,
            ),
            port: field(
                &CONNECTION_PORT_INPUT_FIELD,
                key::lng_connection_port_ph,
                &port,
                InputMode::Text,
            ),
            user: field(
                &CONNECTION_USER_INPUT_FIELD,
                key::lng_connection_user_ph,
                user,
                InputMode::Text,
            ),
            password: field(
                &CONNECTION_PASSWORD_INPUT_FIELD,
                key::lng_connection_password_ph,
                password,
                InputMode::Password,
            ),
            secret: field(
                &CONNECTION_USER_INPUT_FIELD,
                key::lng_connection_proxy_secret_ph,
                secret,
                InputMode::Text,
            ),
            on_saved,
        }
    });
    let host = view.read(cx).host.clone();
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
    crate::focus(&host, window, cx);
}

impl ProxyBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = |field: &Entity<InputField>, cx: &App| field.read(cx).text().to_owned();
        let collected = collect(
            self.kind,
            &text(&self.host, cx),
            &text(&self.port, cx),
            &text(&self.user, cx),
            &text(&self.password, cx),
            &text(&self.secret, cx),
        );
        let wrong = match collected {
            Ok(proxy) => {
                self.close(window, cx);
                (self.on_saved)(proxy, window, cx);
                return;
            }
            Err(Wrong::Host) => &self.host,
            Err(Wrong::Port) => &self.port,
            Err(Wrong::User) => &self.user,
            Err(Wrong::Secret) => &self.secret,
        };
        wrong.update(cx, |field, cx| field.show_error(cx));
    }

    /// A gray label over a group of fields (`addLabel`).
    fn label(text: gpui::SharedString, cx: &App) -> gpui::Div {
        let st = &PROXY_EDIT_TITLE;
        let padding = PROXY_EDIT_TITLE_PADDING;
        div()
            .pl(px_of(padding.left))
            .pr(px_of(padding.right))
            .pt(px_of(padding.top))
            .pb(px_of(padding.bottom))
            .font(font(st.style.font))
            .text_size(px_of(st.style.font.size))
            .line_height(font_height(st.style.font))
            .text_color(color(cx, st.text_fg))
            .child(text)
    }

    fn input(field: &Entity<InputField>) -> gpui::Div {
        let padding = PROXY_EDIT_INPUT_PADDING;
        div()
            .pl(px_of(padding.left))
            .pr(px_of(padding.right))
            .child(field.clone())
    }

    fn types(&self, cx: &mut Context<Self>) -> Vec<gpui::Div> {
        let padding = PROXY_EDIT_TYPE_PADDING;
        Type::ALL
            .iter()
            .map(|(kind, label)| {
                let kind = *kind;
                let checked = if self.kind == kind {
                    Checked::Yes
                } else {
                    Checked::No
                };
                let weak = cx.weak_entity();
                div()
                    .pl(px_of(padding.left))
                    .pr(px_of(padding.right))
                    .pt(px_of(padding.top))
                    .pb(px_of(padding.bottom))
                    .child(
                        Checkbox::radio(
                            ("proxy-type", kind as usize),
                            &DEFAULT_BOX_CHECKBOX,
                            &DEFAULT_RADIO,
                            *label,
                            checked,
                        )
                        .on_toggle(move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.kind = kind;
                                cx.notify();
                            });
                        }),
                    )
            })
            .collect()
    }

    fn content(&self, cx: &mut Context<Self>) -> gpui::Div {
        let sponsor = PROXY_ABOUT_SPONSOR_PADDING;
        let label = &DEFAULT_DIVIDER_LABEL.label;
        let mut column = div().flex().flex_col().children(self.types(cx));
        if self.kind == Type::Mtproto {
            column = column.child(
                div()
                    .pl(px_of(sponsor.left))
                    .pr(px_of(sponsor.right))
                    .pt(px_of(sponsor.top))
                    .pb(px_of(sponsor.bottom))
                    .font(font(label.style.font))
                    .text_size(px_of(label.style.font.size))
                    .line_height(font_height(label.style.font))
                    .text_color(color(cx, label.text_fg))
                    .child(tr(cx, key::lng_proxy_sponsor_warning)),
            );
        }
        let padding = PROXY_EDIT_INPUT_PADDING;
        column = column
            .child(Self::label(tr(cx, key::lng_proxy_address_label), cx))
            .child(
                div()
                    .pl(px_of(padding.left))
                    .pr(px_of(padding.right))
                    .flex()
                    .gap(px_of(PROXY_EDIT_SKIP))
                    .child(div().flex_1().child(self.host.clone()))
                    .child(
                        div()
                            .w(px_of(CONNECTION_PORT_INPUT_FIELD.width))
                            .child(self.port.clone()),
                    ),
            );
        column = if self.kind == Type::Mtproto {
            column
                .child(Self::label(tr(cx, key::lng_proxy_credentials), cx))
                .child(Self::input(&self.secret))
        } else {
            column
                .child(Self::label(tr(cx, key::lng_proxy_credentials_optional), cx))
                .child(Self::input(&self.user))
                .child(Self::input(&self.password))
        };
        column.child(rows::skip())
    }
}

impl Render for ProxyBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (save, cancel) = (cx.weak_entity(), cx.weak_entity());
        BoxFrame::new("proxy-box", &DEFAULT_BOX)
            .width(px_of(BOX_WIDE_WIDTH))
            .title(tr(cx, key::lng_proxy_edit))
            .row(self.content(cx))
            .button(
                RoundButton::new(
                    "proxy-save",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_settings_save),
                )
                .on_click(move |_, window, cx| {
                    let _ = save.update(cx, |this, cx| this.save(window, cx));
                }),
            )
            .button(
                RoundButton::new("proxy-cancel", &DEFAULT_BOX.button, tr(cx, key::lng_cancel))
                    .on_click(move |_, window, cx| {
                        let _ = cancel.update(cx, |this, cx| this.close(window, cx));
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collecting_checks_each_field_like_tdesktop() {
        let secret = "ee00112233445566778899aabbccddeeff";
        assert_eq!(
            collect(Type::Mtproto, " ", "443", "", "", secret),
            Err(Wrong::Host)
        );
        assert_eq!(
            collect(Type::Mtproto, "h", "0", "", "", secret),
            Err(Wrong::Port)
        );
        assert_eq!(
            collect(Type::Mtproto, "h", "443", "", "", "short"),
            Err(Wrong::Secret)
        );
        assert_eq!(
            collect(Type::Socks5, "h", "1080", "", "pass", ""),
            Err(Wrong::User)
        );
        let proxy = collect(Type::Http, "h", "8080", "", "", "").expect("no credentials is fine");
        assert_eq!(proxy.port, 8080);
        assert!(collect(Type::Mtproto, "h", "443", "", "", secret).is_ok());
    }
}
