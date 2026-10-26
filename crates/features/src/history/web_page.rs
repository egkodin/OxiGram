//! A text message's link preview (`HistoryView::WebPage`): the site, the
//! title and the description in a box tinted like a reply.

use gpui::{
    AnyElement, App, ElementId, Hsla, IntoElement, ParentElement, StatefulInteractiveElement as _,
    Styled, StyledText, TextRun, div, prelude::FluentBuilder as _,
};
use model::message::LinkPreview;
use style::chat::{
    HISTORY_PAGE_PREVIEW, HISTORY_REPLY_BOTTOM, HISTORY_REPLY_TOP, WEB_PAGE_DESCRIPTION_FONT,
    WEB_PAGE_TITLE_FONT,
};
use style::{ColorRef, Font};
use ui::{color, font, font_height};

use super::body::OnLink;
use super::headers::quote_frame;
use super::text::Link;
use crate::px_of;

/// A preview shows at most this many lines (`linesMax`).
const MAX_LINES: usize = 5;
/// The title wraps to two lines at most.
const TITLE_LINES: usize = 2;

/// What the box shows and does.
pub(super) struct WebPage<'a> {
    pub preview: &'a LinkPreview,
    pub id: ElementId,
    /// The tint, the site name's color and the text color.
    pub colors: [ColorRef; 3],
    /// Transparent text after the last line, where the time goes.
    pub room: Option<String>,
    pub on_link: OnLink,
}

/// How many lines the description may take (`restLines`).
// shortcut: counts the title as two lines when it is long, not by measuring it; exact once text layout is measured (M8).
fn description_lines(preview: &LinkPreview) -> usize {
    let site = usize::from(!preview.site_name.is_empty());
    let title = match preview.title.chars().count() {
        0 => 0,
        // About one line of the narrowest bubble.
        1..=40 => 1,
        _ => TITLE_LINES,
    };
    MAX_LINES - site - title
}

pub(super) fn element(page: WebPage<'_>, cx: &App) -> AnyElement {
    let WebPage {
        preview,
        id,
        colors: [tint, name_fg, text_fg],
        room,
        on_link,
    } = page;
    let text_fg = color(cx, text_fg);
    let mut parts: Vec<(String, Font, Hsla, usize)> = Vec::new();
    if !preview.site_name.is_empty() {
        parts.push((
            preview.site_name.clone(),
            WEB_PAGE_TITLE_FONT,
            color(cx, name_fg),
            1,
        ));
    }
    if !preview.title.is_empty() {
        parts.push((
            preview.title.clone(),
            WEB_PAGE_TITLE_FONT,
            text_fg,
            TITLE_LINES,
        ));
    }
    if !preview.description.text.is_empty() {
        // shortcut: the description drops its entities; they come with rich previews in M5.
        parts.push((
            preview.description.text.clone(),
            WEB_PAGE_DESCRIPTION_FONT,
            text_fg,
            description_lines(preview),
        ));
    }
    let last = parts.len().saturating_sub(1);
    let is_last_in_bubble = room.is_some();
    let link = if preview.is_url_in_text {
        Link::Url(preview.url.clone())
    } else {
        Link::HiddenUrl(preview.url.clone())
    };
    quote_frame(id, color(cx, tint), HISTORY_PAGE_PREVIEW.padding)
        .cursor_pointer()
        .on_click(move |_, window, cx| on_link(link.clone(), window, cx))
        .mt(px_of(HISTORY_REPLY_TOP))
        // Below the text the time goes on the box's last line.
        .when(!is_last_in_bubble, |this| {
            this.mb(px_of(HISTORY_REPLY_BOTTOM))
        })
        .children(parts.into_iter().enumerate().map(|(index, part)| {
            let room = (index == last).then(|| room.clone()).flatten();
            line(part, room)
        }))
        .into_any_element()
}

fn line(
    (text, line_font, fg, lines): (String, Font, Hsla, usize),
    room: Option<String>,
) -> AnyElement {
    let run = |len, color| TextRun {
        len,
        font: font(line_font),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let mut runs = vec![run(text.len(), fg)];
    let mut shown = text;
    if let Some(room) = room {
        runs.push(run(room.len(), Hsla::transparent_black()));
        shown.push_str(&room);
    }
    div()
        .line_clamp(lines)
        .font(font(line_font))
        .text_size(px_of(line_font.size))
        .line_height(font_height(line_font))
        .child(StyledText::new(shown).with_runs(runs))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_description_gets_the_lines_the_header_leaves() {
        let preview = |site: &str, title: &str| LinkPreview {
            site_name: site.to_owned(),
            title: title.to_owned(),
            ..LinkPreview::default()
        };
        assert_eq!(description_lines(&preview("", "")), 5);
        assert_eq!(description_lines(&preview("Site", "Short")), 3);
        assert_eq!(description_lines(&preview("Site", &"long ".repeat(20))), 2);
    }
}
