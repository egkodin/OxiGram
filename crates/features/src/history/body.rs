//! A message's text in its bubble: plain paragraphs, quotes and code
//! blocks (`Ui::Text::String::draw` with `st::messageTextStyle`), with
//! clickable links.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Div, ElementId, Hsla, InteractiveElement as _, InteractiveText, IntoElement,
    ParentElement, Styled, StyledText, TextRun, Window, div,
};
use model::text::FormattedText;
use style::basic::QuoteStyle;
use style::chat::{MESSAGE_TEXT_STYLE, MSG_FONT};
use style::{ColorId, ColorRef};
use ui::{color, font};

use super::text::{self, BlockKind, Link, Palette};
use crate::px_of;

/// What a click on a link does.
pub(super) type OnLink = Rc<dyn Fn(Link, &mut Window, &mut App)>;

/// tdesktop's quote and code backgrounds (`kDefaultBgOpacity`).
const BLOCK_BG_OPACITY: f32 = 0.12;
/// Their left lines (`kDefaultOutline1Opacity`).
const OUTLINE_OPACITY: f32 = 0.9;
/// A code block's language (`kDefaultOutline3Opacity`).
const HEADER_OPACITY: f32 = 0.6;

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
    /// The quote color: the sender's name color in incoming messages.
    pub quote: ColorId,
    pub is_spoiler_revealed: bool,
    /// Transparent text after the last line that leaves room for the time.
    pub room: String,
    pub on_link: OnLink,
}

fn palette(body: &Body<'_>, cx: &App) -> Palette {
    let (fg, link, mono, spoiler) = if body.is_out {
        (
            ColorId::HistoryTextOutFg,
            ColorId::HistoryLinkOutFg,
            ColorId::MsgOutMonoFg,
            ColorId::MsgOutDateFg,
        )
    } else {
        (
            ColorId::HistoryTextInFg,
            ColorId::HistoryLinkInFg,
            ColorId::MsgInMonoFg,
            ColorId::MsgInDateFg,
        )
    };
    let get = |id| color(cx, ColorRef::Palette(id));
    Palette {
        fg: get(fg),
        link: get(link),
        mono: get(mono),
        spoiler: get(spoiler),
    }
}

/// One paragraph's styled text, links clickable.
fn paragraph(
    body: &Body<'_>,
    index: usize,
    range: std::ops::Range<usize>,
    room: Option<&str>,
    colors: Palette,
) -> AnyElement {
    let base = font(MSG_FONT);
    let mut runs = text::runs(
        body.text,
        range.clone(),
        &base,
        MONO_FAMILY,
        colors,
        body.is_spoiler_revealed,
    );
    let mut shown = body.text.text[range.clone()].to_owned();
    if let Some(room) = room {
        shown.push_str(room);
        runs.push(TextRun {
            len: room.len(),
            font: base,
            color: Hsla::transparent_black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        });
    }
    let styled = StyledText::new(shown).with_runs(runs);
    let links = text::links(body.text, range, body.is_spoiler_revealed);
    if links.is_empty() {
        return styled.into_any_element();
    }
    let (ranges, targets): (Vec<_>, Vec<_>) = links.into_iter().unzip();
    let on_link = body.on_link.clone();
    InteractiveText::new(("paragraph", index), styled)
        .on_click(ranges, move |index, window, cx| {
            if let Some(link) = targets.get(index) {
                on_link(link.clone(), window, cx);
            }
        })
        .into_any_element()
}

/// A quote or code block: tinted, rounded, a line on the left.
// shortcut: no quote and copy icons, and long code lines wrap instead of scrolling; both come with M8 polish.
fn tinted(st: &QuoteStyle, tint: Hsla, header: Option<AnyElement>, content: AnyElement) -> Div {
    let radius = px_of(st.radius);
    div()
        .relative()
        .rounded(radius)
        .bg(tint.opacity(BLOCK_BG_OPACITY))
        .pl(px_of(st.padding.left))
        .pr(px_of(st.padding.right))
        .pt(px_of(st.padding.top))
        .pb(px_of(st.padding.bottom))
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
        .child(content)
}

pub(super) fn element(body: Body<'_>, cx: &App) -> AnyElement {
    let colors = palette(&body, cx);
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
            let content = paragraph(&body, index, block.range.clone(), room, colors);
            match &block.kind {
                BlockKind::Text => content,
                BlockKind::Quote => {
                    let tint = color(cx, ColorRef::Palette(body.quote));
                    tinted(&style.blockquote, tint, None, content).into_any_element()
                }
                BlockKind::Pre { language } => {
                    let header = language.as_ref().map(|language| {
                        div()
                            .h(px_of(style.pre.header))
                            .text_color(colors.mono.opacity(HEADER_OPACITY))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(language.clone())
                            .into_any_element()
                    });
                    tinted(&style.pre, colors.mono, header, content).into_any_element()
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
        .with_runs(vec![TextRun {
            len: room.len(),
            font: font(MSG_FONT),
            color: Hsla::transparent_black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }])
        .into_any_element()
}
