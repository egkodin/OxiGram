//! lib_ui's `FlatLabel`: wrapped text with bold spans and links.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, ElementId, FontWeight, HighlightStyle, InteractiveText, IntoElement, ParentElement,
    Pixels, RenderOnce, SharedString, StyledText, TextAlign, UnderlineStyle, Window, div,
    prelude::*, px,
};
use style::{Align, ColorRef};

use crate::font::{font_height, text_style};
use crate::theme::color;

/// Text with the spans tdesktop's rich strings carry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Markup {
    text: String,
    bold: Vec<Range<usize>>,
    links: Vec<Range<usize>>,
}

impl Markup {
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    /// Reads `**bold**` spans, as `Ui::Text::RichLangValue` does.
    #[must_use]
    pub fn rich(source: &str) -> Self {
        let mut markup = Self::default();
        for (index, part) in source.split("**").enumerate() {
            if index % 2 == 1 {
                markup = markup.bold(part);
            } else {
                markup = markup.text(part);
            }
        }
        markup
    }

    #[must_use]
    pub fn text(mut self, text: &str) -> Self {
        self.text.push_str(text);
        self
    }

    #[must_use]
    pub fn bold(mut self, text: &str) -> Self {
        let start = self.text.len();
        self.text.push_str(text);
        self.bold.push(start..self.text.len());
        self
    }

    /// Appends a link; links are numbered in the order they are added.
    #[must_use]
    pub fn link(mut self, text: &str) -> Self {
        let start = self.text.len();
        self.text.push_str(text);
        self.links.push(start..self.text.len());
        self
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

type LinkHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// A label drawn with a `style::widgets::FlatLabel`.
#[derive(IntoElement)]
pub struct FlatLabel {
    id: ElementId,
    style: &'static style::widgets::FlatLabel,
    markup: Markup,
    width: Option<Pixels>,
    color: Option<ColorRef>,
    on_link: Option<LinkHandler>,
}

impl FlatLabel {
    pub fn new(
        id: impl Into<ElementId>,
        style: &'static style::widgets::FlatLabel,
        markup: Markup,
    ) -> Self {
        Self {
            id: id.into(),
            style,
            markup,
            width: None,
            color: None,
            on_link: None,
        }
    }

    /// Wraps at `width`, like `resizeToWidth`; otherwise the label takes its
    /// natural width.
    #[must_use]
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = Some(width);
        self
    }

    #[must_use]
    pub fn color(mut self, color: ColorRef) -> Self {
        self.color = Some(color);
        self
    }

    /// Calls `handler` with the link's index when a link is clicked.
    #[must_use]
    pub fn on_link(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_link = Some(Rc::new(handler));
        self
    }
}

impl std::fmt::Debug for FlatLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlatLabel")
            .field("id", &self.id)
            .field("markup", &self.markup)
            .finish_non_exhaustive()
    }
}

impl RenderOnce for FlatLabel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let font = self.style.style.font;
        let line_height = if self.style.style.line_height.0 > 0 {
            #[expect(clippy::cast_precision_loss, reason = "line heights are small")]
            px(self.style.style.line_height.0 as f32)
        } else {
            font_height(font)
        };
        let mut text = text_style(font, color(cx, self.color.unwrap_or(self.style.text_fg)));
        text.line_height = line_height.into();
        let hovered = window.use_keyed_state(self.id.clone(), cx, |_, _| None::<usize>);
        let hovered_link = *hovered.read(cx);
        let link_color = color(cx, self.style.palette.link_fg);

        let mut highlights: Vec<(Range<usize>, HighlightStyle)> = self
            .markup
            .bold
            .iter()
            .map(|range| {
                (
                    range.clone(),
                    HighlightStyle {
                        font_weight: Some(FontWeight::SEMIBOLD),
                        ..HighlightStyle::default()
                    },
                )
            })
            .collect();
        for (index, range) in self.markup.links.iter().enumerate() {
            let is_hovered = hovered_link == Some(index);
            highlights.push((
                range.clone(),
                HighlightStyle {
                    color: Some(link_color),
                    underline: is_hovered.then(|| UnderlineStyle {
                        color: Some(link_color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..HighlightStyle::default()
                },
            ));
        }
        highlights.sort_by_key(|(range, _)| range.start);

        let links = self.markup.links.clone();
        let styled = StyledText::new(SharedString::from(self.markup.text))
            .with_default_highlights(&text, highlights);
        let on_link = self.on_link;
        let hover_links = links.clone();
        let content = InteractiveText::new(self.id, styled)
            .on_click(links, move |index, window, cx| {
                if let Some(on_link) = &on_link {
                    on_link(index, window, cx);
                }
            })
            .on_hover(move |position, _, _, cx| {
                let link = position.and_then(|position| {
                    hover_links
                        .iter()
                        .position(|range| range.contains(&position))
                });
                hovered.update(cx, |hovered, cx| {
                    if *hovered != link {
                        *hovered = link;
                        cx.notify();
                    }
                });
            });

        let align = match self.style.align {
            Align::Top | Align::Center | Align::Bottom => TextAlign::Center,
            Align::TopRight | Align::Right | Align::BottomRight => TextAlign::Right,
            _ => TextAlign::Left,
        };
        // StyledText takes the size and line height from the parent's text
        // style; the highlights only carry fonts and colors.
        div()
            .text_align(align)
            .font(crate::font::font(font))
            .text_size(text.font_size)
            .line_height(line_height)
            .text_color(text.color)
            .when_some(self.width, |label, width| label.w(width))
            .child(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bold_markup() {
        let markup = Markup::rich("A code was sent **via Telegram** to you");
        assert_eq!(markup.as_str(), "A code was sent via Telegram to you");
        assert_eq!(&markup.as_str()[markup.bold[0].clone()], "via Telegram");
    }

    #[test]
    fn numbers_links_in_order() {
        let markup = Markup::plain("By signing up, you agree to the ")
            .link("Terms of Service")
            .text(".");
        assert_eq!(
            &markup.as_str()[markup.links[0].clone()],
            "Terms of Service"
        );
    }
}
