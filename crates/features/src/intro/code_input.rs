//! `Ui::CodeInput`: a box per code digit, filled as the user types; it
//! submits itself once every box is filled.

use gpui::{
    App, ClipboardItem, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyBinding,
    KeyDownEvent, MouseButton, ParentElement, Pixels, Render, Styled, Window, actions, div,
    prelude::*, px,
};
use style::basic::{SHAKE_DURATION, SHAKE_SHIFT, UNIVERSAL_DURATION};
use style::intro::{
    INTRO_CODE_DIGIT_BORDER_WIDTH, INTRO_CODE_DIGIT_FONT, INTRO_CODE_DIGIT_HEIGHT,
    INTRO_CODE_DIGIT_SKIP,
};
use style::layers::BOX_RADIUS;
use style::{ColorId, ColorRef};
use ui::anim::{Easing, Simple, shake};
use ui::{color, font, font_height};

use crate::px_of;

actions!(
    code_input,
    [
        Next, Previous, Erase, EraseBack, Collect, First, Last, PasteCode
    ]
);

const CONTEXT: &str = "CodeInput";
/// `kMaxDigitsCount`.
const MAX_DIGITS: usize = 32;
/// A digit box's width to its height (`kWidthRatio`).
const WIDTH_RATIO: f32 = 0.8;
/// How far a digit slides in, to the box height (`kSlideDistanceRatio`).
const SLIDE_RATIO: f32 = 0.2;

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("right", Next, context),
        KeyBinding::new("down", Next, context),
        KeyBinding::new("space", Next, context),
        KeyBinding::new("left", Previous, context),
        KeyBinding::new("up", Previous, context),
        KeyBinding::new("delete", Erase, context),
        KeyBinding::new("backspace", EraseBack, context),
        KeyBinding::new("enter", Collect, context),
        KeyBinding::new("home", First, context),
        KeyBinding::new("pageup", First, context),
        KeyBinding::new("end", Last, context),
        KeyBinding::new("pagedown", Last, context),
        KeyBinding::new(ui::secondary!("v"), PasteCode, context),
    ]);
}

/// Every box is filled; carries the code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Collected(pub String);

#[derive(Debug, Default, Clone, Copy)]
struct Digit {
    /// The digit the box holds.
    value: Option<u8>,
    /// The digit drawn, kept while it fades out.
    shown: Option<u8>,
    appear: Simple,
    shake: Simple,
}

impl Digit {
    /// `CodeDigit::setDigit`.
    fn set(&mut self, value: Option<u8>) {
        if self.value == value && self.appear.animating() {
            return;
        }
        self.value = value;
        if self.shown != value {
            if value.is_none() {
                self.appear
                    .start(1.0, 0.0, UNIVERSAL_DURATION, Easing::Linear);
            } else {
                self.shown = value;
                self.appear
                    .start(0.0, 1.0, UNIVERSAL_DURATION, Easing::Linear);
            }
        }
    }

    fn shake(&mut self) {
        if !self.shake.animating() {
            self.shake.start(0.0, 1.0, SHAKE_DURATION, Easing::Linear);
        }
    }
}

pub(crate) struct CodeInput {
    focus: FocusHandle,
    digits: Vec<Digit>,
    current: usize,
    is_error: bool,
}

/// Leaves the digits out: they are a login code.
impl std::fmt::Debug for CodeInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeInput")
            .field("length", &self.digits.len())
            .finish_non_exhaustive()
    }
}

impl EventEmitter<Collected> for CodeInput {}

impl Focusable for CodeInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl CodeInput {
    pub fn new(length: usize, cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            digits: vec![Digit::default(); length.clamp(1, MAX_DIGITS)],
            current: 0,
            is_error: false,
        }
    }

    pub fn width(&self) -> Pixels {
        let count = self.digits.len();
        #[expect(clippy::cast_precision_loss, reason = "at most 32 digits")]
        let count_f = count as f32;
        let skip = px_of(INTRO_CODE_DIGIT_SKIP);
        skip * 2.0 + digit_width() * count_f + skip * (count_f - 1.0)
    }

    fn collect(&self) -> String {
        self.digits
            .iter()
            .filter_map(|digit| digit.value)
            .map(|digit| char::from(b'0' + digit))
            .collect()
    }

    fn first_empty(&self) -> Option<usize> {
        self.digits.iter().position(|digit| digit.value.is_none())
    }

    /// `requestCode`: submits a full code, shakes the first empty box.
    pub fn request(&mut self, cx: &mut Context<Self>) {
        let code = self.collect();
        if code.len() == self.digits.len() {
            cx.emit(Collected(code));
        } else if let Some(index) = self.first_empty() {
            self.digits[index].shake();
        }
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        for digit in &mut self.digits {
            digit.set(None);
        }
        self.current = 0;
        self.is_error = false;
        cx.notify();
    }

    /// `showError`: empties and shakes every box, with red borders.
    pub fn show_error(&mut self, cx: &mut Context<Self>) {
        self.clear(cx);
        for digit in &mut self.digits {
            digit.shake();
        }
        self.is_error = true;
        cx.notify();
    }

    fn process_digit(&mut self, value: u8, cx: &mut Context<Self>) {
        self.is_error = false;
        self.digits[self.current].set(Some(value));
        self.current = (self.current + 1) % self.digits.len();
        if self.current == 0 {
            let code = self.collect();
            if code.len() == self.digits.len() {
                self.current = self.digits.len() - 1;
                cx.emit(Collected(code));
            } else if let Some(index) = self.first_empty() {
                self.current = index;
            }
        }
        cx.notify();
    }

    /// `insertCodeAndSubmit`: fills the boxes from pasted text.
    pub fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        let digits: Vec<u8> = text
            .bytes()
            .filter(u8::is_ascii_digit)
            .map(|byte| byte - b'0')
            .take(self.digits.len())
            .collect();
        if digits.is_empty() {
            return;
        }
        for (digit, value) in self.digits.iter_mut().zip(digits) {
            digit.set(Some(value));
        }
        self.current = self.first_empty().unwrap_or(self.digits.len() - 1);
        if self.first_empty().is_none() {
            self.request(cx);
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.control || keystroke.modifiers.platform || keystroke.modifiers.alt {
            return;
        }
        let Some(ch) = keystroke
            .key
            .chars()
            .next()
            .filter(|_| keystroke.key.chars().count() == 1)
        else {
            return;
        };
        if let Some(value) = ch.to_digit(10) {
            #[expect(clippy::cast_possible_truncation, reason = "a decimal digit")]
            self.process_digit(value as u8, cx);
            cx.stop_propagation();
        } else if ch.is_ascii_alphabetic() {
            self.digits[self.current].shake();
            cx.notify();
        }
    }

    fn move_to(&mut self, index: usize, cx: &mut Context<Self>) {
        self.current = index;
        cx.notify();
    }
}

fn digit_width() -> Pixels {
    (px_of(INTRO_CODE_DIGIT_HEIGHT) * WIDTH_RATIO).floor()
}

impl Render for CodeInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_focused = self.focus.is_focused(window);
        let height = px_of(INTRO_CODE_DIGIT_HEIGHT);
        let width = digit_width();
        let skip = px_of(INTRO_CODE_DIGIT_SKIP);
        let border = px_of(INTRO_CODE_DIGIT_BORDER_WIDTH);
        let count = self.digits.len();
        let mut boxes = Vec::with_capacity(count);
        for index in 0..count {
            let digit = &mut self.digits[index];
            let shift = if digit.shake.animating() {
                shake(digit.shake.value_in(window, 1.0)) * f32::from(px_of(SHAKE_SHIFT))
            } else {
                0.0
            };
            let progress = digit.appear.value_in(window, 1.0);
            if digit.value.is_none() && !digit.appear.animating() {
                digit.shown = None;
            }
            let border_color = if self.is_error {
                ColorId::ActiveLineFgError
            } else if is_focused && index == self.current {
                ColorId::WindowActiveTextFg
            } else {
                ColorId::WindowBgRipple
            };
            let text = digit.shown.map(|shown| {
                let hiding = digit.value.is_none();
                let (opacity, dy, scale) = if hiding {
                    (progress * progress, 0.0, progress)
                } else {
                    (
                        progress,
                        f32::from(height) * SLIDE_RATIO * (1.0 - progress),
                        1.0,
                    )
                };
                let size = px_of(INTRO_CODE_DIGIT_FONT.size) * scale;
                div()
                    .absolute()
                    .inset_0()
                    .top(px(dy))
                    .flex()
                    .items_center()
                    .justify_center()
                    .opacity(opacity)
                    .font(font(INTRO_CODE_DIGIT_FONT))
                    .text_size(size)
                    .line_height(font_height(INTRO_CODE_DIGIT_FONT))
                    .text_color(color(cx, ColorRef::Palette(ColorId::WindowFg)))
                    .child(char::from(b'0' + shown).to_string())
            });
            let entity = cx.entity();
            boxes.push(
                div()
                    .id(("code-digit", index))
                    .absolute()
                    .left(skip + (width + skip) * index as f32 + px(shift))
                    .top_0()
                    .w(width)
                    .h(height)
                    .rounded(px_of(BOX_RADIUS))
                    .bg(color(cx, ColorRef::Palette(ColorId::WindowBgOver)))
                    .border(border)
                    .border_color(color(cx, ColorRef::Palette(border_color)))
                    .overflow_hidden()
                    .children(text)
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |this, cx| this.move_to(index, cx))
                    }),
            );
        }
        div()
            .id("code-input")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .relative()
            .w(self.width())
            .h(height)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus.focus(window, cx);
                }),
            )
            .on_key_down(cx.listener(|this, event, _, cx| this.key_down(event, cx)))
            .on_action(cx.listener(|this, _: &Next, _, cx| {
                let next = (this.current + 1) % this.digits.len();
                this.move_to(next, cx);
            }))
            .on_action(cx.listener(|this, _: &Previous, _, cx| {
                let count = this.digits.len();
                this.move_to((this.current + count - 1) % count, cx);
            }))
            .on_action(cx.listener(|this, _: &Erase, _, cx| {
                this.digits[this.current].set(None);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &EraseBack, _, cx| {
                let had_digit = this.digits[this.current].value.is_some();
                this.digits[this.current].set(None);
                this.current = this.current.saturating_sub(1);
                if !had_digit {
                    this.digits[this.current].set(None);
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Collect, _, cx| this.request(cx)))
            .on_action(cx.listener(|this, _: &First, _, cx| this.move_to(0, cx)))
            .on_action(cx.listener(|this, _: &Last, _, cx| {
                let last = this.digits.len() - 1;
                this.move_to(last, cx);
            }))
            .on_action(cx.listener(|this, _: &PasteCode, _, cx| {
                if let Some(text) = cx
                    .read_from_clipboard()
                    .and_then(|item: ClipboardItem| item.text())
                {
                    this.insert(&text, cx);
                }
            }))
            .children(boxes)
    }
}
