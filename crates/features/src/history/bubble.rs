//! One history item (`HistoryView::Message`): the date badge over a new
//! day, then the message in its bubble.

use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, Div, ElementId, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement, Pixels, SharedString, StatefulInteractiveElement as _, Styled,
    Window, div, point, prelude::FluentBuilder as _, px,
};
use i18n::key;
use model::message::{Content, LinkPreview, Message, Sending};
use model::text::FormattedText;
use std::ops::Range;
use std::rc::Rc;
use style::chat::{
    BUBBLE_RADIUS_LARGE, BUBBLE_RADIUS_SMALL, HISTORY_COMMENTS_BUTTON_HEIGHT, HISTORY_PAGE_PREVIEW,
    HISTORY_PHOTO_LEFT, HISTORY_VIEWS_SPACE, MSG_DATE_DELTA, MSG_DATE_FONT, MSG_FONT, MSG_MARGIN,
    MSG_MARGIN_TOP_ATTACHED, MSG_MAX_WIDTH, MSG_NAME_FONT, MSG_PADDING, MSG_PHOTO_SIZE,
    MSG_PHOTO_SKIP, MSG_SERVICE_FONT, MSG_SERVICE_MARGIN, MSG_SERVICE_PADDING, MSG_SHADOW,
};
use style::chat_style::{
    HISTORY_BUBBLE_TAIL_IN_LEFT, HISTORY_BUBBLE_TAIL_IN_LEFT_SELECTED,
    HISTORY_BUBBLE_TAIL_OUT_RIGHT, HISTORY_BUBBLE_TAIL_OUT_RIGHT_SELECTED, HISTORY_RECEIVED_ICON,
    HISTORY_RECEIVED_SELECTED_ICON, HISTORY_SENT_ICON, HISTORY_SENT_SELECTED_ICON,
    HISTORY_VIEWS_IN_ICON, HISTORY_VIEWS_OUT_ICON,
};
use style::dialogs::HISTORY_SENDING_ICON;
use style::{ColorId, ColorRef};
use ui::{Userpic, UserpicSource, color, font, font_height, icon, icon_size, peer_palette_index};

use super::body::{self, Body, OnLink};
use super::comments::Comments;
use super::headers::{self, Reply};
use super::selection::{OnPointer, Pointer};
use super::web_page::{self, WebPage};
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
    pub date_badge: Option<DateBadge>,
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
    /// "via @bot" for messages sent through an inline bot.
    pub via: Option<String>,
    pub has_unread_bar: bool,
    pub is_selected: bool,
    /// Shaded after a jump to it (`ElementHighlighter`).
    pub is_highlighted: bool,
    /// The selected part of the text.
    pub selected_text: Option<Range<usize>>,
    pub on_pointer: OnPointer,
    /// The comments button under a channel post.
    pub comments: Option<Comments>,
    /// What follows the message in its row, as a thread's divider.
    pub after: Option<AnyElement>,
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

/// The space between the name and "via @bot": one space of the name font.
const NAME_VIA_GAP: f32 = 4.0;

/// U+2007 FIGURE SPACE: wide as a digit and never broken.
const FIGURE_SPACE: char = '\u{2007}';

fn palette(id: ColorId) -> ColorRef {
    ColorRef::Palette(id)
}

pub(super) fn element(mut item: Item<'_>, cx: &App) -> AnyElement {
    let after = item.after.take();
    div()
        .w_full()
        .flex()
        .flex_col()
        .when_some(item.date_badge.clone(), |this, badge| {
            this.child(date_pill(badge, cx))
        })
        .when(item.has_unread_bar, |this| this.child(unread::bar(cx)))
        .child(with_pointer(message_row(&mut item, cx), &item.on_pointer))
        .children(after)
        .into_any_element()
}

/// Presses, drags and releases over the row that the text did not take.
fn with_pointer(row: Div, on_pointer: &OnPointer) -> Div {
    let (on_press, on_move, on_release, on_menu) = (
        on_pointer.clone(),
        on_pointer.clone(),
        on_pointer.clone(),
        on_pointer.clone(),
    );
    row.on_mouse_down(MouseButton::Left, move |_, window, cx| {
        on_press(Pointer::PressRow, window, cx);
    })
    .on_mouse_down(MouseButton::Right, move |_, window, cx| {
        on_menu(Pointer::Menu(None), window, cx);
    })
    .on_mouse_move(move |event, window, cx| {
        let pointer = if event.pressed_button == Some(MouseButton::Left) {
            Pointer::Drag(None)
        } else {
            Pointer::Hover
        };
        on_move(pointer, window, cx);
    })
    .on_mouse_up(MouseButton::Left, move |event, window, cx| {
        let is_toggle = event.modifiers.secondary(cx);
        on_release(Pointer::Release { is_toggle }, window, cx);
    })
}

/// A service message: its pill under the day's badge; hidden ones leave
/// only the badge.
pub(super) fn service_element(
    date_badge: Option<DateBadge>,
    has_unread_bar: bool,
    text: Option<SharedString>,
    cx: &App,
) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .children(date_badge.map(|badge| date_pill(badge, cx)))
        .when(has_unread_bar, |this| this.child(unread::bar(cx)))
        .children(text.map(|text| service_pill(text, cx)))
        .into_any_element()
}

/// A day's badge over its first message; a click opens the calendar.
#[derive(Clone)]
pub(super) struct DateBadge {
    pub text: SharedString,
    pub id: ElementId,
    pub on_click: OnClick,
}

pub(super) fn date_pill(badge: DateBadge, cx: &App) -> Div {
    let on_click = badge.on_click;
    pill_row(
        pill(badge.text, cx)
            .id(badge.id)
            .cursor_pointer()
            .on_click(move |event, window, cx| on_click(event, window, cx)),
    )
}

/// A centered service pill, as the date badge (`ServiceMessagePainter`).
pub(super) fn service_pill(text: SharedString, cx: &App) -> Div {
    pill_row(pill(text, cx))
}

/// The row a pill sits centered in.
fn pill_row(pill: impl IntoElement) -> Div {
    div()
        .flex()
        .justify_center()
        .pt(px_of(MSG_SERVICE_MARGIN.top))
        .pb(px_of(MSG_SERVICE_MARGIN.bottom))
        .px(px_of(MSG_SERVICE_MARGIN.left))
        .child(pill)
}

fn pill_height() -> Pixels {
    px_of(MSG_SERVICE_PADDING.top)
        + font_height(MSG_SERVICE_FONT)
        + px_of(MSG_SERVICE_PADDING.bottom)
}

/// How far a date badge reaches below its message's top.
pub(super) fn date_row_height() -> Pixels {
    px_of(MSG_SERVICE_MARGIN.top) + pill_height()
}

fn pill(text: SharedString, cx: &App) -> Div {
    let height = pill_height();
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
        .child(text)
}

/// Channel posts show on the left even when the user wrote them
/// (`hasOutLayout`).
fn is_out(message: &Message) -> bool {
    message.is_outgoing && !message.is_channel_post
}

/// The bubble with the margins of its run, the author's userpic under the
/// last one.
// shortcut: no wide mode; tdesktop centers a column of left-aligned messages in wide windows (`ElementChatMode::Wide`).
fn message_row(item: &mut Item<'_>, cx: &App) -> Div {
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
    let overlay = (item.is_selected || item.is_highlighted).then(|| {
        div()
            .absolute()
            .inset_0()
            .bg(color(cx, palette(ColorId::MsgSelectOverlay)))
    });
    div()
        .relative()
        .flex()
        .when(out, |this| this.justify_end())
        .children(overlay)
        .pt(px_of(top))
        .pb(px_of(MSG_MARGIN.bottom))
        .pl(left)
        .pr(right)
        .children(userpic)
        .child(bubble(item, out, cx))
}

fn bubble(item: &mut Item<'_>, out: bool, cx: &App) -> Div {
    let (bg, shadow, fg) = match (out, item.is_selected) {
        (true, false) => (
            ColorId::MsgOutBg,
            ColorId::MsgOutShadow,
            ColorId::HistoryTextOutFg,
        ),
        (true, true) => (
            ColorId::MsgOutBgSelected,
            ColorId::MsgOutShadowSelected,
            ColorId::HistoryTextOutFgSelected,
        ),
        (false, false) => (
            ColorId::MsgInBg,
            ColorId::MsgInShadow,
            ColorId::HistoryTextInFg,
        ),
        (false, true) => (
            ColorId::MsgInBgSelected,
            ColorId::MsgInShadowSelected,
            ColorId::HistoryTextInFgSelected,
        ),
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
        let tail = match (out, item.is_selected) {
            (true, false) => HISTORY_BUBBLE_TAIL_OUT_RIGHT,
            (true, true) => HISTORY_BUBBLE_TAIL_OUT_RIGHT_SELECTED,
            (false, false) => HISTORY_BUBBLE_TAIL_IN_LEFT,
            (false, true) => HISTORY_BUBBLE_TAIL_IN_LEFT_SELECTED,
        };
        let width = icon_size(tail).width;
        div()
            .absolute()
            .bottom_0()
            .when(out, |this| this.right(-width))
            .when(!out, |this| this.left(-width))
            .child(icon(tail))
    });
    let name = name_line(item, out, cx);
    let info = info(item, out, cx);
    let comments = item
        .comments
        .take()
        .map(|comments| super::comments::button(comments, item.message.id.0, item.is_selected, cx));
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
        .children(web_page(item, out, Placement::AboveText, cx))
        .child(
            div()
                .font(font(MSG_FONT))
                .text_size(px_of(MSG_FONT.size))
                .line_height(font_height(MSG_FONT))
                .text_color(color(cx, palette(fg)))
                .child(text(item, out, cx)),
        )
        .children(web_page(item, out, Placement::BelowText, cx))
        .child(info)
        .children(comments)
        .children(tail)
}

/// The sender's name and "via @bot" after it (`HistoryMessageVia`); the
/// via label stands alone when no name shows above a message.
// shortcut: a forwarded message's via label is dropped, tdesktop appends it to the forward line; with forward headers in M5.
fn name_line(item: &Item<'_>, out: bool, cx: &App) -> Option<Div> {
    let author = item
        .author
        .as_ref()
        .filter(|_| !item.run.is_attached_to_previous);
    let via = item
        .via
        .as_ref()
        .filter(|_| author.is_some() || item.forward.is_none());
    if author.is_none() && via.is_none() {
        return None;
    }
    let via_fg = palette(if out {
        ColorId::MsgOutServiceFg
    } else {
        ColorId::MsgInServiceFg
    });
    Some(
        div()
            .flex()
            .gap(px(NAME_VIA_GAP))
            .font(font(MSG_NAME_FONT))
            .text_size(px_of(MSG_NAME_FONT.size))
            .line_height(font_height(MSG_NAME_FONT))
            .children(author.map(|author| {
                div()
                    .truncate()
                    .text_color(color(cx, palette(name_color(author.color_index))))
                    .child(author.name.clone())
            }))
            .children(via.map(|via| {
                div()
                    .flex_none()
                    .text_color(color(cx, via_fg))
                    .child(via.clone())
            })),
    )
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

/// Where a link preview goes against the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    AboveText,
    BelowText,
}

fn link_preview<'a>(item: &Item<'a>) -> Option<&'a LinkPreview> {
    match &item.message.content {
        Content::Text { preview, .. } => preview.as_deref(),
        _ => None,
    }
}

/// The link preview, when it goes at `placement`; below the text it holds
/// the room for the time.
fn web_page(item: &Item<'_>, out: bool, placement: Placement, cx: &App) -> Option<AnyElement> {
    let preview = link_preview(item)?;
    let is_above = placement == Placement::AboveText;
    if preview.is_above_text != is_above {
        return None;
    }
    let (tint, name_fg, text_fg) = if out {
        (
            ColorId::MsgOutReplyBarColor,
            ColorId::MsgOutServiceFg,
            ColorId::HistoryTextOutFg,
        )
    } else {
        let tint = name_color(item.color_index);
        (tint, tint, ColorId::HistoryTextInFg)
    };
    Some(web_page::element(
        WebPage {
            preview,
            id: ("web-page", item.message.id.0.unsigned_abs()).into(),
            colors: [palette(tint), palette(name_fg), palette(text_fg)],
            room: (!is_above).then(|| time_room(item, out, cx)),
            on_link: item.on_link.clone(),
        },
        cx,
    ))
}

/// Transparent text as wide as the time, for the last line under it.
fn time_room(item: &Item<'_>, out: bool, cx: &App) -> String {
    let ticks = if out { "\u{2007}\u{2007}\u{2007}" } else { "" };
    let views = views(item).map_or_else(String::new, |views| {
        format!("{FIGURE_SPACE}{FIGURE_SPACE}{views}{FIGURE_SPACE}")
    });
    format!(
        " {FIGURE_SPACE}{views}{}{ticks}",
        info_text(item, cx).replace(' ', "\u{00A0}")
    )
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
        Content::Text { text, .. } => text,
        Content::Unsupported => {
            preview = crate::dialogs::unsupported_text(strings(cx));
            &preview
        }
        content => {
            preview =
                FormattedText::plain(crate::dialogs::content_preview(strings(cx), content).0.text);
            &preview
        }
    };
    // Below the text the link preview holds the room for the time.
    let has_preview_below = link_preview(item).is_some_and(|preview| !preview.is_above_text);
    let room = if has_preview_below {
        String::new()
    } else {
        time_room(item, out, cx)
    };
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
            is_selected: item.is_selected,
            selected: item.selected_text.clone(),
            quote,
            is_spoiler_revealed: item.is_spoiler_revealed,
            room,
            on_link: item.on_link.clone(),
            on_pointer: item.on_pointer.clone(),
        },
        cx,
    )
}

fn info(item: &Item<'_>, out: bool, cx: &App) -> Div {
    let date_fg = match (out, item.is_selected) {
        (true, false) => ColorId::MsgOutDateFg,
        (true, true) => ColorId::MsgOutDateFgSelected,
        (false, false) => ColorId::MsgInDateFg,
        (false, true) => ColorId::MsgInDateFgSelected,
    };
    // Over a link preview below the text the time sits on its last line.
    let preview_lift = if link_preview(item).is_some_and(|preview| !preview.is_above_text) {
        px_of(HISTORY_PAGE_PREVIEW.padding.bottom)
    } else {
        px(0.0)
    };
    // The comments button holds the bubble's bottom; the time sits over it.
    let comments_lift = if item.comments.is_some() {
        px_of(HISTORY_COMMENTS_BUTTON_HEIGHT)
    } else {
        px(0.0)
    };
    // shortcut: a failed message shows the sending clock; the red failed mark comes with sending in M4.
    let state = out.then_some(match (item.message.sending, item.is_selected) {
        (Some(Sending::Pending | Sending::Failed), _) => HISTORY_SENDING_ICON,
        (None, false) if item.is_read => HISTORY_RECEIVED_ICON,
        (None, true) if item.is_read => HISTORY_RECEIVED_SELECTED_ICON,
        (None, false) => HISTORY_SENT_ICON,
        (None, true) => HISTORY_SENT_SELECTED_ICON,
    });
    div()
        .absolute()
        .right(px_of(MSG_PADDING.right) - px_of(MSG_DATE_DELTA.x))
        .bottom(px_of(MSG_PADDING.bottom) - px_of(MSG_DATE_DELTA.y) + preview_lift + comments_lift)
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
