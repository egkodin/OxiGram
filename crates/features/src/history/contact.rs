//! A contact card in a message (`HistoryView::Contact`): the userpic, the
//! name and the phone in a box tinted like a reply, with MESSAGE and ADD
//! under them when the card is a Telegram account.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement as _, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, div,
};
use i18n::key;
use model::UserId;
use model::message::{Content, Message, SharedContact};
use style::basic::SEMIBOLD_FONT;
use style::boxes::CONTACTS_PHOTO_SIZE;
use style::chat::{
    HISTORY_PAGE_BUTTON_HEIGHT, HISTORY_PAGE_BUTTON_LINE, HISTORY_PAGE_BUTTON_PADDING,
    HISTORY_PAGE_PREVIEW, MSG_DATE_FONT, MSG_PADDING, WEB_PAGE_DESCRIPTION_FONT,
    WEB_PAGE_TITLE_FONT,
};
use style::{ColorId, Font};
use ui::{Userpic, UserpicSource, color, font, font_height};

use super::bubble::{OnClick, name_color, palette};
use super::headers::quote_frame;
use super::{HistoryEvent, HistoryView, Link};
use crate::dialogs::photo_userpic;
use crate::lang::{format_phone, tr, tr_with};
use crate::px_of;

/// The line over the buttons, of the name's color.
const BUTTON_LINE_OPACITY: f32 = 0.3;
/// `kSimpleColorIndexCount`.
const SIMPLE_COLORS: u64 = 7;

pub(super) struct Card {
    userpic: UserpicSource,
    name: String,
    phone: String,
    color_index: i32,
    /// MESSAGE first: a click on the card does the same.
    buttons: Vec<(SharedString, OnClick)>,
}

/// CRC-32 of the name's UTF-16 bytes (`base::crc32`).
fn crc32(bytes: impl IntoIterator<Item = u8>) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

/// The color of a card with no account (`FakePeerIdForJustName`).
fn name_color_index(name: &str) -> i32 {
    let bytes = name.encode_utf16().flat_map(u16::to_le_bytes);
    #[expect(
        clippy::cast_possible_wrap,
        reason = "tdesktop reads the hash as int32"
    )]
    let hash = if name.is_empty() {
        777
    } else {
        crc32(bytes) as i32
    };
    let peer = (0xFE_u64 << 32) + u64::from(hash.unsigned_abs());
    i32::try_from(peer % SIMPLE_COLORS).unwrap_or(0)
}

impl HistoryView {
    /// A contact message's card, downloading the account's userpic.
    // shortcut: a card without an account has no VIEW CONTACT box of its vCard; with contacts in M6.
    pub(super) fn contact(&mut self, message: &Message, cx: &mut Context<Self>) -> Option<Card> {
        let Content::Contact(contact) = &message.content else {
            return None;
        };
        let name = contact.name();
        let chats = &self.store.read(cx).state().chats;
        let user = contact.user.and_then(|user| chats.user(user));
        let (userpic, file, color_index) = match user {
            Some(user) => {
                let (userpic, file) =
                    photo_userpic(chats, user.photo.as_ref(), user.accent_color, &name);
                (userpic, file, user.accent_color)
            }
            None => {
                let color_index = name_color_index(&name);
                let userpic = UserpicSource::Empty {
                    color_index,
                    name: name.clone().into(),
                };
                (userpic, None, color_index)
            }
        };
        let is_contact = user.is_some_and(|user| user.is_contact);
        if let Some(file) = file {
            self.request_file(file, cx);
        }
        let buttons = match contact.user {
            Some(id) => {
                let mut buttons = vec![(
                    upper(cx, key::lng_contact_send_message),
                    self.on_message(id, cx),
                )];
                if !is_contact {
                    let add = self.on_add(id, contact, cx);
                    buttons.push((upper(cx, key::lng_contact_add), add));
                }
                buttons
            }
            None => Vec::new(),
        };
        Some(Card {
            userpic,
            name,
            phone: format_phone(contact.phone.expose()),
            color_index,
            buttons,
        })
    }

    fn on_message(&self, user: UserId, cx: &Context<Self>) -> OnClick {
        let weak = cx.weak_entity();
        Rc::new(move |_, _, cx| {
            let _ = weak.update(cx, |_, cx| cx.emit(HistoryEvent::Link(Link::User(user))));
        })
    }

    /// Adds the account under the card's name.
    // shortcut: no AddContactBox to change the name first; the box comes with contacts in M6.
    fn on_add(&self, user: UserId, contact: &SharedContact, cx: &Context<Self>) -> OnClick {
        let weak = cx.weak_entity();
        let (first, last) = (contact.first_name.clone(), contact.last_name.clone());
        Rc::new(move |_, _, cx| {
            let (first, last) = (first.clone(), last.clone());
            let _ = weak.update(cx, |this, cx| {
                let done = tr_with(cx, key::lng_new_contact_add_done, &[("user", &first)]);
                let task = this.store.read(cx).session().add_contact(user, first, last);
                let task = crate::store::spawn(task, cx);
                let layers = this.layers.downgrade();
                cx.spawn(async move |_, cx| match task.await {
                    Ok(()) => {
                        let _ = layers.update(cx, |layers, cx| {
                            layers.show_toast(
                                done.into(),
                                &style::widgets::DEFAULT_TOAST,
                                super::menu::TOAST_DURATION,
                                cx,
                            );
                        });
                    }
                    Err(error) => tracing::warn!(%error, "adding a shared contact failed"),
                })
                .detach();
            });
        })
    }
}

fn upper(cx: &App, key: i18n::Key) -> SharedString {
    tr(cx, key).to_uppercase().into()
}

/// The card in its box, the room for the time under it.
pub(super) fn element(card: Card, id: u64, out: bool, is_selected: bool, cx: &App) -> AnyElement {
    let (tint, name_fg, text_fg) = if out {
        (
            ColorId::MsgOutReplyBarColor,
            ColorId::MsgOutServiceFg,
            ColorId::HistoryTextOutFg,
        )
    } else {
        let tint = name_color(card.color_index);
        (tint, tint, ColorId::HistoryTextInFg)
    };
    let name_fg = color(cx, palette(name_fg));
    let padding = HISTORY_PAGE_PREVIEW.padding;
    let photo = px_of(CONTACTS_PHOTO_SIZE);
    let line_height = font_height(SEMIBOLD_FONT).max(font_height(style::basic::NORMAL_FONT));
    let line = |text: String, line_font: Font, fg| {
        div()
            .truncate()
            .h(line_height)
            .font(font(line_font))
            .text_size(px_of(line_font.size))
            .line_height(line_height)
            .text_color(fg)
            .child(text)
    };
    let main = card.buttons.first().map(|(_, on_click)| on_click.clone());
    let has_buttons = !card.buttons.is_empty();
    let userpic = div()
        .relative()
        .flex_none()
        .size(photo)
        .child(Userpic::new(photo, card.userpic))
        .when(is_selected, |this| {
            this.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded_full()
                    .bg(color(cx, palette(ColorId::MsgSelectOverlay))),
            )
        });
    let texts = div()
        .min_w_0()
        .flex_1()
        .child(line(card.name, WEB_PAGE_TITLE_FONT, name_fg))
        // tdesktop's three line widths between the two.
        .child(
            line(
                card.phone,
                WEB_PAGE_DESCRIPTION_FONT,
                color(cx, palette(text_fg)),
            )
            .mt(gpui::px(3.0)),
        );
    let frame = quote_frame(("contact", id), color(cx, palette(tint)), padding)
        .pb_0()
        .mt(px_of(MSG_PADDING.left) - px_of(MSG_PADDING.top))
        .when_some(main, |this, on_click| {
            this.cursor_pointer()
                .on_click(move |event, window, cx| on_click(event, window, cx))
        })
        .child(
            div()
                .flex()
                .gap(px_of(padding.left))
                .pb(px_of(padding.bottom))
                .child(userpic)
                .child(texts),
        )
        .when(has_buttons, |this| {
            this.child(buttons(card.buttons, id, name_fg))
        });
    div()
        .child(frame)
        // The time's line under the box.
        .pb(font_height(MSG_DATE_FONT))
        .into_any_element()
}

/// The buttons side by side over a thin line (`historyPageButton*`).
fn buttons(buttons: Vec<(SharedString, OnClick)>, id: u64, fg: gpui::Hsla) -> AnyElement {
    div()
        .flex()
        .h(px_of(HISTORY_PAGE_BUTTON_HEIGHT))
        .border_t(px_of(HISTORY_PAGE_BUTTON_LINE))
        .border_color(fg.opacity(BUTTON_LINE_OPACITY))
        .gap(px_of(HISTORY_PAGE_BUTTON_PADDING.left))
        .font(font(SEMIBOLD_FONT))
        .text_size(px_of(SEMIBOLD_FONT.size))
        .line_height(font_height(SEMIBOLD_FONT))
        .text_color(fg)
        .children(
            buttons
                .into_iter()
                .enumerate()
                .map(|(index, (text, on_click))| {
                    div()
                        .id((
                            "contact-button",
                            id.wrapping_mul(4).wrapping_add(index as u64),
                        ))
                        .flex_1()
                        .flex()
                        .justify_center()
                        .pt(px_of(HISTORY_PAGE_BUTTON_PADDING.top))
                        .cursor_pointer()
                        .on_click(move |event, window, cx| {
                            // The card under it does its own.
                            cx.stop_propagation();
                            on_click(event, window, cx);
                        })
                        .child(text)
                }),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_the_standard_check_value() {
        assert_eq!(crc32(*b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn a_name_alone_picks_one_of_seven_colors() {
        // 0xFE << 32 is 1 mod 7, and 777 is 0.
        assert_eq!(name_color_index(""), 1);
        let index = name_color_index("Anna Ivanova");
        assert!((0..7).contains(&index));
        assert_eq!(index, name_color_index("Anna Ivanova"));
    }
}
