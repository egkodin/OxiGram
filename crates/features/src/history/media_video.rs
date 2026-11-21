//! A video playing (`Media::Streaming::Instance`): frames from
//! [`media::VideoDecoder`] shown on the wall clock. The viewer renders it
//! with the controller (`Media::View::PlaybackControls`): the play button,
//! the times and the seek bar; a history paints its frames in the bubble.

use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, Corners, Global, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement as _, Pixels, Render, RenderImage, Styled as _,
    Window, canvas, div, px,
};
use media::{AudioError, AudioPlayer, Output, VideoDecoder, VideoFrame};
use style::basic::{RADIAL_LINE, RADIAL_SIZE};
use style::media_view::{
    MEDIAVIEW_BUTTONS_RIGHT, MEDIAVIEW_BUTTONS_TOP, MEDIAVIEW_CONTROLLER_RADIUS,
    MEDIAVIEW_CONTROLLER_SIZE, MEDIAVIEW_PAUSE_ICON, MEDIAVIEW_PAUSE_ICON_OVER,
    MEDIAVIEW_PLAY_BUTTON, MEDIAVIEW_PLAY_BUTTON_TOP, MEDIAVIEW_PLAY_PROGRESS_LABEL,
    MEDIAVIEW_PLAY_PROGRESS_LEFT, MEDIAVIEW_PLAY_PROGRESS_SKIP, MEDIAVIEW_PLAY_PROGRESS_TOP,
    MEDIAVIEW_PLAYBACK, MEDIAVIEW_PLAYBACK_TOP, MEDIAVIEW_SPEED_BUTTON,
};
use style::widgets::DEFAULT_INFINITE_RADIAL_ANIMATION;
use style::{ColorId, ColorRef};
use ui::{IconButton, InfiniteRadial, RadialState, color, font, font_height, paint_arc};

use super::video::duration_text;
use crate::player::{Command, RoundLink};
use crate::px_of;
use crate::speed::{Kind, Speed};
use crate::streaming::Playable;

static MEDIAVIEW_PAUSE_BUTTON: style::widgets::IconButton = style::widgets::IconButton {
    icon: MEDIAVIEW_PAUSE_ICON,
    icon_over: MEDIAVIEW_PAUSE_ICON_OVER,
    ..MEDIAVIEW_PLAY_BUTTON
};

/// Room the time labels take beside the seek bar ("-10:00" in 12px bold).
const LABEL_WIDTH: f32 = 40.0;
/// `kWaitingShowDelay`, `kWaitingShowDuration`: the buffering spinner shows
/// only when the data is late for a while, then fades in.
const WAITING_SHOW_DELAY: Duration = Duration::from_millis(500);
const WAITING_SHOW_DURATION: Duration = Duration::from_millis(500);

/// The video waiting for data: since when, and where the clock holds.
#[derive(Debug, Clone, Copy)]
struct Stall {
    since: Instant,
    at: Duration,
}

/// The playback clock: where the video is, moving or not.
#[derive(Debug, Clone, Copy)]
enum Clock {
    /// Moving `speed` times as fast as the wall clock.
    Playing {
        since: Instant,
        from: Duration,
        speed: f32,
    },
    Paused(Duration),
}

impl Clock {
    fn start(from: Duration, speed: f32) -> Self {
        Self::Playing {
            since: Instant::now(),
            from,
            speed,
        }
    }

    fn now(self) -> Duration {
        match self {
            Self::Playing { since, from, speed } => from + since.elapsed().mul_f32(speed),
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
    /// The downloaded share past the playhead (`receivedTill`), 0 to 1.
    received: f32,
    stall: Option<Stall>,
    waiting: InfiniteRadial,
    /// When it opened, until the first frame shows (`docs/perf.md`).
    opened: Option<Instant>,
    speed: f32,
    /// A round's place in the app's player, which drives it.
    link: Option<Rc<RoundLink>>,
    /// The viewer's speed presets, open at a point.
    speed_menu: Option<(
        gpui::Point<Pixels>,
        gpui::Entity<ui::PopupMenu>,
        gpui::Subscription,
    )>,
}

impl VideoPlayback {
    /// Starts `file` at `start`, playing, in frames no bigger than `size`;
    /// with its sound unless it loops, as muted GIFs and stickers do. A file
    /// still downloading streams: its clock holds while the data is late.
    // shortcut: sound and frames keep separate clocks, started together; tdesktop syncs video to the audio clock. Upgrade if long videos drift.
    pub(super) fn new(
        file: &Playable,
        start: Duration,
        size: (u32, u32),
        duration: Duration,
        ending: Ending,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut has_failed = false;
        let decoder = match file.video(size) {
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
            .then(|| sound(file, start))
            .flatten();
        cx.on_release(|this: &mut Self, cx| {
            if let Some(link) = this.link.take() {
                link.end();
            }
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
                Clock::start(start, 1.0)
            },
            shown: None,
            pending: None,
            needs_frame: !has_failed,
            is_ended: has_failed,
            has_failed,
            is_dragging: false,
            received: 0.0,
            stall: None,
            waiting: InfiniteRadial::new(&DEFAULT_INFINITE_RADIAL_ANIMATION),
            opened: Some(Instant::now()),
            speed: 1.0,
            link: None,
            speed_menu: None,
        }
    }

    /// Hands a round to the app's player, which then pauses, seeks and
    /// times it.
    pub(super) fn link(&mut self, link: Rc<RoundLink>) {
        self.link = Some(link);
    }

    /// The player let go of this round: another sound started or the bar
    /// closed.
    pub(super) fn is_released(&self) -> bool {
        self.link
            .as_ref()
            .is_some_and(|link| Rc::strong_count(link) == 1)
    }

    /// Runs what the player asked, then tells it where the round is.
    fn sync_link(&mut self, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else {
            return;
        };
        for command in link.take_commands() {
            match command {
                Command::Pause if self.is_playing() => self.toggle(cx),
                Command::Resume if !self.is_playing() => self.toggle(cx),
                Command::Pause | Command::Resume => {}
                Command::Seek(share) => self.seek_share(share, cx),
                Command::Speed(speed) => self.set_speed(speed),
            }
        }
        link.report(
            self.clock.now().min(self.duration),
            self.duration,
            (
                !self.is_playing(),
                if self.has_failed {
                    crate::player::RoundEnd::Stopped
                } else if self.is_finished() {
                    crate::player::RoundEnd::Completed
                } else {
                    crate::player::RoundEnd::Active
                },
            ),
        );
    }

    /// Plays `speed` times as fast, sound and frames, from now on.
    pub(super) fn set_speed(&mut self, speed: f32) {
        let speed = speed.clamp(media::MIN_SPEED, media::MAX_SPEED);
        self.speed = speed;
        if let Some(sound) = &self.sound {
            sound.set_speed(speed);
        }
        if let Clock::Playing { .. } = self.clock {
            self.clock = Clock::start(self.clock.now(), speed);
        }
    }

    /// The viewer's speed button (`mediaviewSpeedButton`), top right.
    fn speed_button(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let (toggle, menu) = (cx.weak_entity(), cx.weak_entity());
        let button = crate::speed::button(
            "media-view-speed",
            &MEDIAVIEW_SPEED_BUTTON,
            self.speed,
            move |_, cx| {
                let _ = toggle.update(cx, |this, cx| {
                    let speed = crate::speed::change(Kind::Video, Speed::toggled, cx);
                    this.set_speed(speed);
                    cx.notify();
                });
            },
            move |window, cx| {
                let _ = menu.update(cx, |this, cx| this.show_speed_menu(window, cx));
            },
            cx,
        );
        div()
            .absolute()
            .top(px_of(MEDIAVIEW_BUTTONS_TOP))
            .right(px_of(MEDIAVIEW_BUTTONS_RIGHT))
            .child(button)
            .children(
                self.speed_menu
                    .as_ref()
                    .map(|(position, menu, _)| crate::speed::overlay(*position, menu.clone())),
            )
            .into_any_element()
    }

    fn show_speed_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        let on_choose: crate::speed::OnChoose = Rc::new(move |speed, _, cx| {
            let _ = weak.update(cx, |this, cx| {
                let speed = crate::speed::change(Kind::Video, |kept| kept.chosen(speed), cx);
                this.set_speed(speed);
                cx.notify();
            });
        });
        let current = self.speed;
        let menu = gpui::AppContext::new(cx, |cx| crate::speed::menu(current, &on_choose, cx));
        let subscription = cx.subscribe(&menu, |this, _, _: &gpui::DismissEvent, cx| {
            this.speed_menu = None;
            cx.notify();
        });
        gpui::Focusable::focus_handle(menu.read(cx), cx).focus(window, cx);
        self.speed_menu = Some((window.mouse_position(), menu, subscription));
        cx.notify();
    }

    /// Whether it plays on off screen: a round with its sound, not a
    /// muted loop.
    pub(super) fn plays_away(&self) -> bool {
        self.ending == Ending::Pause
    }

    /// Jumps to `share` of the way in, 0 to 1.
    pub(super) fn seek_share(&mut self, share: f32, cx: &mut Context<Self>) {
        self.seek(self.duration.mul_f32(share.clamp(0.0, 1.0)));
        cx.notify();
    }

    /// Holds the clock and the sound while the next frame waits for data,
    /// as tdesktop's player does in its waiting state.
    fn hold_while_waiting(&mut self) {
        let is_waiting = self.is_playing()
            && self.pending.is_none()
            && !self.is_ended
            && (self.needs_frame
                || self
                    .decoder
                    .as_ref()
                    .is_some_and(VideoDecoder::is_buffering));
        if !is_waiting {
            if self.stall.take().is_some() {
                self.waiting.stop();
                self.sync_sound();
            }
            return;
        }
        let now = self.clock.now();
        let stall = *self.stall.get_or_insert_with(|| Stall {
            since: Instant::now(),
            at: now,
        });
        if stall.since.elapsed() >= WAITING_SHOW_DELAY {
            self.waiting.start();
        }
        if let Some(sound) = &self.sound {
            sound.pause();
        }
        self.clock = Clock::start(stall.at, self.speed);
    }

    /// The buffering spinner now: its fade and arc; `None` while hidden.
    /// It moves while the clock holds, which asks for frames.
    pub(super) fn spinner(&self) -> Option<(f32, RadialState)> {
        let stall = self.stall?;
        let shown = stall.since.elapsed().checked_sub(WAITING_SHOW_DELAY)?;
        let opacity = (shown.as_secs_f32() / WAITING_SHOW_DURATION.as_secs_f32()).min(1.0);
        Some((opacity, self.waiting.state(Instant::now())?))
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
                Clock::start(Duration::ZERO, self.speed)
            }
            Clock::Paused(at) => Clock::start(at, self.speed),
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
            Clock::Playing { .. } => Clock::start(to, self.speed),
            Clock::Paused(_) => Clock::Paused(to),
        };
    }

    /// Takes what the decoder made, and shows the latest frame due.
    pub(super) fn advance(&mut self, cx: &mut Context<Self>) {
        self.advance_frames(cx);
        self.sync_link(cx);
    }

    fn advance_frames(&mut self, cx: &mut Context<Self>) {
        let Some(decoder) = &self.decoder else {
            return;
        };
        self.received = decoder.received();
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
        self.hold_while_waiting();
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
        if let Some(opened) = self.opened.take() {
            tracing::info!(elapsed = ?opened.elapsed(), "first video frame");
        }
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
        let speed = self.speed_button(cx);
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
        let share = (self.progress(), self.received);
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
            .child(speed)
            .child(text(duration_text(seconds(left))).left(side))
            .child(text(format!("-{}", duration_text(seconds(rest)))).right(side))
            .child(self.bar(share, bar_side, cx))
            .into_any_element()
    }

    /// The seek bar (`mediaviewPlayback`): the played part, the part on
    /// disk past it, the rest, and a click or a drag that seeks.
    fn bar(&self, share: (f32, f32), side: Pixels, cx: &mut Context<Self>) -> impl IntoElement {
        let (share, received) = share;
        let slider = &MEDIAVIEW_PLAYBACK;
        let (active, inactive) = (color(cx, slider.active_fg), color(cx, slider.inactive_fg));
        let received_fg = color(cx, slider.received_till_fg);
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
                        let loaded = bar.size.width * received.max(share);
                        let fill = |left, width, fg| {
                            gpui::fill(
                                Bounds::new(gpui::point(left, top), gpui::size(width, line)),
                                fg,
                            )
                            .corner_radii(line / 2.0)
                        };
                        window.paint_quad(fill(bar.left(), bar.size.width, inactive));
                        window.paint_quad(fill(bar.left(), loaded, received_fg));
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

/// `file`'s sound from `start`; none for a silent video.
fn sound(file: &Playable, start: Duration) -> Option<AudioPlayer> {
    match file.audio(start, 100) {
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

/// tdesktop's streaming wait (`paintRadialLoadingContent`): a `radialBg`
/// circle in the middle with the infinite arc inside it.
fn paint_spinner(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    opacity: f32,
    state: RadialState,
    (bg, fg): (gpui::Hsla, gpui::Hsla),
) {
    let size = gpui::size(px_of(RADIAL_SIZE.width), px_of(RADIAL_SIZE.height));
    let inner = Bounds::centered_at(bounds.center(), size);
    let line = px_of(RADIAL_LINE);
    let bg = gpui::Hsla {
        a: bg.a * opacity,
        ..bg
    };
    window.paint_quad(gpui::fill(inner, bg).corner_radii(size.width / 2.0));
    paint_arc(window, inner.dilate(-line), state, line, fg);
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
        let spinner = self.spinner();
        let (bg, fg) = (
            color(cx, ColorRef::Palette(ColorId::RadialBg)),
            color(cx, ColorRef::Palette(ColorId::RadialFg)),
        );
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
                    if let Some((opacity, state)) = spinner {
                        paint_spinner(window, bounds, opacity, state, (bg, fg));
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
        let playing = Clock::start(at, 1.0);
        assert!(playing.now() >= at);
    }
}
