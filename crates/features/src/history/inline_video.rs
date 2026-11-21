//! Round videos and GIFs playing in their bubbles (`HistoryView::Gif` with
//! its streamed instance): one [`VideoPlayback`] per message on screen,
//! its frames painted in place of the still.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{App, AppContext as _, Context, Entity, RenderImage, Subscription, Window};
use model::MessageId;
use ui::RadialState;

use super::HistoryView;
use super::media_video::{self, Ending, VideoPlayback};
use crate::player::{RoundLink, Sound, Track};
use crate::streaming::Playable;

/// The videos playing in a history.
#[derive(Default)]
pub(super) struct Inline {
    playing: HashMap<MessageId, (Entity<VideoPlayback>, Subscription)>,
    /// The messages whose frames the last layout asked for: the rest are
    /// off screen and stop, except a round, which plays on in the corner.
    seen: HashSet<MessageId>,
    /// The round playing off screen (`Media::Player::Float`).
    floating: Option<MessageId>,
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
    /// The buffering arc while the data is late.
    pub waiting: Option<RadialState>,
}

impl HistoryView {
    /// The window's scale, as the last frame drew it.
    pub(super) fn display_scale(&self) -> f32 {
        self.inline.scale
    }

    /// Stops what scrolled away or ended, and keeps the rest moving; a
    /// round off screen plays on in the corner.
    pub(super) fn tick_inline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let inline = &mut self.inline;
        inline.scale = window.scale_factor();
        let seen = std::mem::take(&mut inline.seen);
        inline.playing.retain(|id, (playback, _)| {
            let playback = playback.read(cx);
            if playback.has_failed() {
                inline.failed.insert(*id);
            }
            let is_kept = seen.contains(id) || playback.plays_away();
            is_kept && !playback.is_finished() && !playback.is_released()
        });
        inline.floating = None;
        for (id, (playback, _)) in &inline.playing {
            if !seen.contains(id) && playback.read(cx).plays_away() {
                inline.floating = Some(*id);
                // Nothing draws it in the list: it runs the player's commands here.
                playback.update(cx, |playback, cx| playback.advance(cx));
            }
        }
        media_video::drop_retired(window, cx);
        crate::lottie::drop_retired(window, cx);
        if inline
            .playing
            .values()
            .any(|(playback, _)| playback.read(cx).needs_redraw())
        {
            window.request_animation_frame();
        }
    }

    /// The round playing off screen, its frame and played part.
    pub(super) fn floating_round(
        &self,
        cx: &App,
    ) -> Option<(MessageId, Option<Arc<RenderImage>>, f32)> {
        let id = self.inline.floating?;
        let (playback, _) = self.inline.playing.get(&id)?;
        let playback = playback.read(cx);
        Some((id, playback.frame(), playback.progress()))
    }

    /// Plays a round in its bubble through the app's player, as tdesktop
    /// plays rounds as voices: it stops what played, and the bar shows it.
    pub(super) fn play_round(
        &mut self,
        message: MessageId,
        file: &Playable,
        (side, length): (f32, Duration),
        track: Track,
        can_follow: bool,
        cx: &mut Context<Self>,
    ) {
        let link = Rc::new(RoundLink::default());
        let playlist = can_follow.then(|| self.voice_playlist(None, cx)).flatten();
        self.play(
            message,
            (track, playlist),
            Sound::Round(Rc::clone(&link)),
            cx,
        );
        self.play_inline(message, file, ((side, side), length), Ending::Pause, cx);
        if let Some((playback, _)) = self.inline.playing.get(&message) {
            playback.update(cx, |playback, _| playback.link(link));
        }
    }

    /// Jumps a round to `share` of the way in.
    pub(super) fn seek_inline(&mut self, message: MessageId, share: f32, cx: &mut Context<Self>) {
        if let Some((playback, _)) = self.inline.playing.get(&message) {
            playback.update(cx, |playback, cx| playback.seek_share(share, cx));
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
                waiting: playback.spinner().map(|(_, state)| state),
            }
        }))
    }

    /// Starts `file` in `message`'s bubble, `size` points large; streamed
    /// while it downloads.
    pub(super) fn play_inline(
        &mut self,
        message: MessageId,
        file: &Playable,
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
            cx.new(|cx| VideoPlayback::new(file, Duration::ZERO, frames, duration, ending, cx));
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
        let file = Playable::Complete(path.to_path_buf());
        self.play_inline(message, &file, (size, Duration::ZERO), Ending::Loop, cx);
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
