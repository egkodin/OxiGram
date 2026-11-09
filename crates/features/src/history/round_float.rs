//! The round video playing off screen, in the corner of the history
//! (`Media::Player::Float`): its frame in a circle with the played part
//! around the edge; a click pauses and resumes, a double click scrolls
//! back to the message.

use gpui::{
    AnyElement, BoxShadow, ClickEvent, Context, Corners, InteractiveElement as _, IntoElement as _,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, point, px,
};
use style::chat::HISTORY_VIDEO_MESSAGE_PROGRESS_OPACITY;
use style::media_player::{MEDIA_PLAYER_FLOAT_MARGIN, MEDIA_PLAYER_FLOAT_SIZE};
use style::{ColorId, ColorRef};
use ui::color;

use super::HistoryView;
use crate::px_of;

impl HistoryView {
    /// The corner circle while a round plays off screen.
    // shortcut: always the top right corner; tdesktop drags it to any corner or off to close it.
    pub(super) fn round_float(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (id, frame, progress) = self.floating_round(cx)?;
        let side = px_of(MEDIA_PLAYER_FLOAT_SIZE);
        let margin = px_of(MEDIA_PLAYER_FLOAT_MARGIN);
        let fg = color(
            cx,
            ColorRef::Palette(ColorId::HistoryVideoMessageProgressFg),
        );
        #[expect(clippy::cast_possible_truncation, reason = "an opacity")]
        let opacity = HISTORY_VIDEO_MESSAGE_PROGRESS_OPACITY as f32;
        let picture = frame.map(|frame| {
            super::photo::playing(frame, Corners::all(side / 2.0))
                .absolute()
                .inset_0()
        });
        let weak = cx.weak_entity();
        let element = div()
            .id("round-float")
            .absolute()
            .top(margin)
            .right(margin)
            .size(side)
            .rounded_full()
            .bg(color(cx, ColorRef::Palette(ColorId::ImageBg)))
            .shadow(vec![BoxShadow {
                color: color(cx, ColorRef::Palette(ColorId::HistoryToDownShadow)),
                offset: point(px(0.0), px(1.0)),
                blur_radius: margin,
                spread_radius: px(0.0),
                inset: false,
            }])
            .cursor_pointer()
            .children(picture)
            .child(super::file_row::arc(progress, fg.opacity(opacity)))
            .on_click(move |event: &ClickEvent, _, cx| {
                cx.stop_propagation();
                let _ = weak.update(cx, |this, cx| {
                    if event.click_count() > 1 {
                        this.jump_to(id, cx);
                    } else {
                        this.toggle_inline(id, cx);
                    }
                });
            });
        Some(element.into_any_element())
    }
}
