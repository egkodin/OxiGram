//! lib_ui's single-line `MaskedInputField`: text margins, a placeholder that
//! shrinks above the text on focus, and a bottom border that grows from the
//! click point. IME composition goes through gpui's input handler.

use std::ops::Range;
use std::time::Duration;

use gpui::{
    App, Bounds, Context, CursorStyle, ElementId, EntityInputHandler, EventEmitter, FocusHandle,
    Focusable, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    ShapedLine, SharedString, Task, UTF16Selection, Window, actions, div, point, prelude::*, px,
};
use style::Align;
use style::widgets::InputField as InputStyle;

use crate::anim::{Easing, Simple};
use crate::font::font_height;
use crate::text_buffer::TextBuffer;

actions!(
    input_field,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        Left,
        Right,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectWordLeft,
        SelectWordRight,
        SelectAll,
        Home,
        End,
        SelectToHome,
        SelectToEnd,
        Paste,
        Cut,
        Copy,
        Submit,
        FocusNext,
        FocusPrevious,
    ]
);

const CONTEXT: &str = "InputField";
pub(crate) const BLINK_INTERVAL: Duration = Duration::from_millis(500);
/// Qt's password character when the font has it (`QCommonStyle`).
const PASSWORD_CHAR: char = '\u{2022}';

/// The word-move modifier: Option on macOS, Ctrl elsewhere.
/// A shortcut with the platform's command key: Cmd on macOS, Ctrl elsewhere.
/// gpui reads `secondary-` only when it is told the OS, which
/// `KeyBinding::new` is not.
#[macro_export]
macro_rules! secondary {
    ($keys:literal) => {
        if cfg!(target_os = "macos") {
            concat!("cmd-", $keys)
        } else {
            concat!("ctrl-", $keys)
        }
    };
}

mod element;
#[macro_use]
mod keys;

use element::FieldElement;

pub(crate) use element::{plain_run, text_runs};
pub(crate) use keys::bind_keys;

/// What the field shows for its text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InputMode {
    #[default]
    Text,
    /// Every character as a bullet (`PasswordInput`).
    Password,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Changed,
    /// Enter was pressed.
    Submitted,
    Focused,
    Blurred,
    /// Backspace with the cursor at the start and nothing selected.
    BackspaceAtStart,
}

/// Rewrites the text after each edit (`MaskedInputField::correctValue`),
/// and may draw a hint after it, such as the rest of a phone number pattern.
pub trait Corrector: 'static {
    fn correct(&mut self, was: &str, was_cursor: usize, now: &mut String, now_cursor: &mut usize);

    /// Text drawn right after the field's text in the placeholder color.
    fn trailing_hint(&self, _text: &str) -> Option<SharedString> {
        None
    }
}

pub struct InputField {
    focus: FocusHandle,
    style: &'static InputStyle,
    mode: InputMode,
    placeholder: SharedString,
    buffer: TextBuffer,
    max_length: Option<usize>,
    corrector: Option<Box<dyn Corrector>>,
    is_selecting: bool,
    focused: bool,
    error: bool,
    placeholder_shifted: bool,
    border_visible: bool,
    border_start: Option<Pixels>,
    focused_animation: Simple,
    error_animation: Simple,
    placeholder_animation: Simple,
    border_shown: Simple,
    border_opacity: Simple,
    cursor_visible: bool,
    blink: Option<Task<()>>,
    last_line: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    _subscriptions: [gpui::Subscription; 2],
}

/// Leaves the text out: it may be a password or a login code.
impl std::fmt::Debug for InputField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputField")
            .field("mode", &self.mode)
            .field("focused", &self.focused)
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl EventEmitter<InputEvent> for InputField {}

impl Focusable for InputField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl InputField {
    pub fn new(
        style: &'static InputStyle,
        placeholder: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let subscriptions = [
            cx.on_focus(&focus, window, |this, window, cx| {
                this.set_focused(true, window, cx)
            }),
            cx.on_blur(&focus, window, |this, window, cx| {
                this.set_focused(false, window, cx)
            }),
        ];
        Self {
            focus,
            style,
            mode: InputMode::Text,
            placeholder: placeholder.into(),
            buffer: TextBuffer::default(),
            max_length: None,
            corrector: None,
            is_selecting: false,
            focused: false,
            error: false,
            placeholder_shifted: false,
            border_visible: false,
            border_start: None,
            focused_animation: Simple::default(),
            error_animation: Simple::default(),
            placeholder_animation: Simple::default(),
            border_shown: Simple::default(),
            border_opacity: Simple::default(),
            cursor_visible: true,
            blink: None,
            last_line: None,
            last_bounds: None,
            _subscriptions: subscriptions,
        }
    }

    #[must_use]
    pub fn with_mode(mut self, mode: InputMode) -> Self {
        self.mode = mode;
        self
    }

    #[must_use]
    pub fn with_max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    #[must_use]
    pub fn with_corrector(mut self, corrector: impl Corrector) -> Self {
        self.corrector = Some(Box::new(corrector));
        self
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.buffer.text
    }

    pub fn corrector_mut(&mut self) -> Option<&mut (dyn Corrector + 'static)> {
        self.corrector.as_deref_mut()
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    /// Replaces the text, puts the cursor at `cursor` (the end if `None`) and
    /// runs the corrector, as typing would.
    pub fn set_text(&mut self, text: &str, cursor: Option<usize>, cx: &mut Context<Self>) {
        let was = std::mem::take(&mut self.buffer.text);
        let was_cursor = self.buffer.cursor();
        self.buffer.text = text.to_owned();
        self.buffer
            .move_to(cursor.unwrap_or(self.buffer.text.len()));
        self.correct(&was, was_cursor);
        self.after_edit(cx);
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.buffer.select_all();
        cx.notify();
    }

    /// `showError`: red border and placeholder until the next edit.
    pub fn show_error(&mut self, cx: &mut Context<Self>) {
        self.set_error(true, cx);
    }

    pub fn hide_error(&mut self, cx: &mut Context<Self>) {
        self.set_error(false, cx);
    }

    fn set_error(&mut self, error: bool, cx: &mut Context<Self>) {
        if self.error != error {
            self.error = error;
            let (from, to) = if error { (0.0, 1.0) } else { (1.0, 0.0) };
            self.error_animation
                .start(from, to, self.style.duration, Easing::Linear);
            self.start_border_animation();
            cx.notify();
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }

    fn set_focused(&mut self, focused: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        let (from, to) = if focused { (0.0, 1.0) } else { (1.0, 0.0) };
        self.focused_animation
            .start(from, to, self.style.duration, Easing::Linear);
        self.start_placeholder_animation();
        self.start_border_animation();
        if focused {
            self.restart_blink(window, cx);
            cx.emit(InputEvent::Focused);
        } else {
            self.blink = None;
            self.border_start = None;
            cx.emit(InputEvent::Blurred);
        }
        cx.notify();
    }

    fn start_placeholder_animation(&mut self) {
        let shifted = (self.focused && self.style.placeholder_scale > 0.0)
            || !self.buffer.text.is_empty()
            || self.buffer.marked.is_some();
        if self.placeholder_shifted != shifted {
            self.placeholder_shifted = shifted;
            let (from, to) = if shifted { (0.0, 1.0) } else { (1.0, 0.0) };
            self.placeholder_animation
                .start(from, to, self.style.duration, Easing::Linear);
        }
    }

    fn start_border_animation(&mut self) {
        let visible = self.error || self.focused;
        if self.border_visible == visible {
            return;
        }
        self.border_visible = visible;
        let duration = self.style.duration;
        if visible {
            if self.border_opacity.animating() {
                self.border_opacity
                    .start(0.0, 1.0, duration, Easing::Linear);
            } else {
                self.border_shown.start(0.0, 1.0, duration, Easing::Linear);
            }
        } else if self.border_shown.value(1.0) == 0.0 {
            self.border_shown.stop();
            self.border_opacity.stop();
        } else {
            self.border_opacity
                .start(1.0, 0.0, duration, Easing::Linear);
        }
    }

    fn restart_blink(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cursor_visible = true;
        self.blink = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(BLINK_INTERVAL).await;
                let alive = this.update(cx, |this, cx| {
                    this.cursor_visible = !this.cursor_visible;
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        }));
    }

    fn move_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.buffer.move_to(offset);
        self.restart_blink(window, cx);
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.buffer.select_to(offset);
        cx.notify();
    }
}

impl InputField {
    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(bounds) = self.last_bounds {
            self.border_start = Some(event.position.x - bounds.left());
        }
        self.is_selecting = true;
        let index = self.index_for_position(event.position);
        if event.modifiers.shift {
            self.select_to(index, cx);
        } else if event.click_count >= 2 {
            self.select_all(cx);
        } else {
            self.move_to(index, window, cx);
        }
        if !self.focus.is_focused(window) {
            self.focus.focus(window, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_position(event.position), cx);
        }
    }

    fn index_for_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (self.last_bounds, self.last_line.as_ref()) else {
            return 0;
        };
        let x = position.x - bounds.left() - self.text_left(bounds.size.width, line.width);
        self.display_to_text(line.closest_index_for_x(x))
    }

    /// The x of the text's start inside the field.
    fn text_left(&self, width: Pixels, text_width: Pixels) -> Pixels {
        let margins = self.style.text_margins;
        let left = px_of(margins.left);
        match self.style.text_align {
            Align::Top | Align::Center | Align::Bottom => {
                let inner = width - left - px_of(margins.right);
                left + (inner - text_width).max(px(0.0)) / 2.0
            }
            _ => left,
        }
    }

    /// The text as drawn: bullets in password mode.
    fn display_text(&self) -> String {
        match self.mode {
            InputMode::Text => self.buffer.text.clone(),
            InputMode::Password => self.buffer.text.chars().map(|_| PASSWORD_CHAR).collect(),
        }
    }

    fn text_to_display(&self, offset: usize) -> usize {
        match self.mode {
            InputMode::Text => offset,
            InputMode::Password => {
                self.buffer.text[..offset].chars().count() * PASSWORD_CHAR.len_utf8()
            }
        }
    }

    fn display_to_text(&self, offset: usize) -> usize {
        match self.mode {
            InputMode::Text => offset,
            InputMode::Password => {
                let chars = offset / PASSWORD_CHAR.len_utf8();
                self.buffer
                    .text
                    .char_indices()
                    .nth(chars)
                    .map_or(self.buffer.text.len(), |(index, _)| index)
            }
        }
    }

    fn correct(&mut self, was: &str, was_cursor: usize) {
        if let Some(max_length) = self.max_length
            && self.buffer.text.chars().count() > max_length
        {
            let cut = self
                .buffer
                .text
                .char_indices()
                .nth(max_length)
                .map_or(self.buffer.text.len(), |(index, _)| index);
            self.buffer.text.truncate(cut);
        }
        let mut cursor = self.buffer.cursor().min(self.buffer.text.len());
        if let Some(corrector) = self.corrector.as_mut() {
            corrector.correct(was, was_cursor, &mut self.buffer.text, &mut cursor);
        }
        self.buffer
            .move_to(floor_char_boundary(&self.buffer.text, cursor));
    }

    fn after_edit(&mut self, cx: &mut Context<Self>) {
        self.start_placeholder_animation();
        self.cursor_visible = true;
        cx.emit(InputEvent::Changed);
        cx.notify();
    }
}

fn floor_char_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

pub(crate) fn px_of(value: style::Px) -> Pixels {
    #[expect(clippy::cast_precision_loss, reason = "style sizes are small")]
    px(value.0 as f32)
}

impl EntityInputHandler for InputField {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.buffer.range_from_utf16(&range);
        actual_range.replace(self.buffer.range_to_utf16(&range));
        Some(self.buffer.text[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.buffer.range_to_utf16(&self.buffer.selected),
            reversed: self.buffer.reversed,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.buffer.selected = self.buffer.range_from_utf16(&range);
        self.buffer.reversed = false;
        cx.notify();
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.buffer.text.encode_utf16().count())
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buffer
            .marked
            .as_ref()
            .map(|range| self.buffer.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .as_ref()
            .map(|range| self.buffer.range_from_utf16(range))
            .unwrap_or_else(|| self.buffer.edited_range());
        let was = self.buffer.text.clone();
        let was_cursor = self.buffer.cursor();
        self.buffer.replace(range, new_text);
        self.correct(&was, was_cursor);
        self.restart_blink(window, cx);
        if self.buffer.text != was {
            self.hide_error(cx);
        }
        self.after_edit(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .as_ref()
            .map(|range| self.buffer.range_from_utf16(range))
            .unwrap_or_else(|| self.buffer.edited_range());
        self.buffer.text.replace_range(range.clone(), new_text);
        self.buffer.marked =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        let start_utf16 = self.buffer.offset_to_utf16(range.start);
        let inserted_utf16 = new_text.encode_utf16().count();
        self.buffer.selected = new_selected.map_or(
            range.start + new_text.len()..range.start + new_text.len(),
            |selected| {
                self.buffer.range_from_utf16(
                    &(start_utf16 + selected.start.min(inserted_utf16)
                        ..start_utf16 + selected.end.min(inserted_utf16)),
                )
            },
        );
        self.buffer.reversed = false;
        self.start_placeholder_animation();
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.last_line.as_ref()?;
        let range = self.buffer.range_from_utf16(&range);
        let left = bounds.left() + self.text_left(bounds.size.width, line.width);
        let top = bounds.top() + px_of(self.style.text_margins.top);
        let height = font_height(self.style.style.font);
        Some(Bounds::from_corners(
            point(
                left + line.x_for_index(self.text_to_display(range.start)),
                top,
            ),
            point(
                left + line.x_for_index(self.text_to_display(range.end)),
                top + height,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let index = self.index_for_position(point);
        Some(self.buffer.offset_to_utf16(index))
    }
}

impl Render for InputField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(ElementId::Name("input-field".into()))
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            // A zero width means the parent sets it, as with a Qt geometry.
            .when(self.style.width.0 == 0, Styled::w_full)
            .when(self.style.width.0 > 0, |this| {
                this.w(px_of(self.style.width))
            })
            .h(px_of(self.style.height_min))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::select_all_action))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_to_home))
            .on_action(cx.listener(Self::select_to_end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::focus_next))
            .on_action(cx.listener(Self::focus_previous))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(FieldElement { input: cx.entity() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floors_offsets_to_char_boundaries() {
        assert_eq!(floor_char_boundary("aé", 2), 1);
        assert_eq!(floor_char_boundary("aé", 9), 3);
    }
}
