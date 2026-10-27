//! The field's key bindings and the actions they run.

use gpui::{App, ClipboardItem, Context, EntityInputHandler, KeyBinding, Window};

use super::{
    Backspace, CONTEXT, Copy, Cut, Delete, DeleteWordLeft, End, FocusNext, FocusPrevious, Home,
    InputEvent, InputField, InputMode, Left, Paste, Right, SelectAll, SelectLeft, SelectRight,
    SelectToEnd, SelectToHome, SelectWordLeft, SelectWordRight, Submit, WordLeft, WordRight,
};

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

impl InputField {
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

    pub(super) fn select_all_action(
        &mut self,
        _: &SelectAll,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_all(cx);
    }

    pub(super) fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, window, cx);
    }

    pub(super) fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.buffer.text.len(), window, cx);
    }

    pub(super) fn select_to_home(
        &mut self,
        _: &SelectToHome,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(0, cx);
    }

    pub(super) fn select_to_end(
        &mut self,
        _: &SelectToEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.buffer.text.len(), cx);
    }

    pub(super) fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.selected.is_empty() {
            if self.buffer.cursor() == 0 {
                cx.emit(InputEvent::BackspaceAtStart);
                return;
            }
            self.select_to(self.buffer.previous_boundary(self.buffer.cursor()), cx);
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
            self.select_to(self.buffer.previous_word(self.buffer.cursor()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    pub(super) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.selected.is_empty() {
            self.select_to(self.buffer.next_boundary(self.buffer.cursor()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    pub(super) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace(['\n', '\r'], " "), window, cx);
        }
    }

    pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.buffer.selected.is_empty() && self.mode == InputMode::Text {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.buffer.selected_text().to_owned(),
            ));
        }
    }

    pub(super) fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.buffer.selected.is_empty() && self.mode == InputMode::Text {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.buffer.selected_text().to_owned(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    pub(super) fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(InputEvent::Submitted);
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
