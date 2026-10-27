//! Voice messages from the microphone button (`VoiceRecordBar`): holding
//! it records, with the bar in place of the field (the blinking dot, the
//! time, "Release outside this field to cancel") and the round button
//! pulsing with the level over the send button. Releasing in the field
//! sends, outside cancels; dragging up to the lock keeps recording, and
//! then the button sends and Cancel throws it away.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, BoxShadow, Context, DispatchPhase, InteractiveElement as _, IntoElement as _,
    MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, SharedString,
    StatefulInteractiveElement as _, Styled as _, Task, TextRun, Window, canvas, div, hsla, point,
    px,
};
use i18n::key;
use model::send::SendOptions;
use model::send_files::VoiceNote;
use style::basic::{SEMIBOLD_FONT, UNIVERSAL_DURATION};
use style::chat_helpers::{
    DEFAULT_RECORD_BAR, HISTORY_RECORD_MAIN_BLOB_MAX_RADIUS, HISTORY_RECORD_MAIN_BLOB_MIN_RADIUS,
    HISTORY_RECORD_MAJOR_BLOB_MAX_RADIUS, HISTORY_RECORD_MAJOR_BLOB_MIN_RADIUS,
    HISTORY_RECORD_MINOR_BLOB_MAX_RADIUS, HISTORY_RECORD_MINOR_BLOB_MIN_RADIUS,
    HISTORY_RECORD_SEND_ICON_POSITION, HISTORY_RECORD_VOICE_FG_ACTIVE,
    HISTORY_RECORD_VOICE_FG_ACTIVE_ICON, HISTORY_RECORD_VOICE_FG_INACTIVE, HISTORY_SEND_ICON,
    HISTORY_SEND_RIGHT, HISTORY_SEND_SIZE,
};
use style::history_view_voice_record_bar::{
    HISTORY_RECORD_DURATION_SKIP, HISTORY_RECORD_FONT, HISTORY_RECORD_LOCK_MARGIN,
    HISTORY_RECORD_LOCK_POSITION, HISTORY_RECORD_LOCK_SIZE, HISTORY_RECORD_SIGNAL_RADIUS,
    HISTORY_RECORD_TEXT_LEFT, HISTORY_RECORD_TEXT_RIGHT,
};
use style::{ColorId, ColorRef, Font};
use ui::{color, font, font_height, icon_size};

use super::HistoryView;
use crate::lang::tr;
use crate::px_of;
use crate::store::spawn;

/// The level that fills the button, on a 16-bit peak (`kMaxLevel`).
const MAX_LEVEL: f32 = 1800.0 / 32767.0;
/// How fast the shown level follows the microphone (`kLevelDuration`).
const LEVEL_DURATION: f32 = 0.265;
const BLOB_ALPHA: f32 = 76.0 / 255.0;
/// The dot's blink (`startRedCircleAnimation`).
const DOT_PERIOD: f32 = 0.4;
/// Recording stops at 100 minutes (`kAudioVoiceMaxLength`).
const MAX_DURATION: Duration = Duration::from_secs(100 * 60);
const TOAST_DURATION: Duration = Duration::from_millis(1500);
/// The widest time the bar makes room for (`FormatVoiceDuration(kMaxSamples)`).
const WIDEST_DURATION: &str = "1:40:00.0";

#[derive(Default)]
pub(super) enum Voice {
    #[default]
    Idle,
    /// The button is down; recording starts unless it comes up first.
    Pressing {
        _start: Task<()>,
        /// Where the pointer went meanwhile, for the recording to start
        /// from.
        moved: Option<Pointer>,
    },
    Recording(Box<Recording>),
}

/// The pointer, the field row's top and the view's bottom right corner.
#[derive(Debug, Clone, Copy)]
pub(super) struct Pointer {
    position: Point<Pixels>,
    bar_top: Pixels,
    corner: Point<Pixels>,
}

/// What recordings record from; the microphone unless preset.
#[derive(Default)]
struct VoiceSource(media::Source);

impl gpui::Global for VoiceSource {}

/// Records `samples` at `rate` instead of the microphone, as the snapshots
/// do.
pub fn preset_voice_samples(rate: u32, samples: std::sync::Arc<[f32]>, cx: &mut App) {
    cx.set_global(VoiceSource(media::Source::Samples { rate, samples }));
}

pub(super) struct Recording {
    recorder: media::Recorder,
    duration: Duration,
    /// The shown level, 0 to 1, following the microphone's.
    level: f32,
    started: Instant,
    frame: Instant,
    is_in_field: bool,
    /// How far up towards the lock, 0 to 1; locked at 1.
    lock: f32,
}

impl Recording {
    fn is_locked(&self) -> bool {
        self.lock >= 1.0
    }

    /// Reads the microphone for a frame.
    fn tick(&mut self) {
        let progress = self.recorder.progress();
        self.duration = progress.duration;
        let now = Instant::now();
        let step = (now - self.frame).as_secs_f32() / LEVEL_DURATION;
        self.frame = now;
        let target = (progress.level / MAX_LEVEL).min(1.0);
        self.level += (target - self.level) * step.min(1.0);
    }

    fn main_radius(&self) -> Pixels {
        radius(
            HISTORY_RECORD_MAIN_BLOB_MIN_RADIUS,
            HISTORY_RECORD_MAIN_BLOB_MAX_RADIUS,
            self.level,
        )
    }
}

fn radius(min: style::Px, max: style::Px, level: f32) -> Pixels {
    px_of(min) + (px_of(max) - px_of(min)) * level
}

/// "0:03.4" (`FormatVoiceDuration`).
fn duration_text(duration: Duration) -> String {
    let tenths = duration.as_millis() / 100;
    let seconds = tenths / 10;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    let whole = if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    };
    format!("{whole}.{}", tenths % 10)
}

/// How far `y` is above the bar's top towards the lock (`calcLockProgress`).
fn lock_progress(y: Pixels, bar_top: Pixels) -> f32 {
    ((bar_top - y) / px_of(HISTORY_RECORD_LOCK_SIZE.height)).clamp(0.0, 1.0)
}

/// The send button's center from the view's bottom right corner.
fn button_offset() -> Point<Pixels> {
    point(
        px_of(HISTORY_SEND_RIGHT) + px_of(HISTORY_SEND_SIZE.width) / 2.0,
        px_of(HISTORY_SEND_SIZE.height) / 2.0,
    )
}

fn voice_path() -> std::io::Result<PathBuf> {
    // shortcut: recordings stay in the temp folder for the system to clear; tdesktop uploads them from memory.
    let folder = std::env::temp_dir().join("rgram-voice");
    std::fs::create_dir_all(&folder)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    Ok(folder.join(format!("voice-{stamp}.ogg")))
}

fn text_width(text: &str, text_font: Font, window: &Window) -> Pixels {
    let run = TextRun {
        len: text.len(),
        font: font(text_font),
        color: hsla(0.0, 0.0, 0.0, 1.0),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(
            SharedString::from(text.to_owned()),
            px_of(text_font.size),
            &[run],
            None,
        )
        .width
}

impl HistoryView {
    /// The microphone where the send button is; holding it records.
    pub(super) fn record_button(
        &self,
        button: ui::IconButton,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.press_record(cx)),
            )
            .child(button)
            .into_any_element()
    }

    fn press_record(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.voice, Voice::Idle) {
            return;
        }
        let delay = Duration::from_millis(UNIVERSAL_DURATION.unsigned_abs().into());
        let start = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            // The view is gone once its chat closed.
            let _ = this.update(cx, |this, cx| this.start_recording(cx));
        });
        self.voice = Voice::Pressing {
            _start: start,
            moved: None,
        };
        cx.notify();
    }

    fn start_recording(&mut self, cx: &mut Context<Self>) {
        let Voice::Pressing { moved, .. } = &self.voice else {
            return;
        };
        let moved = *moved;
        let source = cx.default_global::<VoiceSource>().0.clone();
        let started = voice_path()
            .and_then(|path| media::Recorder::start(&source, path).map_err(std::io::Error::other));
        match started {
            Ok(recorder) => {
                let now = Instant::now();
                self.voice = Voice::Recording(Box::new(Recording {
                    recorder,
                    duration: Duration::ZERO,
                    level: 0.0,
                    started: now,
                    frame: now,
                    is_in_field: true,
                    lock: 0.0,
                }));
                if let Some(pointer) = moved {
                    self.voice_moved(pointer, cx);
                }
            }
            Err(error) => {
                tracing::warn!(%error, "voice recording did not start");
                self.voice = Voice::Idle;
                self.voice_toast(key::lng_record_audio_problem, cx);
            }
        }
        cx.notify();
    }

    fn voice_toast(&self, text: i18n::Key, cx: &mut Context<Self>) {
        let text = tr(cx, text);
        self.layers.update(cx, |layers, cx| {
            layers.show_toast(text, &style::widgets::DEFAULT_TOAST, TOAST_DURATION, cx);
        });
    }

    /// The pointer moved while the button is held.
    fn voice_moved(&mut self, pointer: Pointer, cx: &mut Context<Self>) {
        let recording = match &mut self.voice {
            Voice::Idle => return,
            Voice::Pressing { moved, .. } => {
                *moved = Some(pointer);
                return;
            }
            Voice::Recording(recording) => recording,
        };
        let Pointer {
            position,
            bar_top,
            corner,
        } = pointer;
        if recording.is_locked() {
            return;
        }
        let center = corner - button_offset();
        let to_center = position - center;
        let (dx, dy) = (to_center.x / px(1.0), to_center.y / px(1.0));
        let reach = recording.main_radius() / px(1.0);
        let in_circle = dx * dx + dy * dy <= reach * reach;
        recording.is_in_field = position.y >= bar_top && position.y <= corner.y || in_circle;
        recording.lock = lock_progress(position.y, bar_top);
        cx.notify();
    }

    fn voice_released(&mut self, cx: &mut Context<Self>) {
        match &self.voice {
            Voice::Idle => {}
            Voice::Pressing { .. } => {
                // A click: tdesktop shows how to record instead.
                self.voice = Voice::Idle;
                self.voice_toast(key::lng_record_hold_tip, cx);
                cx.notify();
            }
            Voice::Recording(recording) if recording.is_locked() => {}
            Voice::Recording(recording) => {
                let send = recording.is_in_field;
                self.finish_voice(send, cx);
            }
        }
    }

    /// Ends the recording, sending it when `send` is set.
    fn finish_voice(&mut self, send: bool, cx: &mut Context<Self>) {
        let Voice::Recording(recording) = std::mem::take(&mut self.voice) else {
            return;
        };
        cx.notify();
        if !send {
            recording.recorder.cancel();
            return;
        }
        let stopping = recording.recorder.stop();
        let reply = self.take_reply();
        let (chat, topic) = (self.chat, self.topic);
        let written = cx
            .background_executor()
            .spawn(async move { stopping.wait() });
        cx.spawn(async move |this, cx| {
            let recorded = match written.await {
                Ok(Some(recorded)) => recorded,
                // Under a second: nothing to send.
                Ok(None) => return,
                Err(error) => {
                    tracing::warn!(%error, "finishing the voice message failed");
                    return;
                }
            };
            let voice = VoiceNote {
                path: recorded.path,
                duration: recorded.duration,
                waveform: recorded.waveform,
            };
            let Ok(task) = this.update(cx, |this, cx| {
                let request = this.store.read(cx).session().send_voice(
                    chat,
                    topic,
                    reply,
                    &voice,
                    SendOptions::default(),
                );
                this.scroll_down(cx);
                spawn(request, cx)
            }) else {
                return;
            };
            if let Err(error) = task.await {
                tracing::warn!(%error, "sending the voice message failed");
            }
        })
        .detach();
    }

    /// The bar in place of the field while recording.
    pub(super) fn voice_bar(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Voice::Recording(recording) = &self.voice else {
            return None;
        };
        let st = &DEFAULT_RECORD_BAR;
        let height = px_of(HISTORY_SEND_SIZE.height);
        let dot = px_of(HISTORY_RECORD_SIGNAL_RADIUS) * 2.0;
        let dot_left = height / 2.0 - px_of(HISTORY_RECORD_SIGNAL_RADIUS);
        let blink = (recording.started.elapsed().as_secs_f32() / DOT_PERIOD)
            .sin()
            .abs();
        let duration_left = dot_left + dot + px_of(HISTORY_RECORD_DURATION_SKIP);
        let message_left = duration_left
            + text_width(WIDEST_DURATION, HISTORY_RECORD_FONT, window)
            + px_of(HISTORY_RECORD_TEXT_LEFT);
        let message_right = px_of(HISTORY_SEND_SIZE.width) + px_of(HISTORY_RECORD_TEXT_RIGHT);
        let text = |text: SharedString, text_font: Font, fg: ColorRef| {
            div()
                .font(font(text_font))
                .text_size(px_of(text_font.size))
                .line_height(font_height(text_font))
                .text_color(color(cx, fg))
                .whitespace_nowrap()
                .child(text)
        };
        let center = if recording.is_locked() {
            let weak = cx.weak_entity();
            text(
                tr(cx, key::lng_selected_clear),
                SEMIBOLD_FONT,
                st.cancel_active,
            )
            .id("voice-cancel")
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.finish_voice(false, cx));
            })
            .into_any_element()
        } else {
            let fg = if recording.is_in_field {
                st.cancel
            } else {
                st.cancel_active
            };
            text(tr(cx, key::lng_record_cancel), HISTORY_RECORD_FONT, fg).into_any_element()
        };
        Some(
            div()
                .relative()
                .h(height)
                .bg(color(cx, st.bg))
                .child(
                    div()
                        .absolute()
                        .left(dot_left + px(1.0))
                        .top(dot_left + px(1.0))
                        .size(dot)
                        .rounded_full()
                        .bg(color(cx, HISTORY_RECORD_VOICE_FG_INACTIVE))
                        .opacity(1.0 - blink),
                )
                .child(
                    div()
                        .absolute()
                        .left(duration_left)
                        .top(px(0.0))
                        .h(height)
                        .flex()
                        .items_center()
                        .child(text(
                            duration_text(recording.duration).into(),
                            HISTORY_RECORD_FONT,
                            st.duration_fg,
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .left(message_left)
                        .right(message_right)
                        .top(px(0.0))
                        .h(height)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(center),
                )
                .into_any_element(),
        )
    }

    /// Over the view while the button is held: the round button and the
    /// lock above it, and what follows the pointer anywhere in the window.
    /// Reads the microphone for this frame, before anything shows it.
    pub(super) fn tick_voice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Voice::Recording(recording) = &mut self.voice else {
            return;
        };
        recording.tick();
        window.request_animation_frame();
        if recording.duration >= MAX_DURATION {
            let send = recording.is_in_field;
            let weak = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = weak.update(cx, |this, cx| this.finish_voice(send, cx));
            });
        }
    }

    pub(super) fn voice_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let recording = match &self.voice {
            Voice::Idle => return None,
            Voice::Pressing { .. } => None,
            Voice::Recording(recording) => Some(&**recording),
        };
        let weak = cx.weak_entity();
        let tracker = canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                let corner = bounds.bottom_right();
                let bar_top = corner.y - px_of(HISTORY_SEND_SIZE.height);
                let moved = weak.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture {
                        let pointer = Pointer {
                            position: event.position,
                            bar_top,
                            corner,
                        };
                        let _ = moved.update(cx, |this, cx| this.voice_moved(pointer, cx));
                    }
                });
                let released = weak.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        let _ = released.update(cx, |this, cx| this.voice_released(cx));
                    }
                });
            },
        )
        .absolute()
        .size_full();
        Some(
            div()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .size_full()
                .child(tracker)
                .children(
                    recording
                        .filter(|recording| !recording.is_locked())
                        .map(|recording| lock(recording, cx)),
                )
                .children(recording.map(|recording| level_button(recording, cx)))
                .into_any_element(),
        )
    }
}

/// The round button over the send button (`VoiceRecordButton`).
fn level_button(recording: &Recording, cx: &mut Context<HistoryView>) -> AnyElement {
    let fg = if recording.is_in_field || recording.is_locked() {
        HISTORY_RECORD_VOICE_FG_ACTIVE
    } else {
        HISTORY_RECORD_VOICE_FG_INACTIVE
    };
    let fill = color(cx, fg);
    let size = px_of(HISTORY_RECORD_MAJOR_BLOB_MAX_RADIUS) * 2.0;
    let offset = button_offset();
    // shortcut: the blobs are still circles growing with the level; tdesktop wobbles them (`Ui::Paint::Blobs`).
    let circle = |radius: Pixels, alpha: f32| {
        div()
            .absolute()
            .left(size / 2.0 - radius)
            .top(size / 2.0 - radius)
            .size(radius * 2.0)
            .rounded_full()
            .bg(fill.opacity(alpha))
    };
    let level = recording.level;
    let icon = if recording.is_locked() {
        let position = HISTORY_RECORD_SEND_ICON_POSITION;
        div()
            .ml(px_of(position.x) * 2.0)
            .mt(px_of(position.y) * 2.0)
            .child(ui::icon(HISTORY_SEND_ICON).color(HISTORY_RECORD_VOICE_FG_ACTIVE_ICON))
    } else {
        div()
            .child(ui::icon(super::compose::RECORD_ICON).color(HISTORY_RECORD_VOICE_FG_ACTIVE_ICON))
    };
    let weak = cx.weak_entity();
    div()
        .id("voice-level")
        .absolute()
        .right(offset.x - size / 2.0)
        .bottom(offset.y - size / 2.0)
        .size(size)
        .child(circle(
            radius(
                HISTORY_RECORD_MINOR_BLOB_MIN_RADIUS,
                HISTORY_RECORD_MINOR_BLOB_MAX_RADIUS,
                level,
            ),
            BLOB_ALPHA,
        ))
        .child(circle(
            radius(
                HISTORY_RECORD_MAJOR_BLOB_MIN_RADIUS,
                HISTORY_RECORD_MAJOR_BLOB_MAX_RADIUS,
                level,
            ),
            BLOB_ALPHA,
        ))
        .child(circle(recording.main_radius(), 1.0))
        .child(
            div()
                .absolute()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(icon),
        )
        .when(recording.is_locked(), |button| {
            button.cursor_pointer().on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.finish_voice(true, cx));
            })
        })
        .into_any_element()
}

/// The lock above the button (`RecordLock`, `DrawLockCircle`): a capsule
/// that shortens as the pointer climbs, with the padlock and the arrow.
// shortcut: the padlock is drawn closed and does not tilt, and locking does not turn it into a stop button for the preview.
fn lock(recording: &Recording, cx: &App) -> AnyElement {
    let st = &DEFAULT_RECORD_BAR.lock;
    let width = icon_size(st.shadow_top).width;
    let height = px_of(HISTORY_RECORD_LOCK_SIZE.height);
    let progress = recording.lock;
    let edge = icon_size(st.shadow_top).height + icon_size(st.shadow_bottom).height;
    let top_margin = height / 4.0 * (1.0 - progress);
    let bottom_margin = (height - edge) * progress;
    let margin = px_of(HISTORY_RECORD_LOCK_MARGIN.left);
    let bottom = px_of(HISTORY_SEND_SIZE.height) + px_of(HISTORY_RECORD_LOCK_POSITION.y);
    let fg = color(cx, st.fg);
    let capsule_width = width - margin * 2.0;
    div()
        .absolute()
        .right(px_of(HISTORY_RECORD_LOCK_POSITION.x))
        .bottom(bottom)
        .w(width)
        .h(height)
        .child(
            div()
                .absolute()
                .left(margin)
                .right(margin)
                .top(top_margin + margin)
                .bottom(bottom_margin + margin)
                .rounded(capsule_width / 2.0)
                .bg(color(cx, ColorRef::Palette(ColorId::HistoryToDownBg)))
                .shadow(vec![BoxShadow {
                    color: color(cx, ColorRef::Palette(ColorId::HistoryToDownShadow)),
                    offset: point(px(0.0), px(1.0)),
                    blur_radius: margin,
                    spread_radius: px(0.0),
                    inset: false,
                }])
                .flex()
                .flex_col()
                .items_center()
                .pt(px(14.0))
                .gap(px(10.0))
                .child(padlock(fg))
                .child(div().opacity(1.0 - progress).child(ui::icon(st.arrow))),
        )
        .into_any_element()
}

/// A closed padlock in `fg` (`historyRecordLockIconSize`).
fn padlock(fg: gpui::Hsla) -> AnyElement {
    use style::history_view_voice_record_bar::{
        HISTORY_RECORD_LOCK_ICON_ARC_HEIGHT, HISTORY_RECORD_LOCK_ICON_BOTTOM_HEIGHT,
        HISTORY_RECORD_LOCK_ICON_LINE_SKIP, HISTORY_RECORD_LOCK_ICON_LINE_WIDTH,
        HISTORY_RECORD_LOCK_ICON_SIZE,
    };
    let size = HISTORY_RECORD_LOCK_ICON_SIZE;
    let line = px_of(HISTORY_RECORD_LOCK_ICON_LINE_WIDTH);
    let skip = px_of(HISTORY_RECORD_LOCK_ICON_LINE_SKIP);
    let block = px_of(HISTORY_RECORD_LOCK_ICON_BOTTOM_HEIGHT);
    let shackle_height =
        px_of(size.height) - block + px_of(HISTORY_RECORD_LOCK_ICON_ARC_HEIGHT) / 2.0;
    div()
        .relative()
        .w(px_of(size.width))
        .h(px_of(size.height))
        .child(
            div()
                .absolute()
                .left(skip - line / 2.0)
                .right(skip - line / 2.0)
                .top(px(0.0))
                .h(shackle_height)
                .border_color(fg)
                .border_t(line)
                .border_l(line)
                .border_r(line)
                .rounded_t(px_of(HISTORY_RECORD_LOCK_ICON_ARC_HEIGHT) * 2.0),
        )
        .child(
            div()
                .absolute()
                .left(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .h(block)
                .rounded(px(2.0))
                .bg(fg),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_show_tenths() {
        assert_eq!(duration_text(Duration::from_millis(3_470)), "0:03.4");
        assert_eq!(duration_text(Duration::from_secs(61)), "1:01.0");
        assert_eq!(duration_text(Duration::from_secs(3_725)), "1:02:05.0");
    }

    #[test]
    fn the_lock_fills_over_its_height_above_the_bar() {
        let top = px(500.0);
        assert_eq!(lock_progress(px(520.0), top), 0.0);
        assert_eq!(lock_progress(top - px(66.5), top), 0.5);
        assert_eq!(lock_progress(px(0.0), top), 1.0);
    }
}
