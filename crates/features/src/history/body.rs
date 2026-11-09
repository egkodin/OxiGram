//! A message's text in its bubble: plain paragraphs, quotes and code
//! blocks (`Ui::Text::String::draw` with `st::messageTextStyle`), with
//! clickable links.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Div, ElementId, Hsla, InteractiveElement as _, InteractiveText, IntoElement,
    MouseButton, ParentElement, SharedString, StatefulInteractiveElement as _, Styled, StyledText,
    TextRun, Window, div,
};
use i18n::key;
use model::text::FormattedText;
use style::basic::{QuoteStyle, TextPalette};
use style::chat::{
    IN_TEXT_PALETTE, IN_TEXT_PALETTE_SELECTED, MESSAGE_TEXT_STYLE, MSG_FONT, OUT_TEXT_PALETTE,
    OUT_TEXT_PALETTE_SELECTED,
};
use style::{ColorId, ColorRef};
use ui::{color, font};

use super::inline_emoji;
use super::selection::{OnPointer, Pointer};
use super::text::{self, BlockKind, Colors, Link, Look};
use crate::lang::tr;
use crate::px_of;

/// What a click on a link does.
pub(super) type OnLink = Rc<dyn Fn(Link, &mut Window, &mut App)>;

/// tdesktop's quote and code backgrounds (`kDefaultBgOpacity`).
const BLOCK_BG_OPACITY: f32 = 0.12;
/// Their left lines (`kDefaultOutline1Opacity`).
const OUTLINE_OPACITY: f32 = 0.9;
/// A code block's header band (`kDefaultOutline2Opacity`).
const HEADER_BG_OPACITY: f32 = 0.3;
/// Its copy icon (`kDefaultOutline3Opacity`).
const ICON_OPACITY: f32 = 0.6;

#[cfg(target_os = "macos")]
const MONO_FAMILY: &str = "Menlo";
#[cfg(target_os = "windows")]
const MONO_FAMILY: &str = "Consolas";
// shortcut: Linux relies on fontconfig's `monospace` alias; bundle a mono font if a distro lacks one.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const MONO_FAMILY: &str = "monospace";

/// The text to draw and how.
pub(super) struct Body<'a> {
    pub id: ElementId,
    pub text: &'a FormattedText,
    pub is_out: bool,
    /// The whole message is selected, so it takes the selected colors.
    pub is_selected: bool,
    /// The selected part of the text.
    pub selected: Option<Range<usize>>,
    /// The quote color: the sender's name color in incoming messages.
    pub quote: ColorId,
    pub is_spoiler_revealed: bool,
    /// Transparent text after the last line that leaves room for the time.
    pub room: String,
    pub on_link: OnLink,
    pub on_pointer: OnPointer,
    pub emoji: inline_emoji::Pictures,
}

/// The text palette and the plain text color for the message's side and
/// selection.
fn palette(body: &Body<'_>) -> (&'static TextPalette, ColorId) {
    match (body.is_out, body.is_selected) {
        (false, false) => (&IN_TEXT_PALETTE, ColorId::HistoryTextInFg),
        (false, true) => (&IN_TEXT_PALETTE_SELECTED, ColorId::HistoryTextInFgSelected),
        (true, false) => (&OUT_TEXT_PALETTE, ColorId::HistoryTextOutFg),
        (true, true) => (
            &OUT_TEXT_PALETTE_SELECTED,
            ColorId::HistoryTextOutFgSelected,
        ),
    }
}

fn look<'a>(body: &Body<'_>, font: &'a gpui::Font, cx: &App) -> Look<'a> {
    let (palette, fg) = palette(body);
    let get = |color_ref| color(cx, color_ref);
    Look {
        font,
        mono_family: MONO_FAMILY,
        colors: Colors {
            fg: get(ColorRef::Palette(fg)),
            link: get(palette.link_fg),
            mono: get(palette.mono_fg),
            spoiler: get(palette.spoiler_fg),
        },
        selected_colors: Colors {
            fg: get(palette.select_fg),
            link: get(palette.select_link_fg),
            mono: get(palette.select_mono_fg),
            spoiler: get(palette.select_spoiler_fg),
        },
        select_bg: get(palette.select_bg),
        is_spoiler_revealed: body.is_spoiler_revealed,
        selected: body.selected.clone(),
    }
}

/// One paragraph's styled text, links clickable; presses and drags over
/// it report where in the text they are.
fn paragraph(
    body: &Body<'_>,
    index: usize,
    range: Range<usize>,
    room: Option<&str>,
    look: &Look<'_>,
) -> AnyElement {
    let places = inline_emoji::places(body.text, &range, &body.emoji);
    let hidden: Vec<Range<usize>> = places.iter().map(|(range, _)| range.clone()).collect();
    let mut runs = inline_emoji::hide(text::runs(body.text, range.clone(), look), &hidden);
    let mut shown = body.text.text[range.clone()].to_owned();
    if let Some(room) = room {
        shown.push_str(room);
        runs.push(transparent_run(room.len()));
    }
    let styled = StyledText::new(shown).with_runs(runs);
    let layout = styled.layout().clone();
    let emoji_layout = layout.clone();
    let links = text::links(body.text, range.clone(), body.is_spoiler_revealed);
    let content = if links.is_empty() {
        styled.into_any_element()
    } else {
        let (ranges, targets): (Vec<_>, Vec<_>) = links.into_iter().unzip();
        let on_link = body.on_link.clone();
        InteractiveText::new(("paragraph", index), styled)
            .on_click(ranges, move |index, window, cx| {
                if let Some(link) = targets.get(index) {
                    on_link(link.clone(), window, cx);
                }
            })
            .into_any_element()
    };
    let offset_at = move |position| {
        let index = layout
            .index_for_position(position)
            .unwrap_or_else(|nearest| nearest);
        range.start + index.min(range.len())
    };
    let offset_at = Rc::new(offset_at);
    let (on_press, on_drag, on_menu) = (
        body.on_pointer.clone(),
        body.on_pointer.clone(),
        body.on_pointer.clone(),
    );
    let (drag_offset, menu_offset) = (offset_at.clone(), offset_at.clone());
    let overlay = (!places.is_empty()).then(|| inline_emoji::overlay(emoji_layout, places));
    div()
        .relative()
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            on_press(Pointer::PressText(offset_at(event.position)), window, cx);
            cx.stop_propagation();
        })
        .on_mouse_down(MouseButton::Right, move |event, window, cx| {
            on_menu(Pointer::Menu(Some(menu_offset(event.position))), window, cx);
            cx.stop_propagation();
        })
        .on_mouse_move(move |event, window, cx| {
            if event.pressed_button == Some(MouseButton::Left) {
                on_drag(Pointer::Drag(Some(drag_offset(event.position))), window, cx);
                cx.stop_propagation();
            }
        })
        .child(content)
        .children(overlay)
        .into_any_element()
}

fn transparent_run(len: usize) -> TextRun {
    TextRun {
        len,
        font: font(MSG_FONT),
        color: Hsla::transparent_black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

/// A quote or code block: tinted, rounded, a line on the left, and a
/// header band across the top for code.
// shortcut: no quote icon, and long code lines wrap instead of scrolling; both come with M8 polish.
fn tinted(st: &QuoteStyle, tint: Hsla, header: Option<AnyElement>, content: AnyElement) -> Div {
    let radius = px_of(st.radius);
    div()
        .relative()
        .rounded(radius)
        .overflow_hidden()
        .bg(tint.opacity(BLOCK_BG_OPACITY))
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px_of(st.outline))
                .rounded_l(radius)
                .bg(tint.opacity(OUTLINE_OPACITY)),
        )
        .children(header)
        .child(
            div()
                .pl(px_of(st.padding.left))
                .pr(px_of(st.padding.right))
                .pt(px_of(st.padding.top))
                .pb(px_of(st.padding.bottom))
                .child(content),
        )
}

/// A code block's header: its language, or "copy", and the copy icon; a
/// click copies the code (`QuoteDetails::copy`).
// shortcut: no syntax highlighting; tdesktop's highlighter (lib_spellcheck, libprisma) is not in the checkout.
fn pre_header(
    body: &Body<'_>,
    index: usize,
    language: Option<&str>,
    code: &str,
    look: &Look<'_>,
    cx: &App,
) -> AnyElement {
    let st = &MESSAGE_TEXT_STYLE.pre;
    let mono = look.colors.mono;
    let title = language.map_or_else(
        || tr(cx, key::lng_code_block_header_copy),
        |language| SharedString::from(language.to_owned()),
    );
    let (on_link, code) = (body.on_link.clone(), code.to_owned());
    div()
        .id(("pre-header", index))
        .relative()
        .h(px_of(st.header))
        .bg(mono.opacity(HEADER_BG_OPACITY))
        .cursor_pointer()
        .child(
            div()
                .absolute()
                .left(px_of(st.header_position.x))
                .top(px_of(st.header_position.y))
                .font_family(look.mono_family)
                .text_color(mono)
                .whitespace_nowrap()
                .child(title),
        )
        .child(
            div()
                .absolute()
                .right(px_of(st.icon_position.x))
                .top(px_of(st.icon_position.y))
                .opacity(ICON_OPACITY)
                .child(ui::icon(st.icon).color(palette(body).0.mono_fg)),
        )
        .on_click(move |_, window, cx| on_link(Link::Code(code.clone()), window, cx))
        .into_any_element()
}

pub(super) fn element(body: Body<'_>, cx: &App) -> AnyElement {
    let base = font(MSG_FONT);
    let look = look(&body, &base, cx);
    let mono = look.colors.mono;
    let blocks = text::blocks(body.text);
    let ends_in_text = blocks
        .last()
        .is_none_or(|block| block.kind == BlockKind::Text);
    let last = blocks.len().saturating_sub(1);
    let style = &MESSAGE_TEXT_STYLE;
    let mut children: Vec<AnyElement> = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let room = (ends_in_text && index == last).then_some(body.room.as_str());
            let content = paragraph(&body, index, block.range.clone(), room, &look);
            match &block.kind {
                BlockKind::Text => content,
                BlockKind::Quote => {
                    let tint = color(cx, ColorRef::Palette(body.quote));
                    tinted(&style.blockquote, tint, None, content).into_any_element()
                }
                BlockKind::Pre { language } => {
                    let code = &body.text.text[block.range.clone()];
                    let header = pre_header(&body, index, language.as_deref(), code, &look, cx);
                    tinted(&style.pre, mono, Some(header), content).into_any_element()
                }
            }
        })
        .collect();
    if !ends_in_text || blocks.is_empty() {
        // The time goes on a line of its own under a closing block.
        children.push(room_line(&body.room));
    }
    div()
        .id(body.id)
        .flex()
        .flex_col()
        .gap(px_of(style.blockquote.vertical_skip))
        .children(children)
        .into_any_element()
}

/// Transparent text as tall as a line, as wide as the time.
fn room_line(room: &str) -> AnyElement {
    StyledText::new(room.to_owned())
        .with_runs(vec![transparent_run(room.len())])
        .into_any_element()
}
