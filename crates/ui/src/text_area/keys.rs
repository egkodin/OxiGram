//! The field's key bindings and the handler behind each action.

use gpui::{App, ClipboardItem, Context, EntityInputHandler as _, KeyBinding, Window, actions};

use super::{EnterPress, TextArea, TextAreaEvent, normalize_newlines};
use crate::input::{
    Backspace, Copy, Cut, Delete, DeleteWordLeft, End, FocusNext, FocusPrevious, Home, Left, Paste,
    Right, SelectAll, SelectLeft, SelectRight, SelectToEnd, SelectToHome, SelectWordLeft,
    SelectWordRight, Submit, WordLeft, WordRight,
};

actions!(
    text_area,
    [
        Up,
        Down,
        SelectUp,
        SelectDown,
        DocumentStart,
        DocumentEnd,
        SelectToDocumentStart,
        SelectToDocumentEnd,
        Newline,
        SecondaryEnter,
        ForceSubmit,
    ]
);

pub(super) const CONTEXT: &str = "TextArea";

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);
    let mut bindings = vec![
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("shift-backspace", Backspace, context),
        KeyBinding::new(word!("backspace"), DeleteWordLeft, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("up", Up, context),
        KeyBinding::new("down", Down, context),
        KeyBinding::new(word!("left"), WordLeft, context),
        KeyBinding::new(word!("right"), WordRight, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("shift-up", SelectUp, context),
        KeyBinding::new("shift-down", SelectDown, context),
        KeyBinding::new(word!("shift-left"), SelectWordLeft, context),
        KeyBinding::new(word!("shift-right"), SelectWordRight, context),
        KeyBinding::new(crate::secondary!("a"), SelectAll, context),
        KeyBinding::new("home", Home, context),
        KeyBinding::new("end", End, context),
        KeyBinding::new("shift-home", SelectToHome, context),
        KeyBinding::new("shift-end", SelectToEnd, context),
        KeyBinding::new(crate::secondary!("home"), DocumentStart, context),
        KeyBinding::new(crate::secondary!("end"), DocumentEnd, context),
        KeyBinding::new(crate::secondary!("v"), Paste, context),
        KeyBinding::new(crate::secondary!("x"), Cut, context),
        KeyBinding::new(crate::secondary!("c"), Copy, context),
        KeyBinding::new("enter", Submit, context),
        KeyBinding::new("shift-enter", Newline, context),
        KeyBinding::new(crate::secondary!("enter"), SecondaryEnter, context),
        KeyBinding::new(crate::secondary!("shift-enter"), ForceSubmit, context),
        KeyBinding::new("tab", FocusNext, context),
        KeyBinding::new("shift-tab", FocusPrevious, context),
    ];
    if cfg!(target_os = "macos") {
        // Qt counts both Cmd and Ctrl as control for Enter.
        bindings.extend([
            KeyBinding::new("ctrl-enter", SecondaryEnter, context),
            KeyBinding::new("ctrl-shift-enter", ForceSubmit, context),
            KeyBinding::new("cmd-left", Home, context),
            KeyBinding::new("cmd-right", End, context),
            KeyBinding::new("cmd-shift-left", SelectToHome, context),
            KeyBinding::new("cmd-shift-right", SelectToEnd, context),
            KeyBinding::new("cmd-up", DocumentStart, context),
            KeyBinding::new("cmd-down", DocumentEnd, context),
            KeyBinding::new("cmd-shift-up", SelectToDocumentStart, context),
            KeyBinding::new("cmd-shift-down", SelectToDocumentEnd, context),
        ]);
    }
    cx.bind_keys(bindings);
}

/// The key handlers, one per action.
impl TextArea {
    pub(super) fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        let target = if self.buffer.selected.is_empty() {
            self.buffer.previous_boundary(self.buffer.cursor())
        } else {
            self.buffer.selected.start
        };
        self.move_to(target, window, cx);
    }

    pub(super) fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        let target = if self.buffer.selected.is_empty() {
            self.buffer.next_boundary(self.buffer.cursor())
        } else {
            self.buffer.selected.end
        };
        self.move_to(target, window, cx);
    }

    pub(super) fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        // An empty field leaves Up to its owner, which edits the last message.
        if self.buffer.text.is_empty() {
            cx.propagate();
            return;
        }
        self.vertical_move(-1, window, cx);
    }

    pub(super) fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        self.vertical_move(1, window, cx);
    }

    pub(super) fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical_select(-1, cx);
    }

    pub(super) fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical_select(1, cx);
    }

    pub(super) fn word_left(&mut self, _: &WordLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.buffer.previous_word(self.buffer.cursor()), window, cx);
    }

    pub(super) fn word_right(
        &mut self,
        _: &WordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(self.buffer.next_word(self.buffer.cursor()), window, cx);
    }

    pub(super) fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.buffer.previous_boundary(self.buffer.cursor()), cx);
    }

    pub(super) fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.buffer.next_boundary(self.buffer.cursor()), cx);
    }

    pub(super) fn select_word_left(
        &mut self,
        _: &SelectWordLeft,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.buffer.previous_word(self.buffer.cursor()), cx);
    }

    pub(super) fn select_word_right(
        &mut self,
        _: &SelectWordRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.buffer.next_word(self.buffer.cursor()), cx);
    }

    pub(super) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.select_all();
        cx.notify();
    }

    pub(super) fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.cursor_row().start, window, cx);
    }

    pub(super) fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.cursor_row().end, window, cx);
    }

    pub(super) fn select_to_home(
        &mut self,
        _: &SelectToHome,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.cursor_row().start, cx);
    }

    pub(super) fn select_to_end(
        &mut self,
        _: &SelectToEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.cursor_row().end, cx);
    }

    pub(super) fn document_start(
        &mut self,
        _: &DocumentStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Cmd+Up in an empty field replies to a message (`eventFilter`).
        if self.buffer.text.is_empty() {
            cx.propagate();
            return;
        }
        self.move_to(0, window, cx);
    }

    pub(super) fn document_end(
        &mut self,
        _: &DocumentEnd,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.buffer.text.is_empty() {
            cx.propagate();
            return;
        }
        self.move_to(self.buffer.text.len(), window, cx);
    }

    pub(super) fn select_to_document_start(
        &mut self,
        _: &SelectToDocumentStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(0, cx);
    }

    pub(super) fn select_to_document_end(
        &mut self,
        _: &SelectToDocumentEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.buffer.text.len(), cx);
    }

    pub(super) fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.selected.is_empty() {
            self.buffer
                .select_to(self.buffer.previous_boundary(self.buffer.cursor()));
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    pub(super) fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.buffer.selected.is_empty() {
            self.buffer
                .select_to(self.buffer.previous_word(self.buffer.cursor()));
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    pub(super) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.selected.is_empty() {
            self.buffer
                .select_to(self.buffer.next_boundary(self.buffer.cursor()));
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    pub(super) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        // shortcut: text only; images and files come with attachments (M4).
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &normalize_newlines(&text), window, cx);
        }
    }

    pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.buffer.selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.buffer.selected_text().to_owned(),
            ));
        }
    }

    pub(super) fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.buffer.selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.buffer.selected_text().to_owned(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    pub(super) fn enter(&mut self, _: &Submit, window: &mut Window, cx: &mut Context<Self>) {
        self.submit_or_newline(EnterPress::Plain, window, cx);
    }

    pub(super) fn secondary_enter(
        &mut self,
        _: &SecondaryEnter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.submit_or_newline(EnterPress::Control, window, cx);
    }

    pub(super) fn force_submit(&mut self, _: &ForceSubmit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextAreaEvent::Submitted);
    }

    pub(super) fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(None, "\n", window, cx);
    }

    pub(super) fn focus_next(
        &mut self,
        _: &FocusNext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus_next(cx);
    }

    pub(super) fn focus_previous(
        &mut self,
        _: &FocusPrevious,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus_prev(cx);
    }
}
