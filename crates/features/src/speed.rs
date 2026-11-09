//! Playback speed (`Media::Player::SpeedController`): a click on the
//! button switches between normal and the last faster speed, its menu
//! picks one of tdesktop's presets; each kind of media keeps its own.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement as _, MouseButton, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};
use i18n::{Key, key};
use serde::{Deserialize, Serialize};
use style::media_player::MediaSpeedButton;
use style::menu_icons::{
    PLAYER_SPEED_FAST, PLAYER_SPEED_FAST_ACTIVE, PLAYER_SPEED_MEDIUM, PLAYER_SPEED_MEDIUM_ACTIVE,
    PLAYER_SPEED_NORMAL, PLAYER_SPEED_NORMAL_ACTIVE, PLAYER_SPEED_SLOW, PLAYER_SPEED_SLOW_ACTIVE,
    PLAYER_SPEED_SUPER_FAST, PLAYER_SPEED_SUPER_FAST_ACTIVE, PLAYER_SPEED_VERY_FAST,
    PLAYER_SPEED_VERY_FAST_ACTIVE,
};
use style::widgets::POPUP_MENU_WITH_ICONS;
use ui::{PopupMenu, color, font, font_height};

use crate::lang::tr;
use crate::px_of;

/// `kSpedUpDefault`: the faster speed before one is chosen.
const SPED_UP_DEFAULT: f32 = 1.7;

/// What a speed belongs to (`voicePlaybackSpeed`, `audioPlaybackSpeed`,
/// `videoPlaybackSpeed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Voices and round videos.
    Voice,
    /// Songs.
    Audio,
    /// Videos in the viewer.
    Video,
}

/// A kind's speed: normal, or the faster one it keeps for the toggle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Speed {
    sped_up: f32,
    is_sped_up: bool,
}

impl Default for Speed {
    fn default() -> Self {
        Self {
            sped_up: SPED_UP_DEFAULT,
            is_sped_up: false,
        }
    }
}

impl Speed {
    pub(crate) fn value(self) -> f32 {
        if self.is_sped_up { self.sped_up } else { 1.0 }
    }

    /// A click on the button: normal and the faster speed swap.
    #[must_use]
    pub(crate) fn toggled(self) -> Self {
        Self {
            is_sped_up: !self.is_sped_up,
            ..self
        }
    }

    /// `speed` picked from the menu; normal keeps the faster one for later.
    #[must_use]
    pub(crate) fn chosen(self, speed: f32) -> Self {
        if is_normal(speed) {
            return Self {
                is_sped_up: false,
                ..self
            };
        }
        Self {
            sped_up: speed.clamp(media::MIN_SPEED, media::MAX_SPEED),
            is_sped_up: true,
        }
    }
}

/// `EqualSpeeds` with 1: the same to a tenth.
fn is_normal(speed: f32) -> bool {
    tenths(speed) == 10
}

#[expect(clippy::cast_possible_truncation, reason = "speeds are small")]
fn tenths(speed: f32) -> i32 {
    (speed * 10.0).round() as i32
}

/// "1.7X" (`SpeedText`).
pub(crate) fn text(speed: f32) -> String {
    let tenths = tenths(speed);
    if tenths % 10 == 0 {
        format!("{}X", tenths / 10)
    } else {
        format!("{}.{}X", tenths / 10, tenths % 10)
    }
}

pub(crate) fn current(kind: Kind, cx: &App) -> f32 {
    crate::settings::speed(kind, cx).value()
}

/// Saves `edit` of `kind`'s speed and returns the new value.
pub(crate) fn change(kind: Kind, edit: impl FnOnce(Speed) -> Speed, cx: &mut App) -> f32 {
    let speed = edit(crate::settings::speed(kind, cx));
    crate::settings::set_speed(kind, speed, cx);
    speed.value()
}

/// tdesktop's points in the speed menu (`FillSpeedMenu`).
const PRESETS: [(f32, Key, style::Icon, style::Icon); 6] = [
    (
        0.5,
        key::lng_voice_speed_slow,
        PLAYER_SPEED_SLOW,
        PLAYER_SPEED_SLOW_ACTIVE,
    ),
    (
        1.0,
        key::lng_voice_speed_normal,
        PLAYER_SPEED_NORMAL,
        PLAYER_SPEED_NORMAL_ACTIVE,
    ),
    (
        1.2,
        key::lng_voice_speed_medium,
        PLAYER_SPEED_MEDIUM,
        PLAYER_SPEED_MEDIUM_ACTIVE,
    ),
    (
        1.5,
        key::lng_voice_speed_fast,
        PLAYER_SPEED_FAST,
        PLAYER_SPEED_FAST_ACTIVE,
    ),
    (
        1.7,
        key::lng_voice_speed_very_fast,
        PLAYER_SPEED_VERY_FAST,
        PLAYER_SPEED_VERY_FAST_ACTIVE,
    ),
    (
        2.0,
        key::lng_voice_speed_super_fast,
        PLAYER_SPEED_SUPER_FAST,
        PLAYER_SPEED_SUPER_FAST_ACTIVE,
    ),
];

/// What a choice does with the speed picked.
pub(crate) type OnChoose = Rc<dyn Fn(f32, &mut Window, &mut App)>;

/// The presets, the one playing in its active icon.
// shortcut: presets only; tdesktop's menu also has a 0.5-2.5 slider on top, and drops down on hover rather than on a right click.
pub(crate) fn menu(
    current: f32,
    on_choose: &OnChoose,
    cx: &mut gpui::Context<PopupMenu>,
) -> PopupMenu {
    let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
    for (speed, text, icon, active) in PRESETS {
        let on_choose = on_choose.clone();
        let icon = if tenths(speed) == tenths(current) {
            active
        } else {
            icon
        };
        menu.option(tr(cx, text), Some(icon), move |window, cx| {
            on_choose(speed, window, cx);
        });
    }
    menu
}

/// The speed button (`SpeedButton`): the icon with the speed over it,
/// in the active color while faster than normal. A click toggles, a right
/// click opens the presets.
pub(crate) fn button(
    id: &'static str,
    st: &'static MediaSpeedButton,
    speed: f32,
    on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    on_menu: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let fg = if is_normal(speed) {
        st.fg
    } else {
        st.active_fg
    };
    div()
        .id(id)
        .relative()
        .flex_none()
        .w(px_of(st.size.width))
        .h(px_of(st.size.height))
        .cursor_pointer()
        .child(
            div()
                .absolute()
                .left(px_of(st.padding.left))
                .right(px_of(st.padding.right))
                .top(px_of(st.padding.top))
                .bottom(px_of(st.padding.bottom))
                .flex()
                .items_center()
                .justify_center()
                .child(ui::icon(st.icon).color(fg))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .font(font(st.font))
                        .text_size(px_of(st.font.size))
                        .line_height(font_height(st.font))
                        .text_color(color(cx, fg))
                        .child(text(speed)),
                ),
        )
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            on_toggle(window, cx);
        })
        .on_mouse_down(MouseButton::Right, move |_, window, cx| {
            cx.stop_propagation();
            on_menu(window, cx);
        })
        .into_any_element()
}

/// An open presets menu at `position`, above the window's content.
pub(crate) fn overlay(
    position: gpui::Point<gpui::Pixels>,
    menu: gpui::Entity<PopupMenu>,
) -> AnyElement {
    gpui::deferred(
        gpui::anchored()
            .position(position)
            .snap_to_window_with_margin(gpui::px(8.0))
            .child(div().child(menu)),
    )
    .with_priority(1)
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toggle_keeps_the_faster_speed() {
        let speed = Speed::default();
        assert!((speed.value() - 1.0).abs() < f32::EPSILON);
        assert!((speed.toggled().value() - SPED_UP_DEFAULT).abs() < f32::EPSILON);
        let fast = speed.chosen(2.0);
        assert!((fast.value() - 2.0).abs() < f32::EPSILON);
        let normal = fast.chosen(1.0);
        assert!((normal.value() - 1.0).abs() < f32::EPSILON);
        assert!((normal.toggled().value() - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn speeds_read_to_a_tenth() {
        assert_eq!(text(1.0), "1X");
        assert_eq!(text(1.7), "1.7X");
        assert_eq!(text(0.5), "0.5X");
        assert_eq!(text(2.04), "2X");
    }
}
