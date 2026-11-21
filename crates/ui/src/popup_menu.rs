//! lib_ui's `PopupMenu`: a rounded, shadowed list of actions shown at the
//! pointer, closed by a choice, a click outside or Escape.

use std::rc::Rc;

use gpui::{
    App, BoxShadow, Context, DismissEvent, EventEmitter, FocusHandle, Focusable, IntoElement,
    KeyBinding, ParentElement, Render, SharedString, Styled, Window, actions, div,
    hsla as gpui_hsla, point, prelude::*, px,
};
use style::widgets::PopupMenu as PopupStyle;
use style::{ColorId, ColorRef};

use crate::button::px_of;
use crate::font::{font, font_height};
use crate::icon::icon;
use crate::theme::color;

actions!(popup_menu, [CloseMenu]);

const CONTEXT: &str = "PopupMenu";

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CloseMenu, Some(CONTEXT))]);
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Whether an action destroys something, drawn in the attention color
/// (`isAttention`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuTone {
    Normal,
    Attention,
}

/// Menu icons take the menu's icon color; marks such as a check keep their
/// own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IconFill {
    Menu,
    Own,
}

enum Item {
    Action {
        text: SharedString,
        icon: Option<(style::Icon, IconFill)>,
        tone: MenuTone,
        handler: Handler,
    },
    Separator,
    /// An inert line, such as what a lookup found nothing for.
    Note(SharedString),
}

pub struct PopupMenu {
    style: &'static PopupStyle,
    items: Vec<Item>,
    hovered: Option<usize>,
    dismiss_outside: bool,
    focus: FocusHandle,
}

impl std::fmt::Debug for PopupMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PopupMenu")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

impl EventEmitter<DismissEvent> for PopupMenu {}

impl Focusable for PopupMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl PopupMenu {
    pub fn new(style: &'static PopupStyle, cx: &mut Context<Self>) -> Self {
        Self {
            style,
            items: Vec::new(),
            hovered: None,
            dismiss_outside: true,
            focus: cx.focus_handle(),
        }
    }

    /// A parent that contains additional controls handles outside clicks.
    pub fn use_parent_dismissal(&mut self) {
        self.dismiss_outside = false;
    }

    /// Adds an action; the menu closes before `handler` runs.
    pub fn action(
        &mut self,
        text: impl Into<SharedString>,
        icon: style::Icon,
        tone: MenuTone,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) {
        self.items.push(Item::Action {
            text: text.into(),
            icon: Some((icon, IconFill::Menu)),
            tone,
            handler: Rc::new(handler),
        });
    }

    /// Adds an option of a choice, `mark` (a check) beside the chosen one.
    pub fn option(
        &mut self,
        text: impl Into<SharedString>,
        mark: Option<style::Icon>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) {
        self.items.push(Item::Action {
            text: text.into(),
            icon: mark.map(|mark| (mark, IconFill::Own)),
            tone: MenuTone::Normal,
            handler: Rc::new(handler),
        });
    }

    pub fn separator(&mut self) {
        self.items.push(Item::Separator);
    }

    /// Adds a line that does nothing, centered in the shortcut color
    /// (`ResolvePhoneAction`'s "not on Telegram").
    pub fn note(&mut self, text: impl Into<SharedString>) {
        self.items.push(Item::Note(text.into()));
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn choose(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Item::Action { handler, .. }) = self.items.get(index) else {
            return;
        };
        let handler = handler.clone();
        cx.emit(DismissEvent);
        handler(window, cx);
    }

    fn item(&self, index: usize, item: &Item, cx: &mut Context<Self>) -> gpui::AnyElement {
        let st = &self.style.menu;
        match item {
            Item::Separator => div()
                .pt(px_of(st.separator.padding.top))
                .pb(px_of(st.separator.padding.bottom))
                .child(
                    div()
                        .h(px_of(st.separator.width))
                        .bg(color(cx, st.separator.fg)),
                )
                .into_any_element(),
            Item::Note(text) => {
                let text_font = st.item_style.font;
                div()
                    .pl(px_of(st.item_padding.left))
                    .pr(px_of(st.item_padding.right))
                    .pt(px_of(st.item_padding.top))
                    .pb(px_of(st.item_padding.bottom))
                    .bg(color(cx, st.item_bg))
                    .text_center()
                    .font(font(text_font))
                    .text_size(px_of(text_font.size))
                    .line_height(font_height(text_font))
                    .text_color(color(cx, st.item_fg_shortcut))
                    .child(text.clone())
                    .into_any_element()
            }
            Item::Action {
                text,
                icon: shown,
                tone,
                ..
            } => {
                let is_hovered = self.hovered == Some(index);
                let text_font = st.item_style.font;
                let (text_fg, icon_fg) = match (tone, is_hovered) {
                    // Attention icons carry their own color (`menuIconDeleteAttention`).
                    (MenuTone::Attention, _) => {
                        (ColorRef::Palette(ColorId::AttentionButtonFg), None)
                    }
                    (MenuTone::Normal, true) => (
                        st.item_fg_over,
                        Some(ColorRef::Palette(ColorId::MenuIconFgOver)),
                    ),
                    (MenuTone::Normal, false) => {
                        (st.item_fg, Some(ColorRef::Palette(ColorId::MenuIconFg)))
                    }
                };
                div()
                    .id(("menu-item", index))
                    .relative()
                    .pl(px_of(st.item_padding.left))
                    .pr(px_of(st.item_padding.right))
                    .pt(px_of(st.item_padding.top))
                    .pb(px_of(st.item_padding.bottom))
                    .bg(color(
                        cx,
                        if is_hovered {
                            st.item_bg_over
                        } else {
                            st.item_bg
                        },
                    ))
                    .whitespace_nowrap()
                    .font(font(text_font))
                    .text_size(px_of(text_font.size))
                    .line_height(font_height(text_font))
                    .text_color(color(cx, text_fg))
                    .when_some(*shown, |this, (shown, fill)| {
                        this.child(
                            div()
                                .absolute()
                                .left(px_of(st.item_icon_position.x))
                                .top(px_of(st.item_icon_position.y))
                                .child(match icon_fg.filter(|_| fill == IconFill::Menu) {
                                    Some(fg) => icon(shown).color(fg),
                                    None => icon(shown),
                                }),
                        )
                    })
                    .child(text.clone())
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        let next = if *hovered {
                            Some(index)
                        } else if this.hovered == Some(index) {
                            None
                        } else {
                            this.hovered
                        };
                        if this.hovered != next {
                            this.hovered = next;
                            cx.notify();
                        }
                    }))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.choose(index, window, cx)),
                    )
                    .into_any_element()
            }
        }
    }
}

impl Render for PopupMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let st = self.style;
        let shadow = st.shadow;
        #[expect(clippy::cast_possible_truncation, reason = "an opacity in 0..1")]
        let shadow_color = gpui_hsla(0.0, 0.0, 0.0, shadow.opacity as f32);
        let items: Vec<gpui::AnyElement> = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| self.item(index, item, cx))
            .collect();
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &CloseMenu, _, cx| cx.emit(DismissEvent)))
            .when(self.dismiss_outside, |menu| {
                menu.on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(DismissEvent)))
            })
            // Clicks inside must not reach what lies under the menu.
            .occlude()
            .min_w(px_of(st.menu.width_min))
            .max_w(px_of(st.menu.width_max))
            .pt(px_of(st.scroll_padding.top))
            .pb(px_of(st.scroll_padding.bottom))
            .rounded(px_of(st.radius))
            .overflow_hidden()
            .bg(color(cx, ColorRef::Palette(ColorId::MenuBg)))
            .shadow(vec![BoxShadow {
                color: shadow_color,
                offset: point(px_of(shadow.offset.x), px_of(shadow.offset.y)),
                blur_radius: px_of(shadow.blur_radius),
                spread_radius: px(0.0),
                inset: false,
            }])
            .children(items)
    }
}
