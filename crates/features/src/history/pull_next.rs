//! "Pull to next channel" (`HistoryView::PullToNextChannel`): scrolling on
//! past the bottom of a channel grows a capsule with an arrow; far enough,
//! it shows the next unread channel, and letting go there opens it.

use std::time::Duration;

use gpui::{
    AnyElement, Bounds, Context, Hsla, IntoElement as _, ParentElement as _, PathBuilder, Pixels,
    ScrollEvent, Styled as _, TouchPhase, Window, canvas, div, point, px,
};
use i18n::key;
use model::ChatId;
use model::chat::{ChatKind, ChatList};
use style::chat::HISTORY_SCROLL;
use style::chat_helpers::{
    HISTORY_PULL_NEXT_ARROW_RAISE, HISTORY_PULL_NEXT_ARROW_SIZE, HISTORY_PULL_NEXT_ARROW_STROKE,
    HISTORY_PULL_NEXT_AVATAR, HISTORY_PULL_NEXT_EXPAND, HISTORY_PULL_NEXT_INSET,
    HISTORY_PULL_NEXT_MAX_HEIGHT, HISTORY_PULL_NEXT_NAME_FONT, HISTORY_PULL_NEXT_NAME_RADIUS,
    HISTORY_PULL_NEXT_PADDING, HISTORY_PULL_NEXT_RISE, HISTORY_PULL_NEXT_SKIP,
    HISTORY_PULL_NEXT_THRESHOLD,
};
use style::{ColorId, ColorRef};
use ui::{Userpic, color, font, font_height};

use super::{HistoryEvent, HistoryView, pinned_section};
use crate::dialogs::{Lit, photo_userpic, unread_badge};
use crate::lang::tr;
use crate::px_of;

/// A wheel has no lift, so a pause this long lets go of the pull.
const RELEASE_AFTER: Duration = Duration::from_millis(250);
/// The list scrolls this far per wheel line, as gpui's list does.
const LINE_HEIGHT: f32 = 20.0;
/// Counts above this read "999+".
const MAX_COUNT: i32 = 999;

fn len(value: style::Px) -> f32 {
    f32::from(px_of(value))
}

/// How far past the bottom opens the next channel (`PullThreshold`).
fn threshold() -> f32 {
    len(HISTORY_PULL_NEXT_THRESHOLD) * 2.0
}

/// The pull in progress.
#[derive(Debug, Default)]
pub(super) struct Pull {
    /// How far past the bottom, in pixels; 0 when not pulling.
    amount: f32,
    /// The channel letting go would open, found when the pull starts.
    next: Option<ChatId>,
    /// Counts scroll events, so only a pause after the last lets go.
    events: u64,
    /// After letting go, until the scrolling stops, so momentum does not
    /// start another pull.
    is_settling: bool,
}

impl Pull {
    fn is_ready(&self) -> bool {
        self.amount >= threshold()
    }

    /// How tall the indicator is: the pull, grown to the card once ready.
    // shortcut: the card grows at once; tdesktop animates the expand, the release and a bounce.
    fn shown(&self) -> f32 {
        let card = len(HISTORY_PULL_NEXT_EXPAND);
        let grown = if self.is_ready() {
            self.amount + card - threshold()
        } else {
            self.amount
        };
        grown.min(len(HISTORY_PULL_NEXT_MAX_HEIGHT))
    }

    /// Takes a scroll `delta` (up is positive); false when it ends the pull.
    fn apply(&mut self, delta: f32) -> bool {
        self.amount = (self.amount - delta).clamp(0.0, len(HISTORY_PULL_NEXT_MAX_HEIGHT));
        self.amount > 0.0
    }
}

impl HistoryView {
    /// The open chat is a channel read to its end.
    fn can_pull(&self, cx: &Context<Self>) -> bool {
        let state = self.store.read(cx).state();
        let is_channel = state.chats.chat(self.chat).is_some_and(|chat| {
            matches!(
                chat.kind,
                ChatKind::Supergroup {
                    is_channel: true,
                    ..
                }
            )
        });
        let has_newest = state
            .history(self.key())
            .is_some_and(model::history::History::has_newest);
        is_channel
            && self.topic.is_none()
            && self.section == pinned_section::Section::Chat
            && has_newest
            && self.below_view() < px(1.0)
    }

    /// The first unread channel other than this one, in the main list,
    /// then the folders, then the archive (`FindNextUnreadChannel`).
    // shortcut: the open folder does not come first; history does not know which folder the chat list shows.
    fn next_unread_channel(&self, cx: &Context<Self>) -> Option<ChatId> {
        let chats = &self.store.read(cx).state().chats;
        let lists = std::iter::once(ChatList::Main)
            .chain(
                chats
                    .folders()
                    .iter()
                    .map(|folder| ChatList::Folder(folder.id)),
            )
            .chain(std::iter::once(ChatList::Archive));
        lists
            .flat_map(|list| chats.list(list))
            .filter(|&id| id != self.chat)
            .find(|&id| {
                chats.chat(id).is_some_and(|chat| {
                    chat.unread_count > 0
                        && matches!(
                            chat.kind,
                            ChatKind::Supergroup {
                                is_channel: true,
                                ..
                            }
                        )
                })
            })
    }

    /// Scrolling over the history: grows or shrinks the pull.
    pub(super) fn on_pull_scroll(&mut self, event: &ScrollEvent, cx: &mut Context<Self>) {
        let delta = f32::from(event.delta.pixel_delta(px(LINE_HEIGHT)).y);
        self.pull.events += 1;
        let is_idle = !self.pull.is_settling && self.pull.amount <= 0.0;
        if is_idle && (delta >= 0.0 || !self.can_pull(cx)) {
            return;
        }
        self.release_later(cx);
        if self.pull.is_settling {
            return;
        }
        if is_idle {
            self.pull.next = self.next_unread_channel(cx);
        } else if delta > 0.0 {
            // The list moved up with this scroll; the pull takes it instead.
            self.list.scroll_to_end();
        }
        if !self.pull.apply(delta) {
            self.pull = Pull {
                events: self.pull.events,
                ..Pull::default()
            };
        } else if event.touch_phase == TouchPhase::Ended {
            self.release_pull(cx);
        }
        cx.notify();
    }

    fn release_later(&mut self, cx: &mut Context<Self>) {
        let events = self.pull.events;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RELEASE_AFTER).await;
            let _ = this.update(cx, |this, cx| {
                if this.pull.events != events {
                    return;
                }
                if this.pull.amount > 0.0 {
                    this.release_pull(cx);
                }
                this.pull.is_settling = false;
            });
        })
        .detach();
    }

    /// Letting go: opens the next channel when the pull reached it.
    fn release_pull(&mut self, cx: &mut Context<Self>) {
        let next = self.pull.next.filter(|_| self.pull.is_ready());
        self.pull = Pull {
            is_settling: true,
            events: self.pull.events,
            ..Pull::default()
        };
        if let Some(next) = next {
            cx.emit(HistoryEvent::OpenChat(next));
        }
        cx.notify();
    }

    /// How far the list moves up for the pull.
    pub(super) fn pull_shift(&self) -> Pixels {
        px(self.pull.shown())
    }

    /// The capsule, or the next channel's userpic and name, under the list.
    // shortcut: no "Pull up" / "Release" hint over the compose area, which comes with the compose bar (M4).
    pub(super) fn pull_indicator(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let offset = self.pull.shown();
        if offset <= 0.0 {
            return None;
        }
        let height = len(HISTORY_PULL_NEXT_MAX_HEIGHT);
        let layer = div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .h(px(height))
            .pr(px_of(HISTORY_SCROLL.width));
        let card = len(HISTORY_PULL_NEXT_EXPAND);
        let progress = (offset / card).clamp(0.0, 1.0);
        let parts = if self.pull.is_ready() {
            self.ready_parts(height, offset, cx)
        } else {
            self.pulling_parts(height, offset, progress, cx)
        };
        Some(layer.children(parts).opacity(progress).into_any_element())
    }

    /// The capsule with the arrow, and the userpic growing in it.
    fn pulling_parts(
        &mut self,
        height: f32,
        offset: f32,
        progress: f32,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let skip = len(HISTORY_PULL_NEXT_SKIP);
        let radius = len(HISTORY_PULL_NEXT_AVATAR) / 2.0;
        let card = len(HISTORY_PULL_NEXT_EXPAND);
        let width_radius = (radius * (offset - skip) / (card - skip)).clamp(0.0, radius);
        let (bg, fg) = service_colors(cx);
        let capsule_top = height - offset;
        let capsule = centered(capsule_top).child(
            div()
                .w(px(2.0 * width_radius))
                .h(px((offset - skip).max(0.0)))
                .rounded(px(width_radius))
                .bg(bg)
                .overflow_hidden()
                .child(arrow(progress, fg)),
        );
        let inner = (radius * progress)
            .min(offset / 2.0 - skip * progress)
            .max(0.0);
        let size = inner * 2.0 - len(HISTORY_PULL_NEXT_INSET);
        let mut parts = vec![capsule.into_any_element()];
        if size >= 1.0 {
            let top = height - skip - skip * progress - size;
            parts.push(
                centered(top)
                    .child(self.next_face(size, false, cx))
                    .into_any_element(),
            );
        }
        parts
    }

    /// The next channel's userpic with its unread count, and its name.
    fn ready_parts(&mut self, height: f32, offset: f32, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let card = len(HISTORY_PULL_NEXT_EXPAND);
        let padding = len(HISTORY_PULL_NEXT_PADDING);
        let avatar = len(HISTORY_PULL_NEXT_AVATAR);
        let face = self.next_face(avatar, true, cx);
        let name = match self
            .pull
            .next
            .and_then(|id| self.store.read(cx).state().chats.chat(id))
        {
            Some(chat) => chat.title.clone(),
            None => tr(cx, key::lng_pull_no_unread_channels).to_string(),
        };
        let (bg, fg) = service_colors(cx);
        let skip = px_of(HISTORY_PULL_NEXT_SKIP);
        let name_font = HISTORY_PULL_NEXT_NAME_FONT;
        let pill = div()
            .max_w_full()
            .px(skip)
            .pb(skip - px_of(HISTORY_PULL_NEXT_PADDING))
            .pt(px_of(HISTORY_PULL_NEXT_PADDING))
            .rounded(px_of(HISTORY_PULL_NEXT_NAME_RADIUS))
            .bg(bg)
            .text_color(fg)
            .font(font(name_font))
            .text_size(px_of(name_font.size))
            .line_height(font_height(name_font))
            .truncate()
            .child(name);
        let name_top = height - len(HISTORY_PULL_NEXT_RISE) - padding;
        vec![
            centered(height - offset.min(card) + padding)
                .child(face)
                .into_any_element(),
            centered(name_top)
                .px(px_of(HISTORY_PULL_NEXT_SKIP) * 2.0)
                .child(pill)
                .into_any_element(),
        ]
    }

    /// The next channel's userpic at `size`, with its unread count once
    /// ready; a check when every channel is read.
    fn next_face(&mut self, size: f32, has_badge: bool, cx: &mut Context<Self>) -> AnyElement {
        let state = self.store.read(cx).state();
        let Some(chat) = self.pull.next.and_then(|id| state.chats.chat(id)) else {
            let (bg, fg) = service_colors(cx);
            return div()
                .size(px(size))
                .rounded_full()
                .bg(bg)
                .child(check(size, fg))
                .into_any_element();
        };
        let count = chat.unread_count;
        let (source, file) = photo_userpic(
            &state.chats,
            chat.photo.as_ref(),
            chat.accent_color,
            &chat.title,
        );
        if let Some(file) = file {
            self.request_file(file, cx);
        }
        let text = if count > MAX_COUNT {
            format!("{MAX_COUNT}+")
        } else {
            count.to_string()
        };
        let badge_height = len(style::chat_helpers::HISTORY_PULL_NEXT_BADGE);
        div()
            .relative()
            .size(px(size))
            .child(Userpic::new(px(size), source))
            .children((has_badge && count > 0).then(|| {
                div()
                    .absolute()
                    .top(px(-badge_height / 4.0))
                    .right(px(-badge_height / 3.0))
                    .child(unread_badge(text.into(), false, Lit::Normal, cx))
            }))
            .into_any_element()
    }
}

/// A full-width row at `top`, centering what it holds.
fn centered(top: f32) -> gpui::Div {
    div()
        .absolute()
        .left_0()
        .right_0()
        .top(px(top))
        .flex()
        .justify_center()
}

fn service_colors(cx: &Context<HistoryView>) -> (Hsla, Hsla) {
    (
        color(cx, ColorRef::Palette(ColorId::MsgServiceBg)),
        color(cx, ColorRef::Palette(ColorId::MsgServiceFg)),
    )
}

/// The up arrow in the capsule (`PaintArrow`), drawn in a 24 px box that
/// grows with the pull.
fn arrow(progress: f32, fg: Hsla) -> AnyElement {
    let size = len(HISTORY_PULL_NEXT_ARROW_SIZE);
    let skip = len(HISTORY_PULL_NEXT_SKIP);
    // From the capsule's top; tdesktop measures from the bottom of the area.
    let center_y = size + skip * (1.0 - progress) - len(HISTORY_PULL_NEXT_ARROW_RAISE);
    let stroke = len(HISTORY_PULL_NEXT_ARROW_STROKE) * progress;
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, (), window: &mut Window, _| {
            let center = point(
                bounds.origin.x + bounds.size.width / 2.0,
                bounds.origin.y + px(center_y),
            );
            let at = |x: f32, y: f32| {
                point(
                    center.x + px((x - 12.0) * progress),
                    center.y + px((y + 8.0) * progress),
                )
            };
            stroke_lines(window, &[at(12.5, 4.0), at(12.5, 22.0)], stroke, fg);
            stroke_lines(
                window,
                &[at(3.5, 12.0), at(12.5, 3.5), at(21.5, 12.0)],
                stroke,
                fg,
            );
        },
    )
    .size_full()
    .into_any_element()
}

/// The check on an empty circle when no channel is left (`PaintCheck`).
fn check(size: f32, fg: Hsla) -> AnyElement {
    let stroke = len(HISTORY_PULL_NEXT_ARROW_STROKE);
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, (), window: &mut Window, _| {
            let center = bounds.center();
            let s = size / 2.0 / 2.4;
            let at = |x: f32, y: f32| point(center.x + px(x), center.y + px(y));
            stroke_lines(
                window,
                &[at(-s, 0.0), at(-s / 3.0, s * 0.7), at(s, -s * 0.7)],
                stroke,
                fg,
            );
        },
    )
    .size_full()
    .into_any_element()
}

fn stroke_lines(window: &mut Window, points: &[gpui::Point<Pixels>], width: f32, fg: Hsla) {
    if width <= 0.0 {
        return;
    }
    let mut builder = PathBuilder::stroke(px(width));
    builder.add_polygon(points, false);
    if let Ok(path) = builder.build() {
        window.paint_path(path, fg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pull_is_ready_past_the_threshold_and_grows_to_the_card() {
        let mut pull = Pull::default();
        assert!(pull.apply(-50.0));
        assert!(!pull.is_ready());
        assert!((pull.shown() - 50.0).abs() < f32::EPSILON);
        assert!(pull.apply(-50.0));
        assert!(pull.is_ready(), "100 px is past the 96 px threshold");
        let card = len(HISTORY_PULL_NEXT_EXPAND);
        assert!((pull.shown() - (100.0 + card - threshold())).abs() < f32::EPSILON);
    }

    #[test]
    fn scrolling_back_ends_the_pull_and_it_stays_capped() {
        let mut pull = Pull::default();
        assert!(pull.apply(-1000.0));
        let max = len(HISTORY_PULL_NEXT_MAX_HEIGHT);
        assert!((pull.amount - max).abs() < f32::EPSILON);
        assert!(!pull.apply(1000.0), "scrolling back past the start ends it");
    }
}
