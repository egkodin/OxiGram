//! lib_ui's multi-line `InputField`: text that wraps, grows the field up to
//! a maximum height and then scrolls, a placeholder that slides away as
//! typing starts, and Enter or Ctrl+Enter to submit. IME composition goes
//! through gpui's input handler.

mod element;
mod keys;
mod layout;

pub(crate) use self::keys::bind_keys;

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, Context, CursorStyle, ElementId, EntityInputHandler, EventEmitter, FocusHandle,
    Focusable, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    ScrollEvent, SharedString, Task, UTF16Selection, Window, div, point, prelude::*, px,
};
use style::widgets::InputField as InputStyle;

use self::element::AreaElement;
use self::layout::{Layout, Metrics};
use crate::anim::{Easing, Simple};
use crate::font::font_height;
use crate::input::{BLINK_INTERVAL, px_of};
use crate::text_buffer::TextBuffer;

/// Which Enter submits (`Ui::InputSubmitSettings`); the other one, and
/// Shift+Enter, start a new line. Ctrl+Shift+Enter always submits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SubmitKey {
    #[default]
    Enter,
    CtrlEnter,
}

/// The Enter presses that differ (`InputField::ShouldSubmit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnterPress {
    Plain,
    /// With Ctrl, or Cmd on macOS.
    Control,
}

fn submits(key: SubmitKey, press: EnterPress) -> bool {
    matches!(
        (key, press),
        (SubmitKey::Enter, EnterPress::Plain) | (SubmitKey::CtrlEnter, EnterPress::Control)
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAreaEvent {
    Changed,
    Submitted,
    Focused,
    Blurred,
}

/// What the last paint laid out, for the mouse and the IME.
#[derive(Debug)]
struct Painted {
    layout: Rc<Layout>,
    bounds: Bounds<Pixels>,
}

pub struct TextArea {
    focus: FocusHandle,
    style: &'static InputStyle,
    metrics: Metrics,
    placeholder: SharedString,
    buffer: TextBuffer,
    submit: SubmitKey,
    focused: bool,
    is_selecting: bool,
    /// The x a run of Up and Down presses keeps to.
    goal_x: Option<Pixels>,
    scroll_top: Pixels,
    /// Set by edits and moves, so the next paint scrolls to the cursor.
    reveal_cursor: bool,
    placeholder_hidden: bool,
    placeholder_animation: Simple,
    focused_animation: Simple,
    cursor_visible: bool,
    blink: Option<Task<()>>,
    painted: Option<Painted>,
    _subscriptions: [gpui::Subscription; 2],
}

/// Leaves the text out: it is the user's message.
impl std::fmt::Debug for TextArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextArea")
            .field("focused", &self.focused)
            .field("submit", &self.submit)
            .finish_non_exhaustive()
    }
}

impl EventEmitter<TextAreaEvent> for TextArea {}

impl Focusable for TextArea {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextArea {
    /// A field `style.height_min` to `style.height_max` tall with no margin
    /// around the text; `with_geometry` overrides them.
    pub fn new(
        style: &'static InputStyle,
        placeholder: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let subscriptions = [
            cx.on_focus(&focus, window, |this, window, cx| {
                this.set_focused(true, window, cx);
            }),
            cx.on_blur(&focus, window, |this, window, cx| {
                this.set_focused(false, window, cx);
            }),
        ];
        Self {
            focus,
            style,
            metrics: Metrics {
                font_size: px_of(style.style.font.size),
                line_height: font_height(style.style.font),
                margin: px(0.0),
                min_height: px_of(style.height_min),
                max_height: px_of(style.height_max),
            },
            placeholder: placeholder.into(),
            buffer: TextBuffer::default(),
            submit: SubmitKey::default(),
            focused: false,
            is_selecting: false,
            goal_x: None,
            scroll_top: px(0.0),
            reveal_cursor: false,
            placeholder_hidden: false,
            placeholder_animation: Simple::default(),
            focused_animation: Simple::default(),
            cursor_visible: true,
            blink: None,
            painted: None,
            _subscriptions: subscriptions,
        }
    }

    /// The height range and the margin around the text
    /// (`InitMessageFieldGeometry`).
    #[must_use]
    pub fn with_geometry(mut self, min_height: Pixels, max_height: Pixels, margin: Pixels) -> Self {
        self.metrics.min_height = min_height;
        self.metrics.max_height = max_height;
        self.metrics.margin = margin;
        self
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.buffer.text
    }

    pub fn set_submit_key(&mut self, key: SubmitKey) {
        self.submit = key;
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    /// Replaces the text and puts the cursor at its end.
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.buffer.text = normalize_newlines(text);
        self.buffer.move_to(self.buffer.text.len());
        self.buffer.marked = None;
        self.after_edit(cx);
    }

    /// Types `text` over the selection, as pasting would.
    pub fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        let range = self.buffer.edited_range();
        self.buffer.replace(range, &normalize_newlines(text));
        self.after_edit(cx);
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
        if focused {
            self.restart_blink(window, cx);
            cx.emit(TextAreaEvent::Focused);
        } else {
            self.blink = None;
            cx.emit(TextAreaEvent::Blurred);
        }
        cx.notify();
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

    fn after_edit(&mut self, cx: &mut Context<Self>) {
        let hidden = !self.buffer.text.is_empty() || self.buffer.marked.is_some();
        if self.placeholder_hidden != hidden {
            self.placeholder_hidden = hidden;
            let (from, to) = if hidden { (0.0, 1.0) } else { (1.0, 0.0) };
            self.placeholder_animation
                .start(from, to, self.style.duration, Easing::Linear);
        }
        self.goal_x = None;
        self.reveal_cursor = true;
        self.cursor_visible = true;
        cx.emit(TextAreaEvent::Changed);
        cx.notify();
    }

    fn move_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.buffer.move_to(offset);
        self.goal_x = None;
        self.reveal_cursor = true;
        self.restart_blink(window, cx);
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.buffer.select_to(offset);
        self.goal_x = None;
        self.reveal_cursor = true;
        cx.notify();
    }

    fn layout(&self) -> Option<&Layout> {
        self.painted.as_ref().map(|painted| &*painted.layout)
    }

    /// The offset a row above (`rows` < 0) or below the cursor, at the x
    /// the vertical moves keep; the text's ends past the first and last row.
    fn vertical_target(&mut self, rows: isize) -> usize {
        let cursor = self.buffer.cursor();
        let Some(layout) = self.layout() else {
            return cursor;
        };
        let position = layout.position(cursor);
        let goal_x = self.goal_x.unwrap_or(position.x);
        let row = layout.row_of(cursor).checked_add_signed(rows);
        let target = match row {
            None => 0,
            Some(row) if row >= layout.rows() => self.buffer.text.len(),
            Some(row) => {
                #[expect(clippy::cast_precision_loss, reason = "row counts are small")]
                let y = layout.line_height * row as f32;
                layout.index_at(point(goal_x, y))
            }
        };
        self.goal_x = Some(goal_x);
        target
    }

    fn vertical_move(&mut self, rows: isize, window: &mut Window, cx: &mut Context<Self>) {
        let target = if self.buffer.selected.is_empty() {
            self.vertical_target(rows)
        } else if rows < 0 {
            self.buffer.selected.start
        } else {
            self.buffer.selected.end
        };
        let goal_x = self.goal_x;
        self.move_to(target, window, cx);
        self.goal_x = goal_x;
    }

    fn vertical_select(&mut self, rows: isize, cx: &mut Context<Self>) {
        let target = self.vertical_target(rows);
        let goal_x = self.goal_x;
        self.select_to(target, cx);
        self.goal_x = goal_x;
    }

    /// The visual row around the cursor.
    fn cursor_row(&self) -> Range<usize> {
        let cursor = self.buffer.cursor();
        self.layout().map_or_else(
            || self.buffer.paragraph_at(cursor),
            |layout| layout.row_range(layout.row_of(cursor)),
        )
    }

    fn index_for_position(&self, position: Point<Pixels>) -> usize {
        let Some(painted) = &self.painted else {
            return 0;
        };
        let origin = self.text_origin(painted.bounds);
        painted.layout.index_at(position - origin)
    }

    /// Where the text's first row starts, scrolled.
    fn text_origin(&self, bounds: Bounds<Pixels>) -> Point<Pixels> {
        let margin = self.metrics.margin;
        point(
            bounds.left() + margin,
            bounds.top() + margin - self.scroll_top,
        )
    }

    fn submit_or_newline(
        &mut self,
        press: EnterPress,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if submits(self.submit, press) {
            cx.emit(TextAreaEvent::Submitted);
        } else {
            self.replace_text_in_range(None, "\n", window, cx);
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        let index = self.index_for_position(event.position);
        if event.modifiers.shift {
            self.select_to(index, cx);
        } else if event.click_count == 2 {
            self.buffer.selected = self.buffer.word_at(index);
            self.buffer.reversed = false;
            cx.notify();
        } else if event.click_count >= 3 {
            self.buffer.selected = self.buffer.paragraph_at(index);
            self.buffer.reversed = false;
            cx.notify();
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

    /// Scrolls the text once it outgrows the field; otherwise the wheel
    /// goes on to what is under the field.
    fn on_scroll(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(painted) = &self.painted else {
            return;
        };
        let max = self
            .metrics
            .max_scroll(painted.layout.rows(), painted.bounds.size.height);
        if max <= px(0.0) {
            return;
        }
        let delta = event.delta.pixel_delta(self.metrics.line_height).y;
        let scroll_top = (self.scroll_top - delta).clamp(px(0.0), max);
        if scroll_top != self.scroll_top {
            self.scroll_top = scroll_top;
            cx.notify();
        }
        cx.stop_propagation();
    }
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

impl EntityInputHandler for TextArea {
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
            .map(|range| self.buffer.range_from_utf16(&range))
            .unwrap_or_else(|| self.buffer.edited_range());
        self.buffer.replace(range, new_text);
        self.restart_blink(window, cx);
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
            .map(|range| self.buffer.range_from_utf16(&range))
            .unwrap_or_else(|| self.buffer.edited_range());
        self.buffer.replace(range.clone(), new_text);
        self.buffer.marked =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        if let Some(selected) = new_selected {
            let start = self.buffer.offset_to_utf16(range.start);
            let inserted = new_text.encode_utf16().count();
            self.buffer.selected = self.buffer.range_from_utf16(
                &(start + selected.start.min(inserted)..start + selected.end.min(inserted)),
            );
        }
        self.after_edit(cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let painted = self.painted.as_ref()?;
        let range = self.buffer.range_from_utf16(&range);
        let origin = self.text_origin(painted.bounds);
        let start = painted.layout.position(range.start);
        let end = painted.layout.position(range.end);
        Some(Bounds::from_corners(
            origin + start,
            origin + point(end.x, end.y + self.metrics.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.buffer.offset_to_utf16(self.index_for_position(point)))
    }
}

impl Render for TextArea {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(ElementId::Name("text-area".into()))
            .key_context(keys::CONTEXT)
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .w_full()
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_to_home))
            .on_action(cx.listener(Self::select_to_end))
            .on_action(cx.listener(Self::document_start))
            .on_action(cx.listener(Self::document_end))
            .on_action(cx.listener(Self::select_to_document_start))
            .on_action(cx.listener(Self::select_to_document_end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::secondary_enter))
            .on_action(cx.listener(Self::force_submit))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::focus_next))
            .on_action(cx.listener(Self::focus_previous))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll(cx.listener(Self::on_scroll))
            .child(AreaElement { area: cx.entity() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chosen_enter_submits_and_the_other_breaks_the_line() {
        assert!(submits(SubmitKey::Enter, EnterPress::Plain));
        assert!(!submits(SubmitKey::Enter, EnterPress::Control));
        assert!(submits(SubmitKey::CtrlEnter, EnterPress::Control));
        assert!(!submits(SubmitKey::CtrlEnter, EnterPress::Plain));
    }

    #[test]
    fn pasted_line_breaks_become_newlines() {
        assert_eq!(normalize_newlines("a\r\nb\rc\nd"), "a\nb\nc\nd");
    }
}
