//! One history item (`HistoryView::Message`): the date badge over a new
//! day, then the message in its bubble.

use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, Div, IntoElement, ParentElement, SharedString, Styled,
    Window, div, point, prelude::FluentBuilder as _, px,
};
use i18n::key;
use model::message::{Content, Message, Sending};
use model::text::FormattedText;
use std::rc::Rc;
use style::chat::{
    BUBBLE_RADIUS_LARGE, BUBBLE_RADIUS_SMALL, HISTORY_PHOTO_LEFT, HISTORY_VIEWS_SPACE,
    MSG_DATE_DELTA, MSG_DATE_FONT, MSG_FONT, MSG_MARGIN, MSG_MARGIN_TOP_ATTACHED, MSG_MAX_WIDTH,
    MSG_NAME_FONT, MSG_PADDING, MSG_PHOTO_SIZE, MSG_PHOTO_SKIP, MSG_SERVICE_FONT,
    MSG_SERVICE_MARGIN, MSG_SERVICE_PADDING, MSG_SHADOW,
};
use style::chat_style::{
    HISTORY_BUBBLE_TAIL_IN_LEFT, HISTORY_BUBBLE_TAIL_OUT_RIGHT, HISTORY_RECEIVED_ICON,
    HISTORY_SENT_ICON, HISTORY_VIEWS_IN_ICON, HISTORY_VIEWS_OUT_ICON,
};
use style::dialogs::HISTORY_SENDING_ICON;
use style::{ColorId, ColorRef};
use ui::{Userpic, UserpicSource, color, font, font_height, icon, icon_size, peer_palette_index};

use super::body::{self, Body, OnLink};
use super::headers::{self, Reply};
use super::{layout, unread};
use crate::lang::{strings, tr};
use crate::px_of;

/// Where a message sits in its run of grouped messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Run {
    pub is_attached_to_previous: bool,
    pub is_attached_to_next: bool,
}

/// Who wrote an incoming group message, shown on its run.
#[derive(Debug, Clone)]
pub(super) struct Author {
    pub name: SharedString,
    pub color_index: i32,
    pub userpic: UserpicSource,
}

/// A history item's content, ready to draw.
pub(super) struct Item<'a> {
    pub message: &'a Message,
    pub date_badge: Option<SharedString>,
    pub run: Run,
    /// Set in groups for incoming messages.
    pub author: Option<Author>,
    pub is_read: bool,
    pub time: String,
    /// The sender's color (`colorIndex`), for quotes and replies.
    pub color_index: i32,
    pub is_spoiler_revealed: bool,
    pub on_link: OnLink,
    pub reply: Option<Reply>,
    pub on_reply: Option<OnClick>,
    /// "Forwarded from ..." for forwarded messages.
    pub forward: Option<String>,
    pub has_unread_bar: bool,
}

/// What a click on part of a message does.
pub(super) type OnClick = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

const NAME_COLORS: [ColorId; 8] = [
    ColorId::HistoryPeer1NameFg,
    ColorId::HistoryPeer2NameFg,
    ColorId::HistoryPeer3NameFg,
    ColorId::HistoryPeer4NameFg,
    ColorId::HistoryPeer5NameFg,
    ColorId::HistoryPeer6NameFg,
    ColorId::HistoryPeer7NameFg,
    ColorId::HistoryPeer8NameFg,
];

/// U+2007 FIGURE SPACE: wide as a digit and never broken.
const FIGURE_SPACE: char = '\u{2007}';

fn palette(id: ColorId) -> ColorRef {
    ColorRef::Palette(id)
}

pub(super) fn element(item: Item<'_>, cx: &App) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .when_some(item.date_badge.clone(), |this, text| {
            this.child(service_pill(text, cx))
        })
        .when(item.has_unread_bar, |this| this.child(unread::bar(cx)))
        .child(message_row(&item, cx))
        .into_any_element()
}

/// A service message: its pill under the day's badge; hidden ones leave
/// only the badge.
pub(super) fn service_element(
    date_badge: Option<SharedString>,
    has_unread_bar: bool,
    text: Option<SharedString>,
    cx: &App,
) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .children(date_badge.map(|text| service_pill(text, cx)))
        .when(has_unread_bar, |this| this.child(unread::bar(cx)))
        .children(text.map(|text| service_pill(text, cx)))
        .into_any_element()
}

/// A centered service pill, as the date badge (`ServiceMessagePainter`).
pub(super) fn service_pill(text: SharedString, cx: &App) -> Div {
    let height = px_of(MSG_SERVICE_PADDING.top)
        + font_height(MSG_SERVICE_FONT)
        + px_of(MSG_SERVICE_PADDING.bottom);
    div()
        .flex()
        .justify_center()
        .pt(px_of(MSG_SERVICE_MARGIN.top))
        .pb(px_of(MSG_SERVICE_MARGIN.bottom))
        .px(px_of(MSG_SERVICE_MARGIN.left))
        .child(
            div()
                .min_h(height)
                .max_w(px_of(MSG_MAX_WIDTH))
                .pl(px_of(MSG_SERVICE_PADDING.left))
                .pr(px_of(MSG_SERVICE_PADDING.right))
                .pt(px_of(MSG_SERVICE_PADDING.top))
                .pb(px_of(MSG_SERVICE_PADDING.bottom))
                .rounded(height / 2.0)
                .bg(color(cx, palette(ColorId::MsgServiceBg)))
                .text_center()
                .font(font(MSG_SERVICE_FONT))
                .text_size(px_of(MSG_SERVICE_FONT.size))
                .line_height(font_height(MSG_SERVICE_FONT))
                .text_color(color(cx, palette(ColorId::MsgServiceFg)))
                .child(text),
        )
}

/// Channel posts show on the left even when the user wrote them
/// (`hasOutLayout`).
fn is_out(message: &Message) -> bool {
    message.is_outgoing && !message.is_channel_post
}

/// The bubble with the margins of its run, the author's userpic under the
/// last one.
// shortcut: no wide mode; tdesktop centers a column of left-aligned messages in wide windows (`ElementChatMode::Wide`).
fn message_row(item: &Item<'_>, cx: &App) -> Div {
    let out = is_out(item.message);
    let top = if item.run.is_attached_to_previous {
        MSG_MARGIN_TOP_ATTACHED
    } else {
        MSG_MARGIN.top
    };
    let photo_skip = if item.author.is_some() {
        px_of(MSG_PHOTO_SKIP)
    } else {
        px(0.0)
    };
    let (left, right) = if out {
        (px_of(MSG_MARGIN.right), px_of(MSG_MARGIN.left))
    } else {
        (px_of(MSG_MARGIN.left) + photo_skip, px_of(MSG_MARGIN.right))
    };
    let userpic = item
        .author
        .as_ref()
        .filter(|_| !item.run.is_attached_to_next)
        .map(|author| {
            div()
                .absolute()
                .left(px_of(HISTORY_PHOTO_LEFT))
                .bottom(px_of(MSG_MARGIN.bottom))
                .child(Userpic::new(px_of(MSG_PHOTO_SIZE), author.userpic.clone()))
        });
    div()
        .relative()
        .flex()
        .when(out, |this| this.justify_end())
        .pt(px_of(top))
        .pb(px_of(MSG_MARGIN.bottom))
        .pl(left)
        .pr(right)
        .children(userpic)
        .child(bubble(item, out, cx))
}

fn bubble(item: &Item<'_>, out: bool, cx: &App) -> Div {
    let (bg, shadow, fg) = if out {
        (
            ColorId::MsgOutBg,
            ColorId::MsgOutShadow,
            ColorId::HistoryTextOutFg,
        )
    } else {
        (
            ColorId::MsgInBg,
            ColorId::MsgInShadow,
            ColorId::HistoryTextInFg,
        )
    };
    let (large, small) = (px_of(BUBBLE_RADIUS_LARGE), px_of(BUBBLE_RADIUS_SMALL));
    let top = if item.run.is_attached_to_previous {
        small
    } else {
        large
    };
    let bottom = if item.run.is_attached_to_next {
        small
    } else {
        large
    };
    let has_tail = !item.run.is_attached_to_next;
    // The bubble's own side gets the run's corners; the far side stays large.
    let (top_left, top_right, bottom_left, bottom_right) = match (out, has_tail) {
        (true, true) => (large, top, large, px(0.0)),
        (true, false) => (large, top, large, bottom),
        (false, true) => (top, large, px(0.0), large),
        (false, false) => (top, large, bottom, large),
    };
    let tail = has_tail.then(|| {
        let tail = if out {
            HISTORY_BUBBLE_TAIL_OUT_RIGHT
        } else {
            HISTORY_BUBBLE_TAIL_IN_LEFT
        };
        let width = icon_size(tail).width;
        div()
            .absolute()
            .bottom_0()
            .when(out, |this| this.right(-width))
            .when(!out, |this| this.left(-width))
            .child(icon(tail))
    });
    let name = item
        .author
        .as_ref()
        .filter(|_| !item.run.is_attached_to_previous)
        .map(|author| {
            div()
                .truncate()
                .font(font(MSG_NAME_FONT))
                .text_size(px_of(MSG_NAME_FONT.size))
                .line_height(font_height(MSG_NAME_FONT))
                .text_color(color(cx, palette(name_color(author.color_index))))
                .child(author.name.clone())
        });
    div()
        .relative()
        .min_w_0()
        .max_w(px_of(MSG_MAX_WIDTH))
        .pt(px_of(MSG_PADDING.top))
        .pb(px_of(MSG_PADDING.bottom))
        .pl(px_of(MSG_PADDING.left))
        .pr(px_of(MSG_PADDING.right))
        .bg(color(cx, palette(bg)))
        .rounded_tl(top_left)
        .rounded_tr(top_right)
        .rounded_bl(bottom_left)
        .rounded_br(bottom_right)
        .shadow(vec![BoxShadow {
            color: color(cx, palette(shadow)),
            offset: point(px(0.0), px_of(MSG_SHADOW)),
            blur_radius: px(0.0),
            spread_radius: px(0.0),
            inset: false,
        }])
        .children(name)
        .children(headers(item, out, cx))
        .child(
            div()
                .font(font(MSG_FONT))
                .text_size(px_of(MSG_FONT.size))
                .line_height(font_height(MSG_FONT))
                .text_color(color(cx, palette(fg)))
                .child(text(item, out, cx)),
        )
        .child(info(item, out, cx))
        .children(tail)
}

/// The forward line and the reply box, in the bubble's colors.
fn headers(item: &Item<'_>, out: bool, cx: &App) -> Vec<AnyElement> {
    let forward_fg = palette(if out {
        ColorId::MsgOutServiceFg
    } else {
        ColorId::MsgInServiceFg
    });
    let forward = item
        .forward
        .clone()
        .map(|text| headers::forward_element(text, forward_fg, cx));
    let reply = item.reply.clone().map(|reply| {
        let (tint, name_fg, text_fg) = if out {
            (
                ColorId::MsgOutReplyBarColor,
                ColorId::MsgOutServiceFg,
                ColorId::HistoryTextOutFg,
            )
        } else {
            let tint = reply
                .color_index
                .map_or(ColorId::MsgInReplyBarColor, name_color);
            (tint, tint, ColorId::HistoryTextInFg)
        };
        let id = ("reply", item.message.id.0.unsigned_abs());
        headers::reply_element(
            reply,
            id,
            [palette(tint), palette(name_fg), palette(text_fg)],
            item.on_reply.clone(),
            cx,
        )
    });
    forward.into_iter().chain(reply).collect()
}

/// The post's views, as tdesktop shortens them (`FormatCountToShort`).
fn views(item: &Item<'_>) -> Option<String> {
    (item.message.views > 0).then(|| layout::short_count(item.message.views))
}

/// The signature, the edit mark and the time (`BottomInfo`).
fn info_text(item: &Item<'_>, cx: &App) -> String {
    let signature = &item.message.author_signature;
    let signature = if signature.is_empty() {
        String::new()
    } else {
        format!("{signature}, ")
    };
    if item.message.edit_date == 0 {
        format!("{signature}{}", item.time)
    } else {
        format!("{signature}{} {}", tr(cx, key::lng_edited), item.time)
    }
}

/// A sender's name color (`FromNameFg`).
pub(super) fn name_color(color_index: i32) -> ColorId {
    NAME_COLORS[peer_palette_index(color_index)]
}

/// The message text with, made transparent, room for the time on the last
/// line (`skipBlock`).
// shortcut: figure spaces approximate the time's width in the text font; tdesktop reserves the exact width.
fn text(item: &Item<'_>, out: bool, cx: &App) -> AnyElement {
    let preview;
    let text = match &item.message.content {
        Content::Text(text) => text,
        content => {
            preview =
                FormattedText::plain(crate::dialogs::content_preview(strings(cx), content).0.text);
            &preview
        }
    };
    let ticks = if out { "\u{2007}\u{2007}\u{2007}" } else { "" };
    let views = views(item).map_or_else(String::new, |views| {
        format!("{FIGURE_SPACE}{FIGURE_SPACE}{views}{FIGURE_SPACE}")
    });
    let room = format!(
        " {FIGURE_SPACE}{views}{}{ticks}",
        info_text(item, cx).replace(' ', "\u{00A0}")
    );
    let quote = if out {
        ColorId::MsgOutReplyBarColor
    } else {
        name_color(item.color_index)
    };
    body::element(
        Body {
            id: ("message", item.message.id.0 as u64).into(),
            text,
            is_out: out,
            quote,
            is_spoiler_revealed: item.is_spoiler_revealed,
            room,
            on_link: item.on_link.clone(),
        },
        cx,
    )
}

fn info(item: &Item<'_>, out: bool, cx: &App) -> Div {
    let date_fg = if out {
        ColorId::MsgOutDateFg
    } else {
        ColorId::MsgInDateFg
    };
    // shortcut: a failed message shows the sending clock; the red failed mark comes with sending in M4.
    let state = out.then_some(match item.message.sending {
        Some(Sending::Pending | Sending::Failed) => HISTORY_SENDING_ICON,
        None if item.is_read => HISTORY_RECEIVED_ICON,
        None => HISTORY_SENT_ICON,
    });
    div()
        .absolute()
        .right(px_of(MSG_PADDING.right) - px_of(MSG_DATE_DELTA.x))
        .bottom(px_of(MSG_PADDING.bottom) - px_of(MSG_DATE_DELTA.y))
        .flex()
        .items_center()
        .font(font(MSG_DATE_FONT))
        .text_size(px_of(MSG_DATE_FONT.size))
        .line_height(font_height(MSG_DATE_FONT))
        .text_color(color(cx, palette(date_fg)))
        .children(views(item).map(|views| {
            let views_icon = if out {
                HISTORY_VIEWS_OUT_ICON
            } else {
                HISTORY_VIEWS_IN_ICON
            };
            div()
                .flex()
                .items_center()
                .mr(px_of(HISTORY_VIEWS_SPACE))
                .child(icon(views_icon))
                .child(div().ml(px(2.0)).child(views))
        }))
        .child(info_text(item, cx))
        .children(state.map(|state| div().ml(px(2.0)).child(icon(state))))
}
