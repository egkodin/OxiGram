//! The macOS title bar in the theme's colours (lib_ui's mac `TitleWidget`):
//! `titleBg` or `titleBgActive` with the `titleShadow` line, the app's name
//! in the middle, and the system's traffic lights over its left end. The
//! native bar is made transparent so the window's frame follows the theme.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use style::{ColorId, ColorRef};
use ui::color;

/// Set by the app when its window's native bar is transparent; windows
/// without it (the snapshots) keep their content at the top.
#[derive(Debug)]
pub struct Custom;

impl gpui::Global for Custom {}

/// Draws the bar in every window from now on.
pub fn enable(cx: &mut App) {
    cx.set_global(Custom);
}

/// The native title bar's height on macOS 11 and later.
pub const HEIGHT: f32 = 28.0;
/// `font.setBold(true); font.setPixelSize(13)` in `PreviewWindowTitle`.
const FONT_SIZE: f32 = 13.0;

/// The bar with `title`; dragging it moves the window and a double click
/// zooms it, as the native bar does.
pub fn title_bar(title: &'static str, window: &Window, cx: &App) -> impl IntoElement + use<> {
    let (bg, fg) = if window.is_window_active() {
        (ColorId::TitleBgActive, ColorId::TitleFgActive)
    } else {
        (ColorId::TitleBg, ColorId::TitleFg)
    };
    // Moving starts on the first move after a press, so a double click
    // still reaches `on_click` (Zed's `PlatformTitleBar`).
    let pressed = Rc::new(Cell::new(false));
    let (on_down, on_up, on_move) = (pressed.clone(), pressed.clone(), pressed);
    div()
        .id("title-bar")
        .flex_none()
        .h(px(HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(color(cx, ColorRef::Palette(bg)))
        .border_b_1()
        .border_color(color(cx, ColorRef::Palette(ColorId::TitleShadow)))
        .text_size(px(FONT_SIZE))
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(color(cx, ColorRef::Palette(fg)))
        .child(title)
        .on_mouse_down(MouseButton::Left, move |_, _, _| on_down.set(true))
        .on_mouse_up(MouseButton::Left, move |_, _, _| on_up.set(false))
        .on_mouse_move(move |_, window, _| {
            if on_move.replace(false) {
                window.start_window_move();
            }
        })
        .on_click(|event, window, _| {
            if event.click_count() == 2 {
                window.titlebar_double_click();
            }
        })
}
