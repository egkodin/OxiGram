//! What the intro draws around its steps (`Intro::Widget` and
//! `Step::paintAnimated`): the cover, titles, errors, the back and next
//! buttons, and the links under them.

use gpui::{
    AnyElement, App, Entity, IntoElement, ParentElement, Pixels, SharedString, Styled, Window, div,
    linear_color_stop, linear_gradient, px,
};
use model::Environment;
use style::intro::{
    INTRO_BACK_BUTTON, INTRO_COVER_DESCRIPTION, INTRO_COVER_DESCRIPTION_TOP, INTRO_COVER_HEIGHT,
    INTRO_COVER_ICON_TOP, INTRO_COVER_ICONS_MIN_SKIP, INTRO_COVER_LEFT, INTRO_COVER_MAX_WIDTH,
    INTRO_COVER_RIGHT, INTRO_COVER_TITLE, INTRO_COVER_TITLE_TOP, INTRO_DESCRIPTION,
    INTRO_DESCRIPTION_TOP, INTRO_ERROR, INTRO_ERROR_CENTERED, INTRO_NEXT_BUTTON, INTRO_NEXT_SLIDE,
    INTRO_NEXT_TOP, INTRO_RESET_BOTTOM, INTRO_RESET_BUTTON, INTRO_SETTINGS_SKIP, INTRO_STEP_HEIGHT,
    INTRO_STEP_WIDTH, INTRO_TERMS_BOTTOM, INTRO_TERMS_LABEL, INTRO_TITLE, INTRO_TITLE_TOP,
};
use style::widgets::{BUTTON_RADIUS, DEFAULT_FLAT_LABEL, DEFAULT_LINK_BUTTON};
use style::{ColorId, ColorRef};
use ui::anim::{Easing, interpolate};
use ui::{FlatLabel, IconButton, LinkButton, Markup, RoundButton, color, font_height, icon_size};

use super::Intro;
use super::step::{Geometry, Header, Step, StepError};
use crate::lang::tr;
use crate::px_of;

/// "Continue in <language>" under the next button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LanguageLink {
    pub text: SharedString,
    /// The pack to switch to; empty for English.
    pub pack: String,
}

/// The distance from the content top to the next button.
pub(crate) fn next_top_offset() -> Pixels {
    px_of(INTRO_NEXT_TOP)
}

/// The top of the back button: under the cover when there is one.
pub(crate) fn controls_top(has_cover: bool) -> Pixels {
    if has_cover {
        px_of(INTRO_COVER_HEIGHT)
    } else {
        Pixels::ZERO
    }
}

pub(crate) struct DepartingFrame<'a> {
    pub header: &'a Header,
    pub step: &'a Step,
    pub had_cover: bool,
    pub backwards: bool,
}

/// One frame of the intro.
pub(crate) struct Frame<'a> {
    pub geometry: Geometry,
    pub header: &'a Header,
    pub step: &'a Step,
    pub error: Option<(&'a StepError, f32)>,
    pub departing: Option<DepartingFrame<'a>>,
    pub progress: f32,
    pub controls_top_from: Pixels,
    pub next_top_from: Pixels,
    pub next_shown: f32,
    pub reset_button: bool,
    pub terms: bool,
    pub language_link: Option<&'a LanguageLink>,
    pub environment: Environment,
}

fn lerp(from: Pixels, to: Pixels, t: f32) -> Pixels {
    px(interpolate(f32::from(from), f32::from(to), t))
}

impl Frame<'_> {
    pub fn render(self, intro: Entity<Intro>, _window: &mut Window, cx: &mut App) -> AnyElement {
        let g = self.geometry;
        let has_cover = self.header.has_cover;
        let p = self.progress;
        let cover_change = self
            .departing
            .as_ref()
            .is_some_and(|departing| departing.had_cover != has_cover);
        let content_top = g.content_top(has_cover);

        let controls_top = if cover_change {
            lerp(self.controls_top_from, controls_top(has_cover), p)
        } else {
            controls_top(has_cover)
        };
        let next_top_to = content_top + next_top_offset();
        let next_top = if cover_change {
            lerp(self.next_top_from, next_top_to, p)
        } else {
            next_top_to
        };
        let real_next_top = lerp(
            next_top + px_of(INTRO_NEXT_SLIDE),
            next_top,
            self.next_shown,
        );

        let mut root = div()
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(color(cx, ColorRef::Palette(ColorId::IntroBg)));

        let cover_top = if cover_change {
            let height = f32::from(px_of(INTRO_COVER_HEIGHT));
            Some(px(if has_cover {
                interpolate(-height, 0.0, p)
            } else {
                interpolate(0.0, -height, p)
            }))
        } else {
            has_cover.then_some(Pixels::ZERO)
        };
        if let Some(top) = cover_top {
            root = root.child(cover(g, top, cx));
        }

        root = match &self.departing {
            None => root.child(step_layer(
                g,
                self.header,
                self.step,
                self.error,
                Pixels::ZERO,
                1.0,
            )),
            Some(departing) if cover_change => {
                let was_top = g.content_top(departing.had_cover);
                root.children(header_labels(g, departing.header, was_top, 1.0 - p))
                    .children(header_labels(g, self.header, content_top, p))
                    .child(moving_body(
                        g,
                        departing.step,
                        p * f32::from(g.height - body_top(g, departing.had_cover)),
                        1.0 - p,
                    ))
                    .child(moving_body(
                        g,
                        self.step,
                        (1.0 - p) * f32::from(g.height - body_top(g, has_cover)),
                        p,
                    ))
            }
            Some(departing) => {
                let width = f32::from(px_of(INTRO_STEP_WIDTH));
                let sign = if departing.backwards { -1.0 } else { 1.0 };
                let was_x = -sign * width * Easing::EaseInCirc.apply(p);
                let now_x = sign * width * (1.0 - Easing::EaseOutCirc.apply(p));
                root.child(slide_clip(
                    g,
                    content_top,
                    step_layer(
                        g,
                        departing.header,
                        departing.step,
                        None,
                        px(was_x),
                        1.0 - Easing::EaseOutCirc.apply(p),
                    ),
                ))
                .child(slide_clip(
                    g,
                    content_top,
                    step_layer(
                        g,
                        self.header,
                        self.step,
                        None,
                        px(now_x),
                        Easing::EaseInCirc.apply(p),
                    ),
                ))
            }
        };

        let back_alpha = match &self.departing {
            Some(departing) if departing.header.has_back != self.header.has_back => {
                if self.header.has_back { p } else { 1.0 - p }
            }
            _ => {
                if self.header.has_back {
                    1.0
                } else {
                    0.0
                }
            }
        };
        if back_alpha > 0.0 {
            let intro = intro.clone();
            root = root.child(
                div()
                    .absolute()
                    .left_0()
                    .top(controls_top)
                    .opacity(back_alpha)
                    .child(IconButton::new("intro-back", &INTRO_BACK_BUTTON).on_click(
                        move |_, window, cx| {
                            intro.update(cx, |intro, cx| intro.back_clicked(window, cx))
                        },
                    )),
            );
        }

        if self.environment == Environment::Test && !has_cover {
            root = root.child(
                div()
                    .absolute()
                    .right(px_of(INTRO_SETTINGS_SKIP))
                    .top(controls_top + px_of(INTRO_SETTINGS_SKIP) * 2.0)
                    .child(
                        FlatLabel::new(
                            "intro-test-mode",
                            &DEFAULT_FLAT_LABEL,
                            Markup::plain("Test Mode"),
                        )
                        .color(ColorRef::Palette(ColorId::WindowSubTextFg)),
                    ),
            );
        }

        let (next_text, next_style) = match (&self.header.next, &self.departing) {
            (Some(text), _) => (Some(text.clone()), self.header.next_style),
            (None, Some(departing)) => (departing.header.next.clone(), departing.header.next_style),
            (None, None) => (None, self.header.next_style),
        };
        if self.next_shown > 0.0
            && let Some(text) = next_text
        {
            let intro = intro.clone();
            root = root.child(
                div()
                    .absolute()
                    .left((g.width - px_of(INTRO_NEXT_BUTTON.width)) / 2.0)
                    .top(real_next_top)
                    .opacity(self.next_shown)
                    .child(RoundButton::new("intro-next", next_style, text).on_click(
                        move |_, window, cx| {
                            intro.update(cx, |intro, cx| intro.submit_clicked(window, cx))
                        },
                    )),
            );
        }

        let next_height = px_of(INTRO_NEXT_BUTTON.height);
        if let Some(link) = self.language_link
            && !self.reset_button
            && !self.terms
            && self.next_shown >= 1.0
            && self.departing.is_none()
        {
            let link_height = font_height(DEFAULT_LINK_BUTTON.font);
            let pack = link.pack.clone();
            let session = intro.read(cx).shared().store.read(cx).session().clone();
            root = root.child(
                div()
                    .absolute()
                    .left_0()
                    .w(g.width)
                    .top(real_next_top + next_height + link_height)
                    .flex()
                    .justify_center()
                    .child(
                        LinkButton::new("intro-language", &DEFAULT_LINK_BUTTON, link.text.clone())
                            .on_click(move |_, _, cx| crate::lang::switch_to(&session, &pack, cx)),
                    ),
            );
        }

        if self.reset_button {
            let intro = intro.clone();
            root = root.child(
                div()
                    .absolute()
                    .left_0()
                    .w(g.width)
                    .bottom(px_of(INTRO_RESET_BOTTOM))
                    .flex()
                    .justify_center()
                    .child(
                        RoundButton::new(
                            "intro-reset",
                            &INTRO_RESET_BUTTON,
                            tr(cx, i18n::key::lng_signin_reset_account),
                        )
                        .uppercase()
                        .on_click(move |_, window, cx| {
                            let shared = intro.read(cx).shared().clone();
                            super::errors::reset_account(&shared, window, cx);
                        }),
                    ),
            );
        }

        if self.terms {
            let template = tr(cx, i18n::key::lng_terms_signup);
            let link = tr(cx, i18n::key::lng_terms_signup_link);
            let markup = match template.split_once("{link}") {
                Some((before, after)) => Markup::plain(before).link(&link).text(after),
                None => Markup::plain(template.to_string()),
            };
            let intro = intro.clone();
            root = root.child(
                div()
                    .absolute()
                    .left_0()
                    .w(g.width)
                    .bottom(px_of(INTRO_TERMS_BOTTOM))
                    .flex()
                    .justify_center()
                    .child(
                        FlatLabel::new("intro-terms", &INTRO_TERMS_LABEL, markup).on_link(
                            move |_, window, cx| {
                                let shared = intro.read(cx).shared().clone();
                                if let Some(terms) = intro.read(cx).terms(cx) {
                                    super::terms::show(&shared.layers, terms, None, window, cx);
                                }
                            },
                        ),
                    ),
            );
        }
        root.into_any_element()
    }
}

/// Where a step's own content starts, under its description
/// (`prepareContentSnapshot`).
fn body_top(g: Geometry, has_cover: bool) -> Pixels {
    let description_top = if has_cover {
        px_of(INTRO_COVER_DESCRIPTION_TOP)
    } else {
        px_of(INTRO_DESCRIPTION_TOP)
    };
    // shortcut: assumes two description lines; measure the label if a step's description is longer.
    g.content_top(has_cover) + description_top + font_height(INTRO_DESCRIPTION.style.font) * 2.0
}

fn slide_clip(g: Geometry, content_top: Pixels, layer: gpui::Div) -> gpui::Div {
    let left = ((g.width - px_of(INTRO_STEP_WIDTH)) / 2.0).floor();
    div()
        .absolute()
        .left(left)
        .top(content_top)
        .w(px_of(INTRO_STEP_WIDTH))
        .h(px_of(INTRO_STEP_HEIGHT))
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .left(-left)
                .top(-content_top)
                .w(g.width)
                .h(g.height)
                .child(layer),
        )
}

/// A step with its header, shifted by `dx`.
fn step_layer(
    g: Geometry,
    header: &Header,
    step: &Step,
    error: Option<(&StepError, f32)>,
    dx: Pixels,
    alpha: f32,
) -> gpui::Div {
    let content_top = g.content_top(header.has_cover);
    let mut layer = div()
        .absolute()
        .left(dx)
        .top_0()
        .w(g.width)
        .h(g.height)
        .opacity(alpha)
        .child(div().absolute().inset_0().child(step.view()))
        .children(header_labels(g, header, content_top, 1.0));
    if let Some((error, error_alpha)) = error {
        let top = content_top + header.error_top;
        layer = layer.child(if error.centered {
            div()
                .absolute()
                .left_0()
                .w(g.width)
                .top(top)
                .opacity(error_alpha)
                .child(
                    FlatLabel::new(
                        "intro-error",
                        &INTRO_ERROR_CENTERED,
                        Markup::plain(error.text.to_string()),
                    )
                    .width(g.width),
                )
        } else {
            div()
                .absolute()
                .left(g.content_left() + px_of(BUTTON_RADIUS))
                .top(top)
                .opacity(error_alpha)
                .child(
                    FlatLabel::new(
                        "intro-error",
                        &INTRO_ERROR,
                        Markup::plain(error.text.to_string()),
                    )
                    .width(px_of(INTRO_ERROR.min_width)),
                )
        });
    }
    layer
}

fn moving_body(g: Geometry, step: &Step, dy: f32, alpha: f32) -> gpui::Div {
    div()
        .absolute()
        .left_0()
        .top(px(dy))
        .w(g.width)
        .h(g.height)
        .opacity(alpha)
        .child(div().absolute().inset_0().child(step.view()))
}

fn header_labels(g: Geometry, header: &Header, content_top: Pixels, alpha: f32) -> Vec<gpui::Div> {
    if alpha <= 0.0 {
        return Vec::new();
    }
    let markup_title = Markup::plain(header.title.to_string());
    if header.has_cover {
        let centered = |top: Pixels| {
            div()
                .absolute()
                .left_0()
                .w(g.width)
                .top(top)
                .opacity(alpha)
                .flex()
                .justify_center()
        };
        vec![
            centered(content_top + px_of(INTRO_COVER_TITLE_TOP)).child(FlatLabel::new(
                "intro-title",
                &INTRO_COVER_TITLE,
                markup_title,
            )),
            centered(content_top + px_of(INTRO_COVER_DESCRIPTION_TOP)).child(FlatLabel::new(
                "intro-description",
                &INTRO_COVER_DESCRIPTION,
                header.description.clone(),
            )),
        ]
    } else {
        let left = g.content_left() + px_of(BUTTON_RADIUS);
        vec![
            div()
                .absolute()
                .left(left)
                .top(content_top + px_of(INTRO_TITLE_TOP))
                .opacity(alpha)
                .child(FlatLabel::new("intro-title", &INTRO_TITLE, markup_title)),
            div()
                .absolute()
                .left(left)
                .top(content_top + px_of(INTRO_DESCRIPTION_TOP))
                .opacity(alpha)
                .child(
                    FlatLabel::new(
                        "intro-description",
                        &INTRO_DESCRIPTION,
                        header.description.clone(),
                    )
                    .width(px_of(INTRO_DESCRIPTION.min_width)),
                ),
        ]
    }
}

/// The cover (`Step::paintCover`) with its top at `top`, negative while it
/// slides away. rgram draws its own mark where tdesktop draws Telegram's
/// plane, which is the official logo.
fn cover(g: Geometry, top: Pixels, cx: &App) -> gpui::Div {
    let cover_height = top + px_of(INTRO_COVER_HEIGHT);
    let left_size = icon_size(INTRO_COVER_LEFT);
    let right_size = icon_size(INTRO_COVER_RIGHT);
    let (mut left, mut right) = (Pixels::ZERO, Pixels::ZERO);
    if g.width < px_of(INTRO_COVER_MAX_WIDTH) {
        let max_skip = px_of(INTRO_COVER_MAX_WIDTH) - left_size.width - right_size.width;
        let min_skip = px_of(INTRO_COVER_ICONS_MIN_SKIP);
        let step_width = px_of(INTRO_STEP_WIDTH);
        let skip = min_skip
            + (max_skip - min_skip)
                * ((g.width - step_width) / (px_of(INTRO_COVER_MAX_WIDTH) - step_width));
        let outside = skip + left_size.width + right_size.width - g.width;
        left = (-outside / 2.0).floor();
        right = -outside - left;
    }
    if top < Pixels::ZERO {
        let shown = cover_height / px_of(INTRO_COVER_HEIGHT);
        let left_full = left + left_size.width;
        left = (left_full * shown).round() - left_size.width;
        let right_full = right + right_size.width;
        right = (right_full * shown).round() - right_size.width;
    }
    let palette = &cx.global::<ui::Theme>().palette;
    let gradient = linear_gradient(
        180.0,
        linear_color_stop(ui::hsla(palette.intro_cover_top_bg), 0.0),
        linear_color_stop(ui::hsla(palette.intro_cover_bottom_bg), 1.0),
    );
    let mark_size = px(76.0);
    div()
        .absolute()
        .inset_0()
        .child(
            div()
                .absolute()
                .left_0()
                .top(top)
                .w(g.width)
                .h(px_of(INTRO_COVER_HEIGHT))
                .bg(gradient),
        )
        .child(
            div()
                .absolute()
                .left(left)
                .top(cover_height - left_size.height)
                .child(ui::icon(INTRO_COVER_LEFT)),
        )
        .child(
            div()
                .absolute()
                .left(g.width - right - right_size.width)
                .top(cover_height - right_size.height)
                .child(ui::icon(INTRO_COVER_RIGHT)),
        )
        .child(
            div()
                .absolute()
                .left((g.width - mark_size) / 2.0)
                .top(top + px_of(INTRO_COVER_ICON_TOP))
                .size(mark_size)
                .rounded_full()
                .bg(ui::hsla(palette.intro_cover_plane_inner))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(44.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .font_family(ui::FONT_FAMILY)
                .text_color(ui::hsla(palette.intro_cover_top_bg))
                .child("r"),
        )
}
