//! Round videos and GIFs playing in their bubbles (`HistoryView::Gif` with
//! its streamed instance): one [`VideoPlayback`] per message on screen,
//! its frames painted in place of the still.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use gpui::{AppContext as _, Context, Entity, RenderImage, Subscription, Window};
use model::MessageId;

use super::HistoryView;
use super::media_video::{self, Ending, VideoPlayback};

/// The videos playing in a history.
// shortcut: a round stops when it scrolls away; tdesktop moves it to a corner (row "Round video PiP"), with the audio step.
#[derive(Default)]
pub(super) struct Inline {
    playing: HashMap<MessageId, (Entity<VideoPlayback>, Subscription)>,
    /// The messages whose frames the last layout asked for: the rest are
    /// off screen and stop.
    seen: HashSet<MessageId>,
    /// GIFs and stickers that cannot be decoded, not to start again.
    failed: HashSet<MessageId>,
    scale: f32,
}

/// What a playing message shows this frame.
pub(super) struct Frame {
    pub image: Option<Arc<RenderImage>>,
    pub progress: f32,
    pub remaining: Duration,
    pub is_playing: bool,
}

impl HistoryView {
    /// Stops what scrolled away or ended, and keeps the rest moving.
    pub(super) fn tick_inline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let inline = &mut self.inline;
        inline.scale = window.scale_factor();
        let seen = std::mem::take(&mut inline.seen);
        inline.playing.retain(|id, (playback, _)| {
            let playback = playback.read(cx);
            if playback.has_failed() {
                inline.failed.insert(*id);
            }
            seen.contains(id) && !playback.is_finished()
        });
        media_video::drop_retired(window, cx);
        if inline
            .playing
            .values()
            .any(|(playback, _)| playback.read(cx).needs_redraw())
        {
            window.request_animation_frame();
        }
    }

    /// The frame for `message` if it plays here; it then counts as on screen.
    pub(super) fn inline_frame(
        &mut self,
        message: MessageId,
        cx: &mut Context<Self>,
    ) -> Option<Frame> {
        let (playback, _) = self.inline.playing.get(&message)?;
        self.inline.seen.insert(message);
        Some(playback.update(cx, |playback, cx| {
            playback.advance(cx);
            Frame {
                image: playback.frame(),
                progress: playback.progress(),
                remaining: playback.remaining(),
                is_playing: playback.is_playing(),
            }
        }))
    }

    /// Starts `path` in `message`'s bubble, `size` points large.
    pub(super) fn play_inline(
        &mut self,
        message: MessageId,
        path: &Path,
        (size, duration): ((f32, f32), Duration),
        ending: Ending,
        cx: &mut Context<Self>,
    ) {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a positive size in pixels"
        )]
        let pixels = |side: f32| (side * self.inline.scale.max(1.0)).ceil().max(1.0) as u32;
        let frames = (pixels(size.0), pixels(size.1));
        let playback =
            cx.new(|cx| VideoPlayback::new(path, Duration::ZERO, frames, duration, ending, cx));
        let subscription = cx.observe(&playback, |_, _, cx| cx.notify());
        self.inline.seen.insert(message);
        self.inline
            .playing
            .insert(message, (playback, subscription));
        cx.notify();
    }

    /// Plays a GIF or a video sticker on disk muted and looped
    /// (`autoplayGifs`), unless it already plays or cannot.
    pub(super) fn autoplay(
        &mut self,
        message: MessageId,
        path: &Path,
        size: (f32, f32),
        cx: &mut Context<Self>,
    ) {
        if self.inline.playing.contains_key(&message) || self.inline.failed.contains(&message) {
            return;
        }
        self.play_inline(message, path, (size, Duration::ZERO), Ending::Loop, cx);
    }

    /// Pauses or resumes `message`'s video; false when it does not play here.
    pub(super) fn toggle_inline(&mut self, message: MessageId, cx: &mut Context<Self>) -> bool {
        let Some((playback, _)) = self.inline.playing.get(&message) else {
            return false;
        };
        playback.update(cx, |playback, cx| playback.toggle(cx));
        true
    }
}
