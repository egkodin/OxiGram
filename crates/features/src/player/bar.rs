//! The bar above the chat while a song plays (`Media::Player::Widget`):
//! previous, play and pause, next, "Performer – Title", the time, close,
//! and the played line along the bottom, which a click seeks; resting on
//! the name opens the playlist panel.

use std::cell::Cell;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Bounds, DispatchPhase, Entity, FontWeight, HitboxBehavior, Hsla,
    InteractiveElement as _, IntoElement as _, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement as _, PathBuilder, Pixels, Point, StatefulInteractiveElement as _, Styled as _,
    Window, canvas, div, point, px, size,
};
use style::media_player::{
    MEDIA_PLAYER_CLOSE, MEDIA_PLAYER_HEIGHT, MEDIA_PLAYER_NAME, MEDIA_PLAYER_NEXT_BUTTON,
    MEDIA_PLAYER_NEXT_DISABLED_ICON, MEDIA_PLAYER_PADDING, MEDIA_PLAYER_PLAY_BUTTON,
    MEDIA_PLAYER_PLAY_LEFT, MEDIA_PLAYER_PLAY_SKIP, MEDIA_PLAYER_PLAY_TOP, MEDIA_PLAYER_PLAYBACK,
    MEDIA_PLAYER_PREVIOUS_BUTTON, MEDIA_PLAYER_PREVIOUS_DISABLED_ICON, MEDIA_PLAYER_SPEED_BUTTON,
    MEDIA_PLAYER_TIME,
};
use style::{ColorId, ColorRef};
use ui::{IconButton, color, font, font_height};

use super::{Player, Sound, Step, Track};
use crate::px_of;
use crate::speed::Speed;

/// The name's and the time's top: `mediaPlayerNameTop` is their baseline.
// shortcut: the 13 px font's ascent taken as 14 px; measure it if the labels sit off by a pixel.
const LABEL_TOP: f32 = 22.0 - 14.0;

/// The bar for the song, voice or round playing; none for nothing.
// shortcut: no volume, repeat or order; with row 302.
pub(crate) fn bar(player: &Entity<Player>, cx: &App) -> Option<AnyElement> {
    let state = player.read(cx);
    let playing = state.current()?;
    let (performer, title, is_song) = match &playing.track {
        Track::Song { performer, title } => (performer, title, true),
        Track::Voice { name, date } => (name, date, false),
    };
    let sound = &playing.sound;
    let (previous, next) = (state.can_step(Step::Previous), state.can_step(Step::Next));
    // `handlePlaylistUpdate`: the skip buttons only while either end is open.
    let skips = previous || next;
    let play_width = px_of(MEDIA_PLAYER_PLAY_BUTTON.size.width);
    let skip_width = px_of(MEDIA_PLAYER_NEXT_BUTTON.width) + px_of(MEDIA_PLAYER_PLAY_SKIP);
    let controls = div()
        .absolute()
        .left(px_of(MEDIA_PLAYER_PLAY_LEFT))
        .top(px_of(MEDIA_PLAYER_PLAY_TOP))
        .flex()
        .gap(px_of(MEDIA_PLAYER_PLAY_SKIP))
        .children(skips.then(|| skip_button(player, Step::Previous, previous)))
        .child(play_button(player, sound.is_paused(), cx))
        .children(skips.then(|| skip_button(player, Step::Next, next)));
    let left = px_of(MEDIA_PLAYER_PLAY_LEFT)
        + play_width
        + if skips { skip_width * 2.0 } else { px(0.0) }
        + px_of(MEDIA_PLAYER_PADDING);
    let speed_width = px_of(MEDIA_PLAYER_SPEED_BUTTON.size.width);
    let right = px_of(MEDIA_PLAYER_CLOSE.width) + speed_width + px_of(MEDIA_PLAYER_PADDING);
    let labels = div()
        .absolute()
        .top(px(LABEL_TOP))
        .left(left)
        .right(right)
        .flex()
        .whitespace_nowrap()
        .child(name(performer, title, color(cx, MEDIA_PLAYER_NAME.text_fg)))
        .child(time(sound, cx));
    let close = div()
        .absolute()
        .top(px_of(MEDIA_PLAYER_PLAY_TOP))
        .right_0()
        .flex()
        .items_center()
        .child(speed_button(player, state.speed(cx), cx))
        .child(
            IconButton::new("media-player-close", &MEDIA_PLAYER_CLOSE).on_click({
                let player = player.clone();
                move |_, _, cx| player.update(cx, |player, cx| player.stop(cx))
            }),
        );
    let element = div()
        .relative()
        .flex_none()
        .w_full()
        .h(px_of(MEDIA_PLAYER_HEIGHT))
        .bg(palette(cx, ColorId::MediaPlayerBg))
        .border_b_1()
        .border_color(palette(cx, ColorId::ShadowFg))
        .child(controls)
        .child(labels)
        .child(close)
        // tdesktop draws the played line and the playlist for songs only.
        .when(is_song, |bar| {
            bar.child(playback(player, sound.progress(), cx))
                .child(labels_hover(player, left, right))
                .children(super::panel::panel(player, cx))
        })
        .children(state.speed_menu_overlay());
    Some(element.into_any_element())
}

/// `mediaPlayerSpeedButton`: a click toggles the faster speed, a right
/// click opens the presets.
fn speed_button(player: &Entity<Player>, speed: f32, cx: &App) -> AnyElement {
    let (toggle, menu) = (player.clone(), player.clone());
    crate::speed::button(
        "media-player-speed",
        &MEDIA_PLAYER_SPEED_BUTTON,
        speed,
        move |_, cx| toggle.update(cx, |player, cx| player.change_speed(Speed::toggled, cx)),
        move |window, cx| menu.update(cx, |player, cx| player.show_speed_menu(window, cx)),
        cx,
    )
}

/// `mediaPlayerPreviousButton` and `mediaPlayerNextButton`; the end that is
/// closed shows its dim icon and does nothing.
fn skip_button(player: &Entity<Player>, step: Step, is_enabled: bool) -> AnyElement {
    let (id, style, disabled) = match step {
        Step::Previous => (
            "media-player-previous",
            &MEDIA_PLAYER_PREVIOUS_BUTTON,
            MEDIA_PLAYER_PREVIOUS_DISABLED_ICON,
        ),
        Step::Next => (
            "media-player-next",
            &MEDIA_PLAYER_NEXT_BUTTON,
            MEDIA_PLAYER_NEXT_DISABLED_ICON,
        ),
    };
    if !is_enabled {
        return div()
            .relative()
            .w(px_of(style.width))
            .h(px_of(style.height))
            .child(
                div()
                    .absolute()
                    .left(px_of(style.icon_position.x))
                    .top(px_of(style.icon_position.y))
                    .child(ui::icon(disabled)),
            )
            .into_any_element();
    }
    let player = player.clone();
    IconButton::new(id, style)
        .on_click(move |_, _, cx| player.update(cx, |player, cx| player.step(step, cx)))
        .into_any_element()
}

fn palette(cx: &App, id: ColorId) -> Hsla {
    color(cx, ColorRef::Palette(id))
}

/// Follows the pointer over the name and the time, above the played line
/// (`updateOverLabelsState`), for the playlist panel.
fn labels_hover(player: &Entity<Player>, left: Pixels, right: Pixels) -> AnyElement {
    let player = player.clone();
    let line = px_of(MEDIA_PLAYER_PLAYBACK.full_width);
    canvas(
        move |bounds, window, _| {
            let labels = Bounds::from_corners(
                point(bounds.left() + left, bounds.top()),
                point(bounds.right() - right, bounds.bottom() - line),
            );
            window.insert_hitbox(labels, HitboxBehavior::Normal)
        },
        move |bounds, hitbox, window, _| {
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                let is_over = hitbox.is_hovered(window);
                if phase != DispatchPhase::Bubble || player.read(cx).is_over_labels() == is_over {
                    return;
                }
                let x = event.position.x - bounds.left();
                player.update(cx, |player, cx| player.hover_labels(is_over, x, cx));
            });
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

/// The performer in semibold, then the title (`textWithEntities(true)`).
pub(super) fn name(performer: &str, title: &str, fg: Hsla) -> AnyElement {
    let style = &MEDIA_PLAYER_NAME.style.font;
    let text = if performer.is_empty() {
        title.to_owned()
    } else {
        format!(" \u{2013} {title}")
    };
    div()
        .flex()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .font(font(*style))
        .text_size(px_of(style.size))
        .line_height(font_height(*style))
        .text_color(fg)
        .when_some_performer(performer)
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .child(text),
        )
        .into_any_element()
}

/// The played time (`FormatDurationText` of the position).
fn time(sound: &Sound, cx: &App) -> AnyElement {
    let label = &MEDIA_PLAYER_TIME;
    let seconds = i32::try_from(sound.position().as_secs()).unwrap_or(i32::MAX);
    div()
        .flex_none()
        .pl(px_of(MEDIA_PLAYER_PADDING))
        .font(font(label.font))
        .text_size(px_of(label.font.size))
        .line_height(font_height(label.font))
        .text_color(color(cx, label.text_fg))
        .child(crate::history::duration_text(seconds))
        .into_any_element()
}

/// `MediaPlayerPlayButton`: a click plays or pauses.
fn play_button(player: &Entity<Player>, is_paused: bool, cx: &App) -> AnyElement {
    let button = &MEDIA_PLAYER_PLAY_BUTTON;
    let fg = palette(cx, ColorId::MediaPlayerActiveFg);
    let player = player.clone();
    div()
        .id("media-player-play")
        .w(px_of(button.size.width))
        .h(px_of(button.size.height))
        .cursor_pointer()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| paint_play_icon(bounds, is_paused, fg, window),
            )
            .size_full(),
        )
        .on_click(move |_, _, cx| {
            player.update(cx, |player, cx| {
                if let Some(playing) = &player.playing {
                    if playing.sound.is_paused() {
                        playing.sound.resume();
                    } else {
                        playing.sound.pause();
                    }
                }
                cx.notify();
            });
        })
        .into_any_element()
}

/// The triangle while paused, the two bars while playing, in the icon's
/// square at `iconPosition`.
// shortcut: square corners and no morph between the two; tdesktop rounds them (`playRadius`) and animates over 200 ms.
fn paint_play_icon(bounds: Bounds<Pixels>, is_paused: bool, fg: Hsla, window: &mut Window) {
    let icon = &MEDIA_PLAYER_PLAY_BUTTON.icon;
    let position = MEDIA_PLAYER_PLAY_BUTTON.icon_position;
    let origin = bounds.origin + point(px_of(position.x), px_of(position.y));
    let at = |x: style::Px, y: style::Px| -> Point<Pixels> { origin + point(px_of(x), px_of(y)) };
    if is_paused {
        let (position, extent) = (icon.play_position, icon.play_size);
        let mut builder = PathBuilder::fill();
        builder.add_polygon(
            &[
                at(position.x, position.y),
                at(
                    style::Px(position.x.0 + extent.width.0),
                    style::Px(position.y.0 + extent.height.0 / 2),
                ),
                at(position.x, style::Px(position.y.0 + extent.height.0)),
            ],
            true,
        );
        if let Ok(path) = builder.build() {
            window.paint_path(path, fg);
        }
        return;
    }
    let (position, extent) = (icon.pause_position, icon.pause_size);
    let bar = size(px_of(icon.pause_bar_width), px_of(extent.height));
    let second = style::Px(position.x.0 + extent.width.0 - icon.pause_bar_width.0);
    for x in [position.x, second] {
        window.paint_quad(gpui::fill(Bounds::new(at(x, position.y), bar), fg));
    }
}

/// `mediaPlayerPlayback`: the played part of a thin line along the bottom;
/// a click there seeks.
fn playback(player: &Entity<Player>, share: f32, cx: &App) -> AnyElement {
    let slider = &MEDIA_PLAYER_PLAYBACK;
    let (active, inactive) = (color(cx, slider.active_fg), color(cx, slider.inactive_fg));
    let line = px_of(slider.line_width);
    let bounds = Rc::new(Cell::new(Bounds::default()));
    let measured = bounds.clone();
    let player = player.clone();
    div()
        .id("media-player-playback")
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .h(px_of(slider.full_width))
        .cursor_pointer()
        .child(
            canvas(
                move |bar, _, _| measured.set(bar),
                move |bar, (), window, _| {
                    let top = bar.bottom() - line;
                    let played = bar.size.width * share;
                    window.paint_quad(gpui::fill(
                        Bounds::new(point(bar.left(), top), size(bar.size.width, line)),
                        inactive,
                    ));
                    window.paint_quad(gpui::fill(
                        Bounds::new(point(bar.left(), top), size(played, line)),
                        active,
                    ));
                },
            )
            .size_full(),
        )
        .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
            cx.stop_propagation();
            let bar = bounds.get();
            if bar.size.width <= px(0.0) {
                return;
            }
            let share = ((event.position.x - bar.left()) / bar.size.width).clamp(0.0, 1.0);
            player.update(cx, |player, cx| {
                if let Some(playing) = &player.playing {
                    playing.sound.seek(share);
                }
                cx.notify();
            });
        })
        .into_any_element()
}

/// The performer in semibold before the rest of the name.
trait PerformerExt {
    fn when_some_performer(self, performer: &str) -> Self;
}

impl PerformerExt for gpui::Div {
    fn when_some_performer(self, performer: &str) -> Self {
        if performer.is_empty() {
            return self;
        }
        self.child(
            div()
                .flex_none()
                .font_weight(FontWeight::SEMIBOLD)
                .child(performer.to_owned()),
        )
    }
}
