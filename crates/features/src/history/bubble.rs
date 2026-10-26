//! One history item (`HistoryView::Message`): the date badge over a new
//! day, then the message in its bubble.

use gpui::{
    AnyElement, App, BoxShadow, Div, HighlightStyle, IntoElement, ParentElement, SharedString,
    Styled, StyledText, div, point, prelude::FluentBuilder as _, px,
};
use i18n::key;
use model::message::{Content, Message, Sending};
use style::chat::{
    BUBBLE_RADIUS_LARGE, BUBBLE_RADIUS_SMALL, HISTORY_PHOTO_LEFT, MSG_DATE_DELTA, MSG_DATE_FONT,
    MSG_FONT, MSG_MARGIN, MSG_MARGIN_TOP_ATTACHED, MSG_MAX_WIDTH, MSG_NAME_FONT, MSG_PADDING,
    MSG_PHOTO_SIZE, MSG_PHOTO_SKIP, MSG_SERVICE_FONT, MSG_SERVICE_MARGIN, MSG_SERVICE_PADDING,
    MSG_SHADOW,
};
use style::chat_style::{
    HISTORY_BUBBLE_TAIL_IN_LEFT, HISTORY_BUBBLE_TAIL_OUT_RIGHT, HISTORY_RECEIVED_ICON,
    HISTORY_SENT_ICON,
};
use style::dialogs::HISTORY_SENDING_ICON;
use style::{ColorId, ColorRef};
use ui::{Userpic, UserpicSource, color, font, font_height, icon, icon_size, peer_palette_index};

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
#[derive(Debug)]
pub(super) struct Item<'a> {
    pub message: &'a Message,
    pub date_badge: Option<SharedString>,
    pub run: Run,
    /// Set in groups for incoming messages.
    pub author: Option<Author>,
    pub is_read: bool,
    pub time: String,
}

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
        .child(message_row(&item, cx))
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
                .text_color(color(
                    cx,
                    palette(NAME_COLORS[peer_palette_index(author.color_index)]),
                ))
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

/// The edit mark, the time and the ticks (`BottomInfo`).
fn info_text(item: &Item<'_>, cx: &App) -> String {
    if item.message.edit_date == 0 {
        item.time.clone()
    } else {
        format!("{} {}", tr(cx, key::lng_edited), item.time)
    }
}

/// The message text and, in its color made transparent, room for the
/// time on the last line (`skipBlock`).
// shortcut: figure spaces approximate the time's width in the text font; tdesktop reserves the exact width.
fn text(item: &Item<'_>, out: bool, cx: &App) -> StyledText {
    let text = match &item.message.content {
        Content::Text(text) => text.text.clone(),
        content => crate::dialogs::content_preview(strings(cx), content).0.text,
    };
    let ticks = if out { "\u{2007}\u{2007}\u{2007}" } else { "" };
    let room = format!(
        " {FIGURE_SPACE}{}{ticks}",
        info_text(item, cx).replace(' ', "\u{00A0}")
    );
    let start = text.len();
    let full = format!("{text}{room}");
    let end = full.len();
    StyledText::new(full).with_highlights([(
        start..end,
        // Highlight colors blend, so only fading hides the text.
        HighlightStyle {
            fade_out: Some(1.0),
            ..HighlightStyle::default()
        },
    )])
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
        .child(info_text(item, cx))
        .children(state.map(|state| div().ml(px(2.0)).child(icon(state))))
}
