//! The own profile's QR code (`Ui::FillPeerQrBox` for the self user): a
//! card with the code of the profile link and the username under it, close
//! above, copy and share below.
// shortcut: no colour themes, emoji or gift patterns, no free-code countdown, and copy and share pass the link, not the card's image; add them with the profile's QR for other peers.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, AppContext as _, ClipboardItem, Context, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, Styled, Task, Window, div, linear_color_stop, linear_gradient,
    prelude::*, px, rgb,
};
use i18n::key;
use model::ChatId;
use style::boxes::{
    PEER_QR_BOX_WIDTH, PEER_QR_BUTTON_SKIP, PEER_QR_CARD_SKIP, PEER_QR_CARD_WIDTH, PEER_QR_CLOSE,
    PEER_QR_COPY, PEER_QR_SHARE, PEER_QR_USERNAME_LARGE, PEER_QR_USERNAME_SKIP,
};
use style::layers::BOX_RADIUS;
use style::widgets::DEFAULT_TOAST;
use style::{ColorId, ColorRef};
use ui::{IconButton, QrCode, QrData, color, font};

use super::Env;
use crate::lang::tr;
use crate::px_of;

/// `kCardHeightRatio`, `kCardRadiusRatio` and `kQrTopRatio`.
const CARD_HEIGHT_RATIO: f32 = 1.07;
const CARD_RADIUS_RATIO: f32 = 0.125;
const QR_TOP_RATIO: f32 = 0.12;
/// `PeerQrDefaultAppearance`'s edge and center.
const CARD_EDGE: u32 = 0x0e_0f_0f;
const CARD_CENTER: u32 = 0x36_37_38;

pub(super) struct QrBox {
    env: Env,
    link: Option<String>,
    code: Option<Arc<QrData>>,
    username: String,
    focus: FocusHandle,
    _load: Task<()>,
}

impl std::fmt::Debug for QrBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QrBox")
            .field("has_link", &self.link.is_some())
            .finish_non_exhaustive()
    }
}

impl Focusable for QrBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Opens the QR box of `chat`, the own profile, over Settings.
pub(super) fn show(env: &Env, chat: ChatId, window: &mut Window, cx: &mut App) {
    let state = env.targets.store.read(cx).state();
    let username = state
        .chats
        .user(model::UserId(chat.0))
        .map(|user| user.username.to_uppercase())
        .unwrap_or_default();
    let task = crate::store::spawn(env.targets.store.read(cx).session().user_link(), cx);
    let env = env.clone();
    let view = cx.new(|cx| {
        let load = cx.spawn(async move |this, cx| match task.await {
            Ok(link) => {
                let _ = this.update(cx, |this: &mut QrBox, cx| this.set_link(link, cx));
            }
            Err(error) => tracing::warn!(%error, "own profile link failed"),
        });
        QrBox {
            env: env.clone(),
            link: None,
            code: None,
            username,
            focus: cx.focus_handle(),
            _load: load,
        }
    });
    let layers = env.targets.layers.clone();
    layers.update(cx, |layers, cx| layers.show_above(view, window, cx));
}

impl QrBox {
    fn set_link(&mut self, link: String, cx: &mut Context<Self>) {
        match QrData::encode(&link) {
            Ok(code) => self.code = Some(Arc::new(code)),
            Err(error) => tracing::warn!(%error, "profile link does not fit a QR code"),
        }
        self.link = Some(link);
        cx.notify();
    }

    fn close(&self, window: &mut Window, cx: &mut App) {
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
    }

    fn copy(&self, cx: &mut App) {
        let Some(link) = self.link.clone() else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(link));
        let text = tr(cx, key::lng_create_channel_link_copied);
        self.env.targets.layers.update(cx, |layers, cx| {
            layers.show_toast(text, &DEFAULT_TOAST, crate::toast::DEFAULT_DURATION, cx);
        });
    }

    fn share(&self, window: &mut Window, cx: &mut App) {
        let Some(link) = self.link.clone() else {
            return;
        };
        let store = self.env.targets.store.clone();
        let session = store.read(cx).session().clone();
        let on_chosen: crate::recipients_box::OnChosen = Rc::new(move |chat, _, cx| {
            let task = session.send_text(chat, None, None, (&link, &[]), Default::default());
            crate::peer_menu::send(task, "share profile link", cx);
        });
        crate::recipients_box::show(&store, &self.env.targets.layers, on_chosen, window, cx);
    }

    fn card(&self, cx: &App) -> impl IntoElement {
        let width = px_of(PEER_QR_CARD_WIDTH);
        let has_username = !self.username.is_empty();
        let height = if has_username {
            width * CARD_HEIGHT_RATIO
        } else {
            width
        };
        let qr_top = if has_username {
            width * QR_TOP_RATIO
        } else {
            (width - QrCode::side()) / 2.0
        };
        let username_font = PEER_QR_USERNAME_LARGE;
        let background = linear_gradient(
            180.0,
            linear_color_stop(rgb(CARD_CENTER), 0.0),
            linear_color_stop(rgb(CARD_EDGE), 1.0),
        );
        div()
            .relative()
            .w(width)
            .h(height)
            .rounded(width * CARD_RADIUS_RATIO)
            .bg(background)
            .child(
                div()
                    .absolute()
                    .top(qr_top)
                    .left((width - QrCode::side()) / 2.0)
                    .child(QrCode::new("settings-qr-code", self.code.clone())),
            )
            .when(has_username, |this| {
                this.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(qr_top + QrCode::side() + px_of(PEER_QR_USERNAME_SKIP) / 2.0)
                        .flex()
                        .justify_center()
                        .font(font(username_font))
                        .text_size(px_of(username_font.size))
                        .text_color(color(cx, ColorRef::Palette(ColorId::WindowFgActive)))
                        .truncate()
                        .child(self.username.clone()),
                )
            })
    }
}

impl Render for QrBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let skip = px_of(PEER_QR_BUTTON_SKIP);
        let weak = cx.weak_entity();
        let (close, copy, share) = (weak.clone(), weak.clone(), weak);
        let buttons = div()
            .flex()
            .justify_between()
            .px(skip)
            .pt(px_of(PEER_QR_CARD_SKIP))
            .pb(skip)
            .child(
                IconButton::new("settings-qr-copy", &PEER_QR_COPY).on_click(move |_, _, cx| {
                    let _ = copy.update(cx, |this, cx| this.copy(cx));
                }),
            )
            .child(
                IconButton::new("settings-qr-share", &PEER_QR_SHARE).on_click(
                    move |_, window, cx| {
                        let _ = share.update(cx, |this, cx| this.share(window, cx));
                    },
                ),
            );
        div()
            .track_focus(&self.focus)
            .w(px_of(PEER_QR_BOX_WIDTH))
            .rounded(px_of(BOX_RADIUS))
            .bg(color(cx, ColorRef::Palette(ColorId::BoxBg)))
            .flex()
            .flex_col()
            .child(div().flex().justify_end().pt(skip).pr(skip).child(
                IconButton::new("settings-qr-close", &PEER_QR_CLOSE).on_click(
                    move |_, window, cx| {
                        let _ = close.update(cx, |this, cx| this.close(window, cx));
                    },
                ),
            ))
            .child(
                div()
                    .flex()
                    .justify_center()
                    .pt(px_of(PEER_QR_CARD_SKIP))
                    .child(self.card(cx)),
            )
            .child(buttons)
            .child(div().h(px(0.0)))
    }
}
