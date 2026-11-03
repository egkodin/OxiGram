//! A video playing (`Media::Streaming::Instance`): frames from
//! [`media::VideoDecoder`] shown on the wall clock. The viewer renders it
//! with the controller (`Media::View::PlaybackControls`): the play button,
//! the times and the seek bar; a history paints its frames in the bubble.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, Corners, Global, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement as _, Pixels, Render, RenderImage, Styled as _,
    Window, canvas, div, px,
};
use media::{AudioError, AudioPlayer, Output, VideoDecoder, VideoFrame};
use style::media_view::{
    MEDIAVIEW_CONTROLLER_RADIUS, MEDIAVIEW_CONTROLLER_SIZE, MEDIAVIEW_PAUSE_ICON,
    MEDIAVIEW_PAUSE_ICON_OVER, MEDIAVIEW_PLAY_BUTTON, MEDIAVIEW_PLAY_BUTTON_TOP,
    MEDIAVIEW_PLAY_PROGRESS_LABEL, MEDIAVIEW_PLAY_PROGRESS_LEFT, MEDIAVIEW_PLAY_PROGRESS_SKIP,
    MEDIAVIEW_PLAY_PROGRESS_TOP, MEDIAVIEW_PLAYBACK, MEDIAVIEW_PLAYBACK_TOP,
};
use style::{ColorId, ColorRef};
use ui::{IconButton, color, font, font_height};

use super::video::duration_text;
use crate::px_of;

static MEDIAVIEW_PAUSE_BUTTON: style::widgets::IconButton = style::widgets::IconButton {
    icon: MEDIAVIEW_PAUSE_ICON,
    icon_over: MEDIAVIEW_PAUSE_ICON_OVER,
    ..MEDIAVIEW_PLAY_BUTTON
};

/// Room the time labels take beside the seek bar ("-10:00" in 12px bold).
const LABEL_WIDTH: f32 = 40.0;

/// The playback clock: where the video is, moving or not.
#[derive(Debug, Clone, Copy)]
enum Clock {
    Playing { since: Instant, from: Duration },
    Paused(Duration),
}

impl Clock {
    fn now(self) -> Duration {
        match self {
            Self::Playing { since, from } => from + since.elapsed(),
            Self::Paused(at) => at,
        }
    }
}

/// Frames taken off screen, for the next window that can drop them from
/// its atlas (gpui keeps a texture until `drop_image`).
#[derive(Default)]
struct Retired(Vec<Arc<RenderImage>>);

impl Global for Retired {}

/// What happens when the video reaches its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ending {
    /// Stops on the last frame.
    Pause,
    /// Starts over, as GIFs do.
    Loop,
}

impl Ending {
    /// Whether the video starts over at its end; a failed decoder is gone,
    /// so nothing does.
    fn restarts(self, has_failed: bool) -> bool {
        self == Self::Loop && !has_failed
    }
}

pub(super) struct VideoPlayback {
    decoder: Option<VideoDecoder>,
    /// The sound track, kept in step on pause and seek.
    sound: Option<AudioPlayer>,
    ending: Ending,
    duration: Duration,
    clock: Clock,
    shown: Option<Arc<RenderImage>>,
    /// The next frame, held until its time.
    pending: Option<VideoFrame>,
    /// Waiting for a first frame after opening or a seek.
    needs_frame: bool,
    is_ended: bool,
    has_failed: bool,
    is_dragging: bool,
}

impl VideoPlayback {
    /// Starts `path` at `start`, playing, in frames no bigger than `size`;
    /// with its sound unless it loops, as muted GIFs and stickers do.
    // shortcut: sound and frames keep separate clocks, started together; tdesktop syncs video to the audio clock. Upgrade if long videos drift.
    pub(super) fn new(
        path: &Path,
        start: Duration,
        size: (u32, u32),
        duration: Duration,
        ending: Ending,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut has_failed = false;
        let decoder = match VideoDecoder::open(path, size) {
            Ok(mut decoder) => {
                if start > Duration::ZERO {
                    decoder.seek(start);
                }
                Some(decoder)
            }
            Err(error) => {
                tracing::warn!(%error, "cannot play the video");
                has_failed = true;
                None
            }
        };
        let sound = (ending == Ending::Pause && !has_failed)
            .then(|| sound(path, start))
            .flatten();
        cx.on_release(|this: &mut Self, cx| {
            if let Some(image) = this.shown.take() {
                cx.default_global::<Retired>().0.push(image);
            }
        })
        .detach();
        Self {
            decoder,
            sound,
            ending,
            duration,
            clock: if has_failed {
                Clock::Paused(start)
            } else {
                Clock::Playing {
                    since: Instant::now(),
                    from: start,
                }
            },
            shown: None,
            pending: None,
            needs_frame: !has_failed,
            is_ended: has_failed,
            has_failed,
            is_dragging: false,
        }
    }

    pub(super) fn is_playing(&self) -> bool {
        matches!(self.clock, Clock::Playing { .. })
    }

    /// Stopped at the end, for good.
    pub(super) fn is_finished(&self) -> bool {
        self.is_ended && self.pending.is_none() && !self.is_playing()
    }

    /// The file cannot be decoded.
    pub(super) fn has_failed(&self) -> bool {
        self.has_failed
    }

    /// Whether the next frame may show something new.
    pub(super) fn needs_redraw(&self) -> bool {
        self.is_playing() || self.needs_frame
    }

    /// The frame on screen, once the first one is decoded.
    pub(super) fn frame(&self) -> Option<Arc<RenderImage>> {
        self.shown.clone()
    }

    /// How much of the video is played, 0 to 1.
    pub(super) fn progress(&self) -> f32 {
        if self.duration.is_zero() {
            return 0.0;
        }
        (self.clock.now().as_secs_f32() / self.duration.as_secs_f32()).min(1.0)
    }

    /// What is left to play.
    pub(super) fn remaining(&self) -> Duration {
        self.duration.saturating_sub(self.clock.now())
    }

    pub(super) fn toggle(&mut self, cx: &mut Context<Self>) {
        let now = self.clock.now();
        self.clock = match self.clock {
            Clock::Playing { .. } => Clock::Paused(now),
            Clock::Paused(_) if self.is_ended => {
                self.seek(Duration::ZERO);
                Clock::Playing {
                    since: Instant::now(),
                    from: Duration::ZERO,
                }
            }
            Clock::Paused(at) => Clock::Playing {
                since: Instant::now(),
                from: at,
            },
        };
        self.sync_sound();
        cx.notify();
    }

    /// Pauses or resumes the sound with the clock.
    fn sync_sound(&self) {
        let Some(sound) = &self.sound else {
            return;
        };
        match self.clock {
            Clock::Playing { .. } => sound.resume(),
            Clock::Paused(_) => sound.pause(),
        }
    }

    fn seek(&mut self, to: Duration) {
        if let Some(decoder) = &mut self.decoder {
            decoder.seek(to);
        }
        if let Some(sound) = &self.sound {
            sound.seek(to);
        }
        self.pending = None;
        self.needs_frame = true;
        self.is_ended = false;
        self.clock = match self.clock {
            Clock::Playing { .. } => Clock::Playing {
                since: Instant::now(),
                from: to,
            },
            Clock::Paused(_) => Clock::Paused(to),
        };
    }

    /// Takes what the decoder made, and shows the latest frame due.
    pub(super) fn advance(&mut self, cx: &mut Context<Self>) {
        let Some(decoder) = &self.decoder else {
            return;
        };
        let now = self.clock.now();
        let mut due = None;
        loop {
            if self.pending.is_none() {
                match decoder.poll() {
                    Some(Output::Info(info)) if !info.duration.is_zero() => {
                        self.duration = info.duration;
                        continue;
                    }
                    Some(Output::Info(_)) => continue,
                    Some(Output::Frame(frame)) => self.pending = Some(frame),
                    Some(Output::End) => self.is_ended = true,
                    Some(Output::Failed(error)) => {
                        tracing::warn!(%error, "the video stopped");
                        self.is_ended = true;
                        self.has_failed = true;
                    }
                    None => {}
                }
            }
            let Some(frame) = self
                .pending
                .take_if(|frame| frame.position <= now || (self.needs_frame && due.is_none()))
            else {
                break;
            };
            self.needs_frame = false;
            due = Some(frame);
        }
        if let Some(frame) = due {
            self.show(frame, cx);
        }
        if self.is_ended && self.pending.is_none() && self.is_playing() {
            if self.ending.restarts(self.has_failed) {
                self.seek(Duration::ZERO);
            } else {
                self.clock = Clock::Paused(self.duration.max(now));
                self.sync_sound();
            }
            cx.notify();
        }
    }

    fn show(&mut self, frame: VideoFrame, cx: &mut App) {
        let Some(buffer) = image::RgbaImage::from_raw(frame.width, frame.height, frame.pixels)
        else {
            return;
        };
        let image = Arc::new(RenderImage::new([image::Frame::new(buffer)]));
        if let Some(old) = self.shown.replace(image) {
            cx.default_global::<Retired>().0.push(old);
        }
    }

    fn seek_to_x(&mut self, x: Pixels, bar: Bounds<Pixels>, cx: &mut Context<Self>) {
        let share = ((x - bar.left()) / bar.size.width).clamp(0.0, 1.0);
        self.seek(self.duration.mul_f32(share));
        cx.notify();
    }

    fn controls(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let now = self.clock.now().min(self.duration);
        let left = now.as_secs();
        let rest = self.duration.saturating_sub(now).as_secs();
        let label = &MEDIAVIEW_PLAY_PROGRESS_LABEL;
        let text = |text: String| {
            div()
                .absolute()
                .top(px_of(MEDIAVIEW_PLAY_PROGRESS_TOP))
                .font(font(label.font))
                .text_size(px_of(label.font.size))
                .line_height(font_height(label.font))
                .text_color(color(cx, label.text_fg))
                .child(text)
        };
        let side = px_of(MEDIAVIEW_PLAY_PROGRESS_LEFT) + px_of(MEDIAVIEW_PLAY_PROGRESS_SKIP);
        let bar_side = side + px(LABEL_WIDTH) + px_of(MEDIAVIEW_PLAY_PROGRESS_SKIP);
        let button_style = if self.is_playing() {
            &MEDIAVIEW_PAUSE_BUTTON
        } else {
            &MEDIAVIEW_PLAY_BUTTON
        };
        let weak = cx.weak_entity();
        let share = self.progress();
        div()
            .relative()
            .w(px_of(MEDIAVIEW_CONTROLLER_SIZE.width))
            .max_w_full()
            .h(px_of(MEDIAVIEW_CONTROLLER_SIZE.height))
            .rounded(px_of(MEDIAVIEW_CONTROLLER_RADIUS))
            .bg(color(cx, ColorRef::Palette(ColorId::MediaviewSaveMsgBg)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .absolute()
                    .top(px_of(MEDIAVIEW_PLAY_BUTTON_TOP))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(IconButton::new("media-view-play", button_style).on_click(
                        move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| this.toggle(cx));
                        },
                    )),
            )
            .child(text(duration_text(seconds(left))).left(side))
            .child(text(format!("-{}", duration_text(seconds(rest)))).right(side))
            .child(self.bar(share, bar_side, cx))
            .into_any_element()
    }

    /// The seek bar (`mediaviewPlayback`): the played part, the rest, and
    /// a click or a drag that seeks.
    fn bar(&self, share: f32, side: Pixels, cx: &mut Context<Self>) -> impl IntoElement {
        let slider = &MEDIAVIEW_PLAYBACK;
        let (active, inactive) = (color(cx, slider.active_fg), color(cx, slider.inactive_fg));
        let line = px_of(slider.width);
        let height = px_of(slider.seek_size.height);
        let entity = cx.entity();
        let bounds = std::rc::Rc::new(std::cell::Cell::new(Bounds::default()));
        let (down, moved) = (bounds.clone(), bounds.clone());
        div()
            .id("media-view-seek")
            .absolute()
            .top(px_of(MEDIAVIEW_PLAYBACK_TOP))
            .left(side)
            .right(side)
            .h(height)
            .cursor_pointer()
            .child(
                canvas(
                    move |bar, _, _| bounds.set(bar),
                    move |bar, (), window, _| {
                        let top = bar.top() + (bar.size.height - line) / 2.0;
                        let played = bar.size.width * share;
                        let fill = |left, width, fg| {
                            gpui::fill(
                                Bounds::new(gpui::point(left, top), gpui::size(width, line)),
                                fg,
                            )
                            .corner_radii(line / 2.0)
                        };
                        window.paint_quad(fill(bar.left(), bar.size.width, inactive));
                        window.paint_quad(fill(bar.left(), played, active));
                    },
                )
                .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.is_dragging = true;
                    this.seek_to_x(event.position.x, down.get(), cx);
                }),
            )
            .on_mouse_move(move |event: &MouseMoveEvent, _, cx| {
                let bar = moved.get();
                entity.update(cx, |this, cx| {
                    if this.is_dragging && event.dragging() {
                        this.seek_to_x(event.position.x, bar, cx);
                    }
                });
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.is_dragging = false),
            )
    }
}

/// `path`'s sound from `start`; none for a silent video.
fn sound(path: &Path, start: Duration) -> Option<AudioPlayer> {
    match AudioPlayer::open(path, start) {
        Ok(sound) => Some(sound),
        Err(AudioError::NoAudio) => None,
        Err(error) => {
            tracing::warn!(%error, "the video plays without sound");
            None
        }
    }
}

/// Frees the frames taken off screen from `window`'s atlas.
pub(super) fn drop_retired(window: &mut Window, cx: &mut App) {
    for image in std::mem::take(&mut cx.default_global::<Retired>().0) {
        if let Err(error) = window.drop_image(image) {
            tracing::warn!(%error, "cannot drop a video frame");
        }
    }
}

fn seconds(whole: u64) -> i32 {
    i32::try_from(whole).unwrap_or(i32::MAX)
}

impl Render for VideoPlayback {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.advance(cx);
        drop_retired(window, cx);
        if self.needs_redraw() {
            window.request_animation_frame();
        }
        let frame = self.shown.clone();
        div().size_full().flex().flex_col().child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    if let Some(image) = frame.clone()
                        && let Err(error) =
                            window.paint_image(bounds, bounds, Corners::default(), image, 0, false)
                    {
                        tracing::warn!(%error, "cannot paint a video frame");
                    }
                },
            )
            .flex_1(),
        )
    }
}

impl VideoPlayback {
    /// The controller under the video, centered over the bottom edge.
    pub(super) fn controller(this: &gpui::Entity<Self>, cx: &mut App) -> gpui::AnyElement {
        // The viewer draws the controller before the video: catch up first.
        this.update(cx, |this, cx| {
            this.advance(cx);
            this.controls(cx)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_decoding_loop_starts_over() {
        assert!(Ending::Loop.restarts(false));
        assert!(!Ending::Loop.restarts(true));
        assert!(!Ending::Pause.restarts(false));
    }

    #[test]
    fn a_paused_clock_stands_still() {
        let at = Duration::from_millis(700);
        assert_eq!(Clock::Paused(at).now(), at);
        let playing = Clock::Playing {
            since: Instant::now(),
            from: at,
        };
        assert!(playing.now() >= at);
    }
}
