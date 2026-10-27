//! The comments button under a channel post (`Message::paintCommentsButton`):
//! who commented last, "12 comments" or "Leave a comment", and an arrow.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};
use i18n::key;
use model::message::Message;
use style::basic::SEMIBOLD_FONT;
use style::chat::{
    HISTORY_COMMENTS_BUTTON_HEIGHT, HISTORY_COMMENTS_SKIP_LEFT, HISTORY_COMMENTS_SKIP_RIGHT,
    HISTORY_COMMENTS_SKIP_TEXT, MEDIA_UNREAD_SIZE, MEDIA_UNREAD_SKIP, MSG_PADDING,
};
use style::chat_style::{
    HISTORY_COMMENTS_IN, HISTORY_COMMENTS_IN_SELECTED, HISTORY_COMMENTS_OPEN_IN,
    HISTORY_COMMENTS_OPEN_IN_SELECTED,
};
use style::{ColorId, ColorRef};
use ui::{Userpic, UserpicSource, color, font, font_height, icon};

use super::bubble::OnClick;
use super::{HistoryView, layout, sender_face};
use crate::lang::strings;
use crate::px_of;

/// tdesktop shows at most three who commented (`kMaxRecentRepliers`).
const MAX_REPLIERS: usize = 3;
/// `historyCommentsUserpics`: each userpic covers this much of the one
/// before, with a ring this wide cut around it.
const USERPIC_SIZE: f32 = 25.0;
const USERPIC_SHIFT: f32 = 6.0;
const USERPIC_STROKE: f32 = 2.0;
/// The line over the button, faded (`p.setOpacity(0.3)`).
const LINE_OPACITY: f32 = 0.3;

/// The button, ready to draw.
pub(super) struct Comments {
    pub text: String,
    /// Who commented last, newest first.
    pub repliers: Vec<UserpicSource>,
    pub has_unread: bool,
    pub on_click: OnClick,
}

impl HistoryView {
    /// The button for a channel post with a discussion group; none in the
    /// thread itself.
    pub(super) fn comments(
        &mut self,
        message: &Message,
        cx: &mut Context<Self>,
    ) -> Option<Comments> {
        let replies = message
            .replies
            .as_ref()
            .filter(|_| message.is_channel_post && self.thread.is_none())?;
        let state = self.store.read(cx).state();
        let text = if replies.count > 0 {
            i18n::fill(
                strings(cx).plural(key::lng_comments_open_count, i64::from(replies.count)),
                &[("count", &layout::short_count(replies.count))],
            )
        } else {
            strings(cx).text(key::lng_comments_open_none).to_owned()
        };
        let faces: Vec<_> = replies
            .recent
            .iter()
            .take(MAX_REPLIERS)
            .filter_map(|&sender| sender_face(&state.chats, sender))
            .map(|(_, _, face)| face)
            .collect();
        let has_unread = replies.has_unread();
        let mut repliers = Vec::with_capacity(faces.len());
        for (userpic, file) in faces {
            if let Some(file) = file {
                self.request_file(file, cx);
            }
            repliers.push(userpic);
        }
        let (post, weak) = (message.id, cx.weak_entity());
        Some(Comments {
            text,
            repliers,
            has_unread,
            on_click: Rc::new(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.open_comments(post, cx));
            }),
        })
    }
}

/// The button across the bubble's bottom, under its text and time.
// shortcut: no ripple on press and no hover; ripples come with M8 polish.
pub(super) fn button(comments: Comments, id: i64, is_selected: bool, cx: &App) -> AnyElement {
    let (bg, link, comment_icon, open_icon) = if is_selected {
        (
            ColorId::MsgInBgSelected,
            ColorId::MsgFileThumbLinkInFgSelected,
            HISTORY_COMMENTS_IN_SELECTED,
            HISTORY_COMMENTS_OPEN_IN_SELECTED,
        )
    } else {
        (
            ColorId::MsgInBg,
            ColorId::MsgFileThumbLinkInFg,
            HISTORY_COMMENTS_IN,
            HISTORY_COMMENTS_OPEN_IN,
        )
    };
    let lead = if comments.repliers.is_empty() {
        div().flex_none().child(icon(comment_icon))
    } else {
        repliers(comments.repliers, color(cx, ColorRef::Palette(bg)))
    };
    let on_click = comments.on_click;
    div()
        .id(("history-comments", u64::from_ne_bytes(id.to_ne_bytes())))
        .flex()
        .items_center()
        .h(px_of(HISTORY_COMMENTS_BUTTON_HEIGHT))
        .mt(px_of(MSG_PADDING.bottom))
        .mb(-px_of(MSG_PADDING.bottom))
        .ml(-px_of(MSG_PADDING.left))
        .mr(-px_of(MSG_PADDING.right))
        .pl(px_of(HISTORY_COMMENTS_SKIP_LEFT))
        .pr(px_of(HISTORY_COMMENTS_SKIP_RIGHT))
        .border_t_1()
        .border_color(color(cx, ColorRef::Palette(ColorId::MsgInDateFg)).opacity(LINE_OPACITY))
        .cursor_pointer()
        .on_click(move |event, window, cx| on_click(event, window, cx))
        .child(lead)
        .child(
            div()
                .ml(px_of(HISTORY_COMMENTS_SKIP_TEXT))
                .min_w_0()
                .truncate()
                .font(font(SEMIBOLD_FONT))
                .text_size(px_of(SEMIBOLD_FONT.size))
                .line_height(font_height(SEMIBOLD_FONT))
                .text_color(color(cx, ColorRef::Palette(link)))
                .child(comments.text),
        )
        .when(comments.has_unread, |this| {
            this.child(
                div()
                    .flex_none()
                    .ml(px_of(MEDIA_UNREAD_SKIP))
                    .size(px_of(MEDIA_UNREAD_SIZE))
                    .rounded_full()
                    .bg(color(cx, ColorRef::Palette(ColorId::MsgFileInBg))),
            )
        })
        .child(div().flex_1())
        .child(div().flex_none().child(icon(open_icon)))
        .into_any_element()
}

/// The userpics overlapping left to right, the newest on top.
fn repliers(userpics: Vec<UserpicSource>, ring: gpui::Hsla) -> gpui::Div {
    let count = userpics.len();
    // Reversed so the newest paints last, over the others.
    div().flex_none().flex().flex_row_reverse().children(
        userpics
            .into_iter()
            .enumerate()
            .rev()
            .map(move |(index, userpic)| {
                div()
                    .relative()
                    .size(px(USERPIC_SIZE))
                    .when(index + 1 < count, |this| this.mr(px(-USERPIC_SHIFT)))
                    .child(Userpic::new(px(USERPIC_SIZE), userpic))
                    .child(
                        div()
                            .absolute()
                            .inset(px(-USERPIC_STROKE / 2.0))
                            .rounded_full()
                            .border(px(USERPIC_STROKE))
                            .border_color(ring),
                    )
            }),
    )
}
