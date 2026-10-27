//! One history item (`HistoryView::Message`): the date badge over a new
//! day, then the message in its bubble.

use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, Corners, Div, ElementId, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement, Pixels, SharedString, StatefulInteractiveElement as _,
    Styled, Window, div, point, prelude::FluentBuilder as _, px,
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
    HISTORY_RECEIVED_INVERTED_ICON, HISTORY_RECEIVED_SELECTED_ICON, HISTORY_SENT_ICON,
    HISTORY_SENT_INVERTED_ICON, HISTORY_SENT_SELECTED_ICON, HISTORY_VIEWS_IN_ICON,
    HISTORY_VIEWS_INVERTED_ICON, HISTORY_VIEWS_OUT_ICON,
};
use style::dialogs::{HISTORY_SENDING_ICON, HISTORY_SENDING_INVERTED_ICON};
use style::{ColorId, ColorRef};
use ui::{Userpic, UserpicSource, color, font, font_height, icon, icon_size, peer_palette_index};

use super::body::{self, Body, OnLink};
use super::comments::Comments;
use super::headers::{self, Reply};
use super::selection::{OnPointer, Pointer};
use super::web_page::{self, WebPage};
use super::{layout, reactions, unread};
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
    pub reactions: Vec<reactions::Button>,
    /// The file row: one going up, or a document message's file.
    pub file: Option<super::file_row::FileRow>,
    /// A photo's, video's or album's pictures, once sent.
    pub photo: Option<super::album::Pictures>,
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

pub(super) fn palette(id: ColorId) -> ColorRef {
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

/// What ends a bubble on its own side at the bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Foot {
    Tail,
    /// A picture reaching the bottom skips the tail and keeps the corner
    /// round (`skipBubbleTail`).
    Media,
}

/// A bubble's corners: its own side gets the run's, small where it joins a
/// neighbor and none at the tail; the far side stays large.
pub(super) fn corners(out: bool, run: &Run, foot: Foot) -> Corners<Pixels> {
    let (large, small) = (px_of(BUBBLE_RADIUS_LARGE), px_of(BUBBLE_RADIUS_SMALL));
    let top = if run.is_attached_to_previous {
        small
    } else {
        large
    };
    let bottom = match (run.is_attached_to_next, foot) {
        (true, _) => small,
        (false, Foot::Tail) => px(0.0),
        (false, Foot::Media) => large,
    };
    let (top_left, top_right, bottom_left, bottom_right) = if out {
        (large, top, large, bottom)
    } else {
        (top, large, bottom, large)
    };
    Corners {
        top_left,
        top_right,
        bottom_right,
        bottom_left,
    }
}

/// A bubble's background, corners, shadow and tail for its side and run;
/// also the text color that goes on it.
pub(super) fn frame(
    out: bool,
    is_selected: bool,
    run: &Run,
    foot: Foot,
    cx: &App,
) -> (Div, ColorId) {
    let (bg, shadow, fg) = match (out, is_selected) {
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
    let has_tail = !run.is_attached_to_next && foot == Foot::Tail;
    let corners = corners(out, run, foot);
    let tail = has_tail.then(|| {
        let tail = match (out, is_selected) {
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
    let frame = div()
        .relative()
        .min_w_0()
        .max_w(px_of(MSG_MAX_WIDTH))
        .pt(px_of(MSG_PADDING.top))
        .pb(px_of(MSG_PADDING.bottom))
        .pl(px_of(MSG_PADDING.left))
        .pr(px_of(MSG_PADDING.right))
        .bg(color(cx, palette(bg)))
        .rounded_tl(corners.top_left)
        .rounded_tr(corners.top_right)
        .rounded_bl(corners.bottom_left)
        .rounded_br(corners.bottom_right)
        .shadow(vec![BoxShadow {
            color: color(cx, palette(shadow)),
            offset: point(px(0.0), px_of(MSG_SHADOW)),
            blur_radius: px(0.0),
            spread_radius: px(0.0),
            inset: false,
        }])
        .children(tail);
    (frame, fg)
}

fn bubble(item: &mut Item<'_>, out: bool, cx: &App) -> Div {
    if let Some(photo) = item.photo.take() {
        return super::photo::bubble(item, out, photo, cx);
    }
    let (frame, fg) = frame(out, item.is_selected, &item.run, Foot::Tail, cx);
    let name = name_line(item, out, cx);
    let info = info(item, out, cx);
    let room = info_content(item, out, InfoOn::Bubble, cx);
    let comments = item
        .comments
        .take()
        .map(|comments| super::comments::button(comments, item.message.id.0, item.is_selected, cx));
    let headers = headers(item, out, cx);
    let has_header = name.is_some() || !headers.is_empty();
    // Before the file is taken: with one, only the caption shows.
    let body = text(item, out, cx);
    let has_caption = !caption(item.message).is_some_and(|caption| caption.text.is_empty());
    // A file alone reaches the bubble's bottom, its status beside the time.
    let is_file_alone = !has_caption && item.reactions.is_empty() && comments.is_none();
    let file = item.file.take().map(|file| {
        let place = super::file_row::Place {
            id: item.message.id.0 as u64,
            is_out: out,
            is_selected: item.is_selected,
            has_header,
            time_room: is_file_alone.then(|| time_room(item, out, cx)),
        };
        super::file_row::row(file, &place, cx)
    });
    let shows_text = file.is_none() || has_caption;
    frame
        .children(name)
        .children(headers)
        .children(file)
        .children(web_page(item, out, Placement::AboveText, cx))
        .when(shows_text, |this| {
            this.child(text_style(fg, cx).child(body))
        })
        .children(web_page(item, out, Placement::BelowText, cx))
        .map(|this| {
            let reactions = std::mem::take(&mut item.reactions);
            if reactions.is_empty() {
                this
            } else {
                this.child(reactions::row(reactions, out, item.is_selected, room, cx))
            }
        })
        .child(info)
        .children(comments)
}

/// The sender's name and "via @bot" after it (`HistoryMessageVia`); the
/// via label stands alone when no name shows above a message.
// shortcut: a forwarded message's via label is dropped, tdesktop appends it to the forward line; with forward headers in M5.
pub(super) fn name_line(item: &Item<'_>, out: bool, cx: &App) -> Option<Div> {
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
pub(super) fn headers(item: &Item<'_>, out: bool, cx: &App) -> Vec<AnyElement> {
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
            room: (!is_above && item.reactions.is_empty()).then(|| time_room(item, out, cx)),
            on_link: item.on_link.clone(),
            ad: None,
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

/// The caption of a message with media.
pub(super) fn caption(message: &Message) -> Option<&FormattedText> {
    match &message.content {
        Content::Photo(media)
        | Content::Video(media)
        | Content::Animation(media)
        | Content::Document { media, .. } => Some(&media.caption),
        Content::Voice { caption, .. } => Some(caption),
        _ => None,
    }
}

/// The message text in its font and color.
pub(super) fn text_block(item: &Item<'_>, out: bool, fg: ColorId, cx: &App) -> Div {
    text_style(fg, cx).child(text(item, out, cx))
}

fn text_style(fg: ColorId, cx: &App) -> Div {
    div()
        .font(font(MSG_FONT))
        .text_size(px_of(MSG_FONT.size))
        .line_height(font_height(MSG_FONT))
        .text_color(color(cx, palette(fg)))
}

/// The message text with, made transparent, room for the time on the last
/// line (`skipBlock`).
// shortcut: figure spaces approximate the time's width in the text font; tdesktop reserves the exact width.
pub(super) fn text(item: &Item<'_>, out: bool, cx: &App) -> AnyElement {
    let preview;
    let text = match &item.message.content {
        Content::Text { text, .. } => text,
        // The picture or the file row stands for the file; only the
        // caption follows it.
        Content::Photo(media) | Content::Video(media) | Content::Animation(media) => &media.caption,
        Content::Document { media, .. } if item.file.is_some() => &media.caption,
        Content::Voice { caption, .. } if item.file.is_some() => caption,
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
    // Below the text the link preview or the reactions hold the room for the time.
    let has_preview_below = link_preview(item).is_some_and(|preview| !preview.is_above_text);
    let room = if has_preview_below || !item.reactions.is_empty() {
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

pub(super) fn info(item: &Item<'_>, out: bool, cx: &App) -> Div {
    // Over a link preview below the text the time sits on its last line.
    let preview_lift = if item.reactions.is_empty()
        && link_preview(item).is_some_and(|preview| !preview.is_above_text)
    {
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
    div()
        .absolute()
        .right(px_of(MSG_PADDING.right) - px_of(MSG_DATE_DELTA.x))
        .bottom(px_of(MSG_PADDING.bottom) - px_of(MSG_DATE_DELTA.y) + preview_lift + comments_lift)
        .child(info_content(item, out, InfoOn::Bubble, cx))
}

/// Where the time is drawn (`InfoDisplayType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InfoOn {
    Bubble,
    /// Over a photo, in white on its dark pill.
    Image,
}

/// The views, the time and the sending state.
pub(super) fn info_content(item: &Item<'_>, out: bool, on: InfoOn, cx: &App) -> Div {
    let date_fg = match (on, out, item.is_selected) {
        (InfoOn::Image, ..) => ColorId::MsgDateImgFg,
        (InfoOn::Bubble, true, false) => ColorId::MsgOutDateFg,
        (InfoOn::Bubble, true, true) => ColorId::MsgOutDateFgSelected,
        (InfoOn::Bubble, false, false) => ColorId::MsgInDateFg,
        (InfoOn::Bubble, false, true) => ColorId::MsgInDateFgSelected,
    };
    // shortcut: a failed message shows the sending clock; the red failed mark comes with sending in M4.
    let is_sending = matches!(
        item.message.sending,
        Some(Sending::Pending | Sending::Failed)
    );
    let state = out.then_some(match (on, is_sending, item.is_read, item.is_selected) {
        (InfoOn::Image, true, ..) => HISTORY_SENDING_INVERTED_ICON,
        (InfoOn::Image, false, true, _) => HISTORY_RECEIVED_INVERTED_ICON,
        (InfoOn::Image, false, false, _) => HISTORY_SENT_INVERTED_ICON,
        (InfoOn::Bubble, true, ..) => HISTORY_SENDING_ICON,
        (InfoOn::Bubble, false, true, false) => HISTORY_RECEIVED_ICON,
        (InfoOn::Bubble, false, true, true) => HISTORY_RECEIVED_SELECTED_ICON,
        (InfoOn::Bubble, false, false, false) => HISTORY_SENT_ICON,
        (InfoOn::Bubble, false, false, true) => HISTORY_SENT_SELECTED_ICON,
    });
    div()
        .flex()
        .items_center()
        .font(font(MSG_DATE_FONT))
        .text_size(px_of(MSG_DATE_FONT.size))
        .line_height(font_height(MSG_DATE_FONT))
        .text_color(color(cx, palette(date_fg)))
        .children(views(item).map(|views| {
            let views_icon = match (on, out) {
                (InfoOn::Image, _) => HISTORY_VIEWS_INVERTED_ICON,
                (InfoOn::Bubble, true) => HISTORY_VIEWS_OUT_ICON,
                (InfoOn::Bubble, false) => HISTORY_VIEWS_IN_ICON,
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
