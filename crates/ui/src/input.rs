//! lib_ui's single-line `MaskedInputField`: text margins, a placeholder that
//! shrinks above the text on focus, and a bottom border that grows from the
//! click point. IME composition goes through gpui's input handler.

use std::ops::Range;
use std::time::Duration;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ShapedLine,
    SharedString, Style, Task, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div, fill,
    point, prelude::*, px, relative, size,
};
use style::Align;
use style::widgets::InputField as InputStyle;
use unicode_segmentation::UnicodeSegmentation as _;

use crate::anim::{Easing, Simple, interpolate, mix};
use crate::font::{font, font_height};
use crate::theme::{Theme, hsla};

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

macro_rules! word {
    ($keys:literal) => {
        if cfg!(target_os = "macos") {
            concat!("alt-", $keys)
        } else {
            concat!("ctrl-", $keys)
        }
    };
}

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);
    let mut bindings = vec![
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("shift-backspace", Backspace, context),
        KeyBinding::new(word!("backspace"), DeleteWordLeft, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new(word!("left"), WordLeft, context),
        KeyBinding::new(word!("right"), WordRight, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new(word!("shift-left"), SelectWordLeft, context),
        KeyBinding::new(word!("shift-right"), SelectWordRight, context),
        KeyBinding::new(crate::secondary!("a"), SelectAll, context),
        KeyBinding::new("home", Home, context),
        KeyBinding::new("end", End, context),
        KeyBinding::new("shift-home", SelectToHome, context),
        KeyBinding::new("shift-end", SelectToEnd, context),
        KeyBinding::new(crate::secondary!("v"), Paste, context),
        KeyBinding::new(crate::secondary!("x"), Cut, context),
        KeyBinding::new(crate::secondary!("c"), Copy, context),
        KeyBinding::new("enter", Submit, context),
        KeyBinding::new("tab", FocusNext, context),
        KeyBinding::new("shift-tab", FocusPrevious, context),
    ];
    if cfg!(target_os = "macos") {
        bindings.extend([
            KeyBinding::new("cmd-left", Home, context),
            KeyBinding::new("cmd-right", End, context),
            KeyBinding::new("cmd-shift-left", SelectToHome, context),
            KeyBinding::new("cmd-shift-right", SelectToEnd, context),
            KeyBinding::new("cmd-backspace", SelectToHome, context),
        ]);
    }
    cx.bind_keys(bindings);
}

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
    text: String,
    selected: Range<usize>,
    reversed: bool,
    marked: Option<Range<usize>>,
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
            text: String::new(),
            selected: 0..0,
            reversed: false,
            marked: None,
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
        &self.text
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
        let was = std::mem::take(&mut self.text);
        let was_cursor = self.cursor();
        self.text = text.to_owned();
        let cursor = cursor.unwrap_or(self.text.len()).min(self.text.len());
        self.selected = cursor..cursor;
        self.correct(&was, was_cursor);
        self.after_edit(cx);
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.selected = 0..self.text.len();
        self.reversed = false;
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
            || !self.text.is_empty()
            || self.marked.is_some();
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

    fn cursor(&self) -> usize {
        if self.reversed {
            self.selected.start
        } else {
            self.selected.end
        }
    }

    fn move_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = offset..offset;
        self.reversed = false;
        self.restart_blink(window, cx);
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.reversed {
            self.selected.start = offset;
        } else {
            self.selected.end = offset;
        }
        if self.selected.end < self.selected.start {
            self.reversed = !self.reversed;
            self.selected = self.selected.end..self.selected.start;
        }
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.text.len())
    }

    fn previous_word(&self, offset: usize) -> usize {
        self.text
            .split_word_bound_indices()
            .rev()
            .find(|(index, word)| *index < offset && word.chars().any(char::is_alphanumeric))
            .map_or(0, |(index, _)| index)
    }

    fn next_word(&self, offset: usize) -> usize {
        self.text
            .split_word_bound_indices()
            .find(|(index, word)| {
                index + word.len() > offset && word.chars().any(char::is_alphanumeric)
            })
            .map_or(self.text.len(), |(index, word)| index + word.len())
    }

    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        let target = if self.selected.is_empty() {
            self.previous_boundary(self.cursor())
        } else {
            self.selected.start
        };
        self.move_to(target, window, cx);
    }

    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        let target = if self.selected.is_empty() {
            self.next_boundary(self.cursor())
        } else {
            self.selected.end
        };
        self.move_to(target, window, cx);
    }

    fn word_left(&mut self, _: &WordLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.previous_word(self.cursor()), window, cx);
    }

    fn word_right(&mut self, _: &WordRight, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.next_word(self.cursor()), window, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor()), cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_word(self.cursor()), cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_word(self.cursor()), cx);
    }

    fn select_all_action(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all(cx);
    }

    fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, window, cx);
    }

    fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.text.len(), window, cx);
    }

    fn select_to_home(&mut self, _: &SelectToHome, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(0, cx);
    }

    fn select_to_end(&mut self, _: &SelectToEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.text.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            if self.cursor() == 0 {
                cx.emit(InputEvent::BackspaceAtStart);
                return;
            }
            self.select_to(self.previous_boundary(self.cursor()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected.is_empty() {
            self.select_to(self.previous_word(self.cursor()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            self.select_to(self.next_boundary(self.cursor()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace(['\n', '\r'], " "), window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected.is_empty() && self.mode == InputMode::Text {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.text[self.selected.clone()].to_owned(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected.is_empty() && self.mode == InputMode::Text {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.text[self.selected.clone()].to_owned(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(InputEvent::Submitted);
    }

    fn focus_next(&mut self, _: &FocusNext, window: &mut Window, cx: &mut Context<Self>) {
        window.focus_next(cx);
    }

    fn focus_previous(&mut self, _: &FocusPrevious, window: &mut Window, cx: &mut Context<Self>) {
        window.focus_prev(cx);
    }

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
            InputMode::Text => self.text.clone(),
            InputMode::Password => self.text.chars().map(|_| PASSWORD_CHAR).collect(),
        }
    }

    fn text_to_display(&self, offset: usize) -> usize {
        match self.mode {
            InputMode::Text => offset,
            InputMode::Password => self.text[..offset].chars().count() * PASSWORD_CHAR.len_utf8(),
        }
    }

    fn display_to_text(&self, offset: usize) -> usize {
        match self.mode {
            InputMode::Text => offset,
            InputMode::Password => {
                let chars = offset / PASSWORD_CHAR.len_utf8();
                self.text
                    .char_indices()
                    .nth(chars)
                    .map_or(self.text.len(), |(index, _)| index)
            }
        }
    }

    fn correct(&mut self, was: &str, was_cursor: usize) {
        if let Some(max_length) = self.max_length
            && self.text.chars().count() > max_length
        {
            let cut = self
                .text
                .char_indices()
                .nth(max_length)
                .map_or(self.text.len(), |(index, _)| index);
            self.text.truncate(cut);
        }
        let mut cursor = self.cursor().min(self.text.len());
        if let Some(corrector) = self.corrector.as_mut() {
            corrector.correct(was, was_cursor, &mut self.text, &mut cursor);
        }
        let cursor = floor_char_boundary(&self.text, cursor);
        self.selected = cursor..cursor;
        self.reversed = false;
    }

    fn after_edit(&mut self, cx: &mut Context<Self>) {
        self.start_placeholder_animation();
        self.cursor_visible = true;
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for c in self.text.chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += c.len_utf16();
            utf8 += c.len_utf8();
        }
        utf8
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .encode_utf16()
            .count()
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
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
        let range = self.range_from_utf16(&range);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.text[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected),
            reversed: self.reversed,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected = self.range_from_utf16(&range);
        self.reversed = false;
        cx.notify();
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.text.encode_utf16().count())
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
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
            .map(|range| self.range_from_utf16(range))
            .or(self.marked.clone())
            .unwrap_or(self.selected.clone());
        let was = self.text.clone();
        let was_cursor = self.cursor();
        self.text.replace_range(range.clone(), new_text);
        let cursor = range.start + new_text.len();
        self.selected = cursor..cursor;
        self.marked = None;
        self.correct(&was, was_cursor);
        self.restart_blink(window, cx);
        if self.text != was {
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
            .map(|range| self.range_from_utf16(range))
            .or(self.marked.clone())
            .unwrap_or(self.selected.clone());
        self.text.replace_range(range.clone(), new_text);
        self.marked = (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        let start_utf16 = self.offset_to_utf16(range.start);
        let inserted_utf16 = new_text.encode_utf16().count();
        self.selected = new_selected.map_or(
            range.start + new_text.len()..range.start + new_text.len(),
            |selected| {
                self.range_from_utf16(
                    &(start_utf16 + selected.start.min(inserted_utf16)
                        ..start_utf16 + selected.end.min(inserted_utf16)),
                )
            },
        );
        self.reversed = false;
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
        let range = self.range_from_utf16(&range);
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
        Some(self.offset_to_utf16(index))
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

/// Paints the field: background, border, placeholder, text, selection and
/// cursor, and registers the input handler.
struct FieldElement {
    input: Entity<InputField>,
}

impl IntoElement for FieldElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

struct Prepaint {
    quads: Vec<gpui::PaintQuad>,
    placeholder: Option<(ShapedLine, Point<Pixels>, Pixels)>,
    line: ShapedLine,
    hint: Option<ShapedLine>,
    origin: Point<Pixels>,
    line_height: Pixels,
    cursor: Option<gpui::PaintQuad>,
}

impl Element for FieldElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        let input = self.input.read(cx);
        let st = input.style;
        let palette = &cx.global::<Theme>().palette;
        let text_font = font(st.style.font);
        let font_size = px_of(st.style.font.size);
        let display = input.display_text();
        let selected =
            input.text_to_display(input.selected.start)..input.text_to_display(input.selected.end);
        let marked = input
            .marked
            .as_ref()
            .map(|range| input.text_to_display(range.start)..input.text_to_display(range.end));
        let runs = text_runs(
            &display,
            &text_font,
            hsla(st.text_fg.resolve(palette)),
            hsla(palette.history_text_in_fg_selected),
            &selected,
            marked.as_ref(),
        );
        let line = window
            .text_system()
            .shape_line(display.into(), font_size, &runs, None);
        let hint = input
            .corrector
            .as_ref()
            .and_then(|corrector| corrector.trailing_hint(&input.text))
            .map(|hint| {
                let run = plain_run(
                    hint.len(),
                    &text_font,
                    hsla(st.placeholder_fg.resolve(palette)),
                );
                window
                    .text_system()
                    .shape_line(hint, font_size, &[run], None)
            });
        // A pattern hint replaces the placeholder (`setPlaceholderHidden`).
        let placeholder = (!input.placeholder.is_empty() && hint.is_none())
            .then(|| placeholder_line(input, bounds, palette, window));

        let line_height = font_height(st.style.font);
        let origin = point(
            bounds.left() + input.text_left(bounds.size.width, line.width),
            bounds.top() + px_of(st.text_margins.top),
        );
        let mut quads = if st.border_radius.0 > 0 {
            vec![round_surrounding(input, bounds, palette, window)]
        } else {
            let mut quads = vec![fill(bounds, hsla(st.text_bg.resolve(palette)))];
            quads.extend(border_quads(input, bounds, palette, window));
            quads
        };
        if !selected.is_empty() {
            let start = line.x_for_index(selected.start);
            let end = line.x_for_index(selected.end);
            quads.push(fill(
                Bounds::from_corners(
                    point(origin.x + start, origin.y),
                    point(origin.x + end, origin.y + line_height),
                ),
                hsla(palette.msg_in_bg_selected),
            ));
        }
        let cursor = (input.focus.is_focused(window)
            && input.cursor_visible
            && selected.is_empty())
        .then(|| {
            let x = line.x_for_index(input.text_to_display(input.cursor()));
            fill(
                Bounds::new(point(origin.x + x, origin.y), size(px(1.0), line_height)),
                hsla(st.text_fg.resolve(palette)),
            )
        });
        Prepaint {
            quads,
            placeholder,
            line,
            hint,
            origin,
            line_height,
            cursor,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.input.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        for quad in prepaint.quads.drain(..) {
            window.paint_quad(quad);
        }
        if let Some((placeholder, origin, line_height)) = prepaint.placeholder.take() {
            window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                if let Err(error) =
                    placeholder.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx)
                {
                    tracing::warn!(%error, "cannot paint a placeholder");
                }
            });
        }
        let Prepaint {
            line,
            origin,
            line_height,
            ..
        } = &*prepaint;
        if let Err(error) = line.paint(
            *origin,
            *line_height,
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        ) {
            tracing::warn!(%error, "cannot paint a field's text");
        }
        if let Some(hint) = prepaint.hint.take() {
            let at = point(prepaint.origin.x + prepaint.line.width, prepaint.origin.y);
            if let Err(error) = hint.paint(
                at,
                prepaint.line_height,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            ) {
                tracing::warn!(%error, "cannot paint a field's hint");
            }
        }
        if let Some(cursor) = prepaint.cursor.take() {
            window.paint_quad(cursor);
        }
        let line = prepaint.line.clone();
        self.input.update(cx, |input, _| {
            input.last_line = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

/// The placeholder, moved up and shrunk by the shift animation and colored
/// by focus and error.
fn placeholder_line(
    input: &InputField,
    bounds: Bounds<Pixels>,
    palette: &style::Palette,
    window: &mut Window,
) -> (ShapedLine, Point<Pixels>, Pixels) {
    let st = input.style;
    let shift = input
        .placeholder_animation
        .value_in(window, if input.placeholder_shifted { 1.0 } else { 0.0 });
    let focused = input
        .focused_animation
        .value_in(window, if input.focused { 1.0 } else { 0.0 });
    let error = input
        .error_animation
        .value_in(window, if input.error { 1.0 } else { 0.0 });
    let fg = mix(
        mix(
            st.placeholder_fg.resolve(palette),
            st.placeholder_fg_active.resolve(palette),
            focused,
        ),
        st.placeholder_fg_error.resolve(palette),
        error,
    );
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the scale is between 0 and 1"
    )]
    let scale = 1.0 - (1.0 - st.placeholder_scale as f32) * shift;
    let size = px_of(st.placeholder_font.size) * scale;
    let run = plain_run(
        input.placeholder.len(),
        &font(st.placeholder_font),
        hsla(fg),
    );
    let shaped = window
        .text_system()
        .shape_line(input.placeholder.clone(), size, &[run], None);
    #[expect(clippy::cast_precision_loss, reason = "style sizes are small")]
    let origin = point(
        bounds.left()
            + px_of(st.text_margins.left)
            + px_of(st.placeholder_margins.left)
            + px(interpolate(0.0, st.placeholder_shift_left.0 as f32, shift)),
        bounds.top()
            + px_of(st.text_margins.top)
            + px_of(st.placeholder_margins.top)
            + px(interpolate(0.0, st.placeholder_shift.0 as f32, shift)),
    );
    (shaped, origin, font_height(st.placeholder_font) * scale)
}

/// The resting bottom border, and the active one growing from the click.
/// `paintRoundSurrounding`: a rounded field whose fill and outline turn to
/// their active colors while focused.
// shortcut: switches at focus without tdesktop's fade; animate when the search field gets its cancel button.
fn round_surrounding(
    input: &InputField,
    bounds: Bounds<Pixels>,
    palette: &style::Palette,
    window: &Window,
) -> gpui::PaintQuad {
    let st = input.style;
    let focused = if input.focus.is_focused(window) {
        1.0
    } else {
        0.0
    };
    let divide = if st.border_denominator > 0 {
        st.border_denominator
    } else {
        1
    };
    #[expect(clippy::cast_precision_loss, reason = "border widths are small")]
    let border = px(st.border.0 as f32 / divide as f32);
    gpui::quad(
        bounds,
        px_of(st.border_radius),
        hsla(mix(
            st.text_bg.resolve(palette),
            st.text_bg_active.resolve(palette),
            focused,
        )),
        border,
        hsla(mix(
            st.border_fg.resolve(palette),
            st.border_fg_active.resolve(palette),
            focused,
        )),
        gpui::BorderStyle::Solid,
    )
}

fn border_quads(
    input: &InputField,
    bounds: Bounds<Pixels>,
    palette: &style::Palette,
    window: &Window,
) -> Vec<gpui::PaintQuad> {
    let st = input.style;
    let mut quads = Vec::with_capacity(2);
    if st.border.0 > 0 {
        let height = px_of(st.border);
        quads.push(fill(
            Bounds::new(
                point(bounds.left(), bounds.bottom() - height),
                size(bounds.size.width, height),
            ),
            hsla(st.border_fg.resolve(palette)),
        ));
    }
    let shown = input.border_shown.value_in(window, 1.0);
    let opacity = input
        .border_opacity
        .value_in(window, if input.border_visible { 1.0 } else { 0.0 });
    if st.border_active.0 == 0 || opacity <= 0.0 {
        return quads;
    }
    let width = f32::from(bounds.size.width);
    let start = input
        .border_start
        .map_or(width / 2.0, f32::from)
        .clamp(0.0, width);
    let from = (start * (1.0 - shown)).round();
    let to = start + ((width - start) * shown).round();
    if to <= from {
        return quads;
    }
    let error = input
        .error_animation
        .value_in(window, if input.error { 1.0 } else { 0.0 });
    let mut color = hsla(mix(
        st.border_fg_active.resolve(palette),
        st.border_fg_error.resolve(palette),
        error,
    ));
    color.a *= opacity;
    let height = px_of(st.border_active);
    quads.push(fill(
        Bounds::new(
            point(bounds.left() + px(from), bounds.bottom() - height),
            size(px(to - from), height),
        ),
        color,
    ));
    quads
}

pub(crate) fn plain_run(len: usize, font: &gpui::Font, color: gpui::Hsla) -> TextRun {
    TextRun {
        len,
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

/// Runs for the field's text: selected text in the selection color, marked
/// (composing) text underlined.
pub(crate) fn text_runs(
    text: &str,
    font: &gpui::Font,
    color: gpui::Hsla,
    selected_color: gpui::Hsla,
    selected: &Range<usize>,
    marked: Option<&Range<usize>>,
) -> Vec<TextRun> {
    let mut cuts = vec![0, text.len(), selected.start, selected.end];
    if let Some(marked) = marked {
        cuts.extend([marked.start, marked.end]);
    }
    cuts.retain(|cut| *cut <= text.len());
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .filter(|pair| pair[1] > pair[0])
        .map(|pair| {
            let range = pair[0]..pair[1];
            let is_selected = selected.start <= range.start && range.end <= selected.end;
            let is_marked =
                marked.is_some_and(|marked| marked.start <= range.start && range.end <= marked.end);
            let run_color = if is_selected { selected_color } else { color };
            TextRun {
                underline: is_marked.then(|| UnderlineStyle {
                    color: Some(run_color),
                    thickness: px(1.0),
                    wavy: false,
                }),
                ..plain_run(range.len(), font, run_color)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_split_at_selection_and_composition() {
        let font = gpui::font("Open Sans");
        let runs = text_runs(
            "hello world",
            &font,
            gpui::black(),
            gpui::white(),
            &(2..5),
            Some(&(4..8)),
        );
        let lens: Vec<usize> = runs.iter().map(|run| run.len).collect();
        assert_eq!(lens, vec![2, 2, 1, 3, 3]);
        assert_eq!(runs[1].color, gpui::white());
        assert!(runs[2].underline.is_some() && runs[3].underline.is_some());
    }

    #[test]
    fn floors_offsets_to_char_boundaries() {
        assert_eq!(floor_char_boundary("aé", 2), 1);
        assert_eq!(floor_char_boundary("aé", 9), 3);
    }
}
