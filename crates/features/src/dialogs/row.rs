//! One chat list row, laid out as tdesktop's `PaintRow` paints it with
//! `defaultDialogRow`.

use std::ops::Range;
use std::sync::Arc;

use gpui::{
    App, FontStyle, HighlightStyle, Image, IntoElement, ParentElement, RenderOnce, SharedString,
    StrikethroughStyle, Styled, StyledText, UnderlineStyle, Window, div, img, prelude::*, px,
};
use style::basic::{SEMIBOLD_FONT, TextPalette};
use style::dialogs::{
    DEFAULT_DIALOG_ROW, DIALOGS_CHAT_TYPE_SKIP, DIALOGS_DATE_FONT, DIALOGS_DATE_SKIP,
    DIALOGS_MINI_PREVIEW, DIALOGS_MINI_PREVIEW_RADIUS, DIALOGS_MINI_PREVIEW_SKIP,
    DIALOGS_MINI_PREVIEW_TOP, DIALOGS_ONLINE_BADGE_SIZE, DIALOGS_ONLINE_BADGE_SKIP,
    DIALOGS_ONLINE_BADGE_STROKE, DIALOGS_PINNED_ICON, DIALOGS_SCAM_FONT, DIALOGS_SCAM_PADDING,
    DIALOGS_SCAM_RADIUS, DIALOGS_SCAM_SKIP, DIALOGS_SEND_STATE_SKIP, DIALOGS_TEXT_FONT,
    DIALOGS_TEXT_PALETTE, DIALOGS_TEXT_PALETTE_ACTIVE, DIALOGS_TEXT_PALETTE_OVER,
    DIALOGS_UNREAD_FONT, DIALOGS_UNREAD_HEIGHT, DIALOGS_UNREAD_MENTION, DIALOGS_UNREAD_PADDING,
    DIALOGS_UNREAD_REACTION, DIALOGS_UNREAD_REACTION_MUTED, DIALOGS_VERIFIED_ICON,
    DIALOGS_VERIFIED_ICON_ACTIVE, DIALOGS_VERIFIED_ICON_OVER, ThreeStateIcon,
};
use style::dialogs_layout::{
    DIALOGS_TEXT_PALETTE_DRAFT, DIALOGS_TEXT_PALETTE_DRAFT_ACTIVE, DIALOGS_TEXT_PALETTE_DRAFT_OVER,
};
use style::{ColorId, ColorRef};
use ui::{Userpic, UserpicSource, color, font, font_height, icon};

use super::preview::{Preview, SpanKind};
use crate::px_of;

/// How the row is lit: as is, under the mouse, or the open chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lit {
    Normal,
    Over,
    Active,
}

/// What follows the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NameBadge {
    Verified,
    Premium,
    Scam,
    Fake,
}

#[derive(Debug, Clone)]
pub(crate) enum RowText {
    Message {
        preview: Preview,
        thumbnail: Option<Arc<Image>>,
    },
    Draft(Preview),
    /// Someone typing or sending, in the service color.
    Action(SharedString),
    Empty,
}

/// The counters at the right of the text line (`BadgesState`).
#[derive(Debug, Clone, Default)]
pub(crate) struct Badges {
    /// The unread count; an empty text for a chat marked as unread.
    pub unread: Option<SharedString>,
    pub is_muted: bool,
    pub has_mention: bool,
    pub has_reaction: bool,
}

impl Badges {
    fn is_empty(&self) -> bool {
        self.unread.is_none() && !self.has_mention && !self.has_reaction
    }
}

/// Everything a row shows, prepared from the model.
#[derive(Debug, Clone, IntoElement)]
pub(crate) struct Row {
    pub userpic: UserpicSource,
    pub is_online: bool,
    pub name: SharedString,
    pub type_icon: Option<&'static ThreeStateIcon>,
    pub name_badge: Option<NameBadge>,
    pub scam_text: SharedString,
    pub date: Option<SharedString>,
    pub send_state: Option<&'static ThreeStateIcon>,
    pub text: RowText,
    pub badges: Badges,
    pub is_pinned: bool,
    pub lit: Lit,
}

fn three_state(icons: &ThreeStateIcon, lit: Lit) -> style::Icon {
    match lit {
        Lit::Normal => icons.icon,
        Lit::Over => icons.over,
        Lit::Active => icons.active,
    }
}

fn pick(lit: Lit, normal: ColorId, over: ColorId, active: ColorId) -> ColorRef {
    ColorRef::Palette(match lit {
        Lit::Normal => normal,
        Lit::Over => over,
        Lit::Active => active,
    })
}

impl Row {
    fn background(&self) -> ColorRef {
        pick(
            self.lit,
            ColorId::DialogsBg,
            ColorId::DialogsBgOver,
            ColorId::DialogsBgActive,
        )
    }

    fn name_line(&self, cx: &App) -> impl IntoElement {
        let lit = self.lit;
        let row = &DEFAULT_DIALOG_ROW;
        let name_color = pick(
            lit,
            ColorId::DialogsNameFg,
            ColorId::DialogsNameFgOver,
            ColorId::DialogsNameFgActive,
        );
        let date_color = pick(
            lit,
            ColorId::DialogsDateFg,
            ColorId::DialogsDateFgOver,
            ColorId::DialogsDateFgActive,
        );
        div()
            .absolute()
            .left(px_of(row.name_left))
            .right(px_of(row.padding.right))
            .top(px_of(row.name_top))
            .h(font_height(SEMIBOLD_FONT))
            .flex()
            .items_center()
            .when_some(self.type_icon, |line, icons| {
                line.child(
                    div()
                        .flex_none()
                        .mr(px_of(DIALOGS_CHAT_TYPE_SKIP))
                        .child(icon(three_state(icons, lit))),
                )
            })
            .child(
                div()
                    .min_w_0()
                    .flex_shrink(1.0)
                    .truncate()
                    .font(font(SEMIBOLD_FONT))
                    .text_size(px_of(SEMIBOLD_FONT.size))
                    .text_color(color(cx, name_color))
                    .child(self.name.clone()),
            )
            .when_some(self.name_badge, |line, badge| {
                line.child(self.name_badge_view(badge, cx))
            })
            .child(div().flex_1())
            .when_some(self.send_state, |line, icons| {
                line.child(
                    div()
                        .flex_none()
                        .w(px_of(DIALOGS_SEND_STATE_SKIP))
                        .child(icon(three_state(icons, lit))),
                )
            })
            .when_some(self.date.clone(), |line, date| {
                line.child(
                    div()
                        .flex_none()
                        .ml(px_of(DIALOGS_DATE_SKIP))
                        .font(font(DIALOGS_DATE_FONT))
                        .text_size(px_of(DIALOGS_DATE_FONT.size))
                        .text_color(color(cx, date_color))
                        .child(date),
                )
            })
    }

    fn name_badge_view(&self, badge: NameBadge, cx: &App) -> gpui::AnyElement {
        let lit = self.lit;
        match badge {
            NameBadge::Verified => div()
                .flex_none()
                .child(icon(match lit {
                    Lit::Normal => DIALOGS_VERIFIED_ICON,
                    Lit::Over => DIALOGS_VERIFIED_ICON_OVER,
                    Lit::Active => DIALOGS_VERIFIED_ICON_ACTIVE,
                }))
                .into_any_element(),
            NameBadge::Premium => div()
                .flex_none()
                .child(icon(three_state(
                    &style::dialogs::DIALOGS_PREMIUM_ICON,
                    lit,
                )))
                .into_any_element(),
            NameBadge::Scam | NameBadge::Fake => {
                let fg = color(
                    cx,
                    pick(
                        lit,
                        ColorId::DialogsScamFg,
                        ColorId::DialogsScamFgOver,
                        ColorId::DialogsScamFgActive,
                    ),
                );
                div()
                    .flex_none()
                    .ml(px_of(DIALOGS_SCAM_SKIP))
                    .px(px_of(DIALOGS_SCAM_PADDING.left))
                    .border_1()
                    .border_color(fg)
                    .rounded(px_of(DIALOGS_SCAM_RADIUS))
                    .font(font(DIALOGS_SCAM_FONT))
                    .text_size(px_of(DIALOGS_SCAM_FONT.size))
                    .text_color(fg)
                    .child(self.scam_text.clone())
                    .into_any_element()
            }
        }
    }

    fn text_line(&self, cx: &App) -> impl IntoElement {
        let row = &DEFAULT_DIALOG_ROW;
        div()
            .absolute()
            .left(px_of(row.text_left))
            .right(px_of(row.padding.right))
            .top(px_of(row.text_top))
            .h(font_height(DIALOGS_TEXT_FONT))
            .flex()
            .items_center()
            .child(self.text_view(cx))
            .child(self.badges_view(cx))
    }

    fn text_view(&self, cx: &App) -> impl IntoElement {
        let lit = self.lit;
        let text_color = color(
            cx,
            pick(
                lit,
                ColorId::DialogsTextFg,
                ColorId::DialogsTextFgOver,
                ColorId::DialogsTextFgActive,
            ),
        );
        let line = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .font(font(DIALOGS_TEXT_FONT))
            .text_size(px_of(DIALOGS_TEXT_FONT.size))
            .line_height(font_height(DIALOGS_TEXT_FONT))
            .text_color(text_color);
        let (preview, palette, thumbnail) = match &self.text {
            RowText::Message { preview, thumbnail } => {
                (preview, text_palette(lit), thumbnail.clone())
            }
            RowText::Draft(preview) => (preview, draft_palette(lit), None),
            RowText::Action(action) => {
                let service = pick(
                    lit,
                    ColorId::DialogsTextFgService,
                    ColorId::DialogsTextFgServiceOver,
                    ColorId::DialogsTextFgServiceActive,
                );
                return line.child(
                    div()
                        .truncate()
                        .text_color(color(cx, service))
                        .child(action.clone()),
                );
            }
            RowText::Empty => return line,
        };
        let highlights = highlights(preview, palette, cx);
        line.when_some(thumbnail, |line, thumbnail| {
            line.child(
                img(thumbnail)
                    .flex_none()
                    .size(px_of(DIALOGS_MINI_PREVIEW))
                    .mt(px_of(DIALOGS_MINI_PREVIEW_TOP))
                    .mr(px_of(DIALOGS_MINI_PREVIEW_SKIP))
                    .rounded(px_of(DIALOGS_MINI_PREVIEW_RADIUS)),
            )
        })
        .child(div().min_w_0().truncate().child(
            StyledText::new(SharedString::from(preview.text.clone())).with_highlights(highlights),
        ))
    }

    fn badges_view(&self, cx: &App) -> impl IntoElement {
        let lit = self.lit;
        let badges = &self.badges;
        let gap = px_of(DIALOGS_UNREAD_PADDING);
        let mut view = div().flex_none().flex().items_center().gap(gap);
        if badges.is_empty() {
            if self.is_pinned {
                view = view
                    .ml(gap)
                    .child(icon(three_state(&DIALOGS_PINNED_ICON, lit)));
            }
            return view;
        }
        view = view.ml(gap);
        if badges.has_reaction {
            let icons = if badges.is_muted {
                &DIALOGS_UNREAD_REACTION_MUTED
            } else {
                &DIALOGS_UNREAD_REACTION
            };
            view = view.child(icon(three_state(icons, lit)));
        }
        if badges.has_mention {
            view = view.child(icon(three_state(&DIALOGS_UNREAD_MENTION, lit)));
        }
        if let Some(count) = badges.unread.clone() {
            view = view.child(unread_badge(count, badges.is_muted, lit, cx));
        }
        view
    }

    /// The green dot of an online user, cut out of the userpic
    /// (`Row::paintUserpic`).
    fn online_badge(&self, cx: &App) -> impl IntoElement {
        let row = &DEFAULT_DIALOG_ROW;
        let size = px_of(DIALOGS_ONLINE_BADGE_SIZE);
        let stroke = px_of(DIALOGS_ONLINE_BADGE_STROKE);
        let half = stroke / 2.0;
        let dot = if self.lit == Lit::Active {
            ColorId::DialogsOnlineBadgeFgActive
        } else {
            ColorId::DialogsOnlineBadgeFg
        };
        let photo = px_of(row.photo_size);
        div()
            .absolute()
            .left(
                px_of(row.padding.left) + photo - px_of(DIALOGS_ONLINE_BADGE_SKIP.x) - size - half,
            )
            .top(px_of(row.padding.top) + photo - px_of(DIALOGS_ONLINE_BADGE_SKIP.y) - size - half)
            .size(size + stroke)
            .rounded_full()
            .bg(color(cx, self.background()))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .size(size - stroke)
                    .rounded_full()
                    .bg(color(cx, ColorRef::Palette(dot))),
            )
    }
}

fn text_palette(lit: Lit) -> &'static TextPalette {
    match lit {
        Lit::Normal => &DIALOGS_TEXT_PALETTE,
        Lit::Over => &DIALOGS_TEXT_PALETTE_OVER,
        Lit::Active => &DIALOGS_TEXT_PALETTE_ACTIVE,
    }
}

fn draft_palette(lit: Lit) -> &'static TextPalette {
    match lit {
        Lit::Normal => &DIALOGS_TEXT_PALETTE_DRAFT,
        Lit::Over => &DIALOGS_TEXT_PALETTE_DRAFT_OVER,
        Lit::Active => &DIALOGS_TEXT_PALETTE_DRAFT_ACTIVE,
    }
}

/// `PaintUnreadBadge`: a pill at least as wide as it is high.
pub(super) fn unread_badge(
    count: SharedString,
    is_muted: bool,
    lit: Lit,
    cx: &App,
) -> impl IntoElement {
    let bg = if is_muted {
        pick(
            lit,
            ColorId::DialogsUnreadBgMuted,
            ColorId::DialogsUnreadBgMutedOver,
            ColorId::DialogsUnreadBgMutedActive,
        )
    } else {
        pick(
            lit,
            ColorId::DialogsUnreadBg,
            ColorId::DialogsUnreadBgOver,
            ColorId::DialogsUnreadBgActive,
        )
    };
    let fg = pick(
        lit,
        ColorId::DialogsUnreadFg,
        ColorId::DialogsUnreadFgOver,
        ColorId::DialogsUnreadFgActive,
    );
    let height = px_of(DIALOGS_UNREAD_HEIGHT);
    div()
        .h(height)
        .min_w(height)
        .px(px_of(DIALOGS_UNREAD_PADDING))
        .rounded(height / 2.0)
        .bg(color(cx, bg))
        .flex()
        .items_center()
        .justify_center()
        .font(font(DIALOGS_UNREAD_FONT))
        .text_size(px_of(DIALOGS_UNREAD_FONT.size))
        .text_color(color(cx, fg))
        .child(count)
}

/// The preview's styles as gpui highlights: sorted and not overlapping.
fn highlights(
    preview: &Preview,
    palette: &TextPalette,
    cx: &App,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let link = color(cx, palette.link_fg);
    let mono = color(cx, palette.mono_fg);
    let spoiler = color(cx, palette.spoiler_fg);
    let style = |kind: SpanKind| -> HighlightStyle {
        match kind {
            SpanKind::Colorized => HighlightStyle {
                color: Some(link),
                ..HighlightStyle::default()
            },
            SpanKind::Semibold => HighlightStyle {
                font_weight: Some(gpui::FontWeight::SEMIBOLD),
                ..HighlightStyle::default()
            },
            SpanKind::Italic => HighlightStyle {
                font_style: Some(FontStyle::Italic),
                ..HighlightStyle::default()
            },
            SpanKind::Underline => HighlightStyle {
                underline: Some(UnderlineStyle {
                    thickness: px(1.0),
                    ..UnderlineStyle::default()
                }),
                ..HighlightStyle::default()
            },
            SpanKind::Strikethrough => HighlightStyle {
                strikethrough: Some(StrikethroughStyle {
                    thickness: px(1.0),
                    ..StrikethroughStyle::default()
                }),
                ..HighlightStyle::default()
            },
            SpanKind::Code => HighlightStyle {
                color: Some(mono),
                ..HighlightStyle::default()
            },
            // shortcut: a spoiler is a solid block of its color; the particle animation comes with M3's spoilers.
            SpanKind::Spoiler => HighlightStyle {
                color: Some(spoiler),
                background_color: Some(spoiler),
                ..HighlightStyle::default()
            },
        }
    };
    runs(preview)
        .into_iter()
        .map(|(range, kinds)| {
            let merged = kinds
                .into_iter()
                .map(style)
                .reduce(|merged, next| merged.highlight(next))
                .unwrap_or_default();
            (range, merged)
        })
        .collect()
}

/// The preview cut where any span starts or ends, each piece with the
/// kinds of the spans covering it; uncovered pieces are left out.
fn runs(preview: &Preview) -> Vec<(Range<usize>, Vec<SpanKind>)> {
    let text = &preview.text;
    let mut cuts: Vec<usize> = preview
        .spans
        .iter()
        .flat_map(|span| [span.range.start, span.range.end])
        .filter(|cut| *cut <= text.len() && text.is_char_boundary(*cut))
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .filter_map(|pair| {
            let range = pair[0]..pair[1];
            let kinds: Vec<SpanKind> = preview
                .spans
                .iter()
                .filter(|span| span.range.start <= range.start && range.end <= span.range.end)
                .map(|span| span.kind)
                .collect();
            (!kinds.is_empty()).then_some((range, kinds))
        })
        .collect()
}

impl RenderOnce for Row {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let row = &DEFAULT_DIALOG_ROW;
        div()
            .relative()
            .w_full()
            .h(px_of(row.height))
            .bg(color(cx, self.background()))
            .child(
                div()
                    .absolute()
                    .left(px_of(row.padding.left))
                    .top(px_of(row.padding.top))
                    .child(Userpic::new(px_of(row.photo_size), self.userpic.clone())),
            )
            .when(self.is_online, |view| view.child(self.online_badge(cx)))
            .child(self.name_line(cx))
            .child(self.text_line(cx))
    }
}

/// A row of the narrow list: the userpic with the counters at its corner
/// (`PaintRow` with `narrow`, `PaintNarrowCounter`).
#[derive(Debug, IntoElement)]
pub(crate) struct NarrowRow(pub Row);

impl RenderOnce for NarrowRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let st = &DEFAULT_DIALOG_ROW;
        let row = self.0;
        let photo = px_of(st.photo_size);
        let shows_counter = !row.badges.is_empty() && matches!(row.text, RowText::Message { .. });
        // shortcut: mention and reaction icons keep their wide look instead of tdesktop's round narrow badges.
        div()
            .relative()
            .w_full()
            .h(px_of(st.height))
            .bg(color(cx, row.background()))
            .child(
                div()
                    .absolute()
                    .left(px_of(st.padding.left))
                    .top(px_of(st.padding.top))
                    .child(Userpic::new(photo, row.userpic.clone())),
            )
            .when(row.is_online && !shows_counter, |view| {
                view.child(row.online_badge(cx))
            })
            .when(shows_counter, |view| {
                view.child(
                    div()
                        .absolute()
                        .left(px(0.0))
                        .w(px_of(st.padding.left) + photo)
                        .top(px_of(st.padding.top) + photo - px_of(DIALOGS_UNREAD_HEIGHT))
                        .flex()
                        .justify_end()
                        .child(row.badges_view(cx)),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::super::preview::Span;
    use super::*;

    #[test]
    fn overlapping_spans_split_into_runs() {
        let preview = Preview {
            text: "abcdefgh".to_owned(),
            spans: vec![
                Span {
                    range: 0..4,
                    kind: SpanKind::Italic,
                },
                Span {
                    range: 2..6,
                    kind: SpanKind::Underline,
                },
            ],
        };
        assert_eq!(
            runs(&preview),
            [
                (0..2, vec![SpanKind::Italic]),
                (2..4, vec![SpanKind::Italic, SpanKind::Underline]),
                (4..6, vec![SpanKind::Underline]),
            ]
        );
    }
}
