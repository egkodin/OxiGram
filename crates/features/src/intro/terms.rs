//! `Window::TermsBox` as the intro shows it (`Widget::showTerms`): the terms
//! of service, an age confirmation, and agree or decline.

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, Styled, WeakEntity, Window, actions, div, prelude::*, px,
};
use i18n::key;
use model::auth;
use model::text::{EntityKind, FormattedText};
use style::layers::{BOX_WIDE_WIDTH, DEFAULT_BOX};
use style::widgets::{DEFAULT_CHECK, DEFAULT_CHECKBOX, DEFAULT_FLAT_LABEL};
use ui::{BoxFrame, Checkbox, Checked, FlatLabel, Markup, RoundButton};

use crate::boxes::{self, Callback, ConfirmArgs};
use crate::lang::{tr, tr_plural};
use crate::layers::{Dismiss, Layers};
use crate::px_of;

actions!(terms_box, [Agree]);

const CONTEXT: &str = "TermsBox";

/// `termsPadding` and `termsAgePadding` (window_lock_widgets.style, which
/// style-gen does not read).
const PADDING: (f32, f32, f32, f32) = (22.0, 4.0, 16.0, 16.0);
const AGE_PADDING: (f32, f32) = (22.0, 16.0);
/// The share of the window the text may take before it scrolls.
// shortcut: tdesktop sizes boxes to the window minus `boxVerticalMargin`; this is close enough until M2 adds the layer scroll.
const MAX_TEXT_SHARE: f32 = 0.6;

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([gpui::KeyBinding::new("enter", Agree, Some(CONTEXT))]);
}

/// The buttons the box shows.
enum Mode {
    /// Agree and decline, each closing the box first.
    Accept {
        accepted: Callback,
        declined: Callback,
    },
    /// Reading the terms from the intro link: one OK button.
    Read,
}

struct TermsBox {
    layers: WeakEntity<Layers>,
    terms: auth::Terms,
    text: Markup,
    urls: Vec<String>,
    mode: Mode,
    age: Checked,
    is_age_error: bool,
    focus: FocusHandle,
}

impl std::fmt::Debug for TermsBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TermsBox")
            .field("age", &self.age)
            .finish_non_exhaustive()
    }
}

impl Focusable for TermsBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Shows `terms` on sign-up (`Widget::showTerms`); with `accepted`, the user
/// must agree to go on, and declining asks again.
pub(crate) fn show(
    layers: &Entity<Layers>,
    terms: auth::Terms,
    accepted: Option<Callback>,
    window: &mut Window,
    cx: &mut App,
) {
    let mode = match accepted {
        Some(accepted) => {
            let declined = signup_declined(layers, terms.clone(), accepted.clone());
            Mode::Accept { accepted, declined }
        }
        None => Mode::Read,
    };
    open(layers, terms, mode, window, cx);
}

/// Shows updated terms the signed-in user must accept or decline
/// (`Controller::checkLockByTerms`).
pub(crate) fn show_update(
    layers: &Entity<Layers>,
    terms: auth::Terms,
    accepted: Callback,
    declined: Callback,
    window: &mut Window,
    cx: &mut App,
) {
    open(
        layers,
        terms,
        Mode::Accept { accepted, declined },
        window,
        cx,
    );
}

/// `lng_terms_signup_sorry`, from which the terms open again.
fn signup_declined(layers: &Entity<Layers>, terms: auth::Terms, accepted: Callback) -> Callback {
    let layers = layers.clone();
    Rc::new(move |window, cx| {
        let again = (layers.clone(), terms.clone(), accepted.clone());
        let mut args = ConfirmArgs::new(Markup::plain(
            tr(cx, key::lng_terms_signup_sorry).to_string(),
        ));
        args.title = Some(tr(cx, key::lng_terms_header));
        args.confirm_text = Some(tr(cx, key::lng_intro_finish));
        args.confirmed = Some(Rc::new(move |window, cx| {
            show(&again.0, again.1.clone(), Some(again.2.clone()), window, cx);
        }));
        args.cancel_text = Some(tr(cx, key::lng_terms_decline));
        boxes::confirm(&layers, args, window, cx);
    })
}

fn open(
    layers: &Entity<Layers>,
    terms: auth::Terms,
    mode: Mode,
    window: &mut Window,
    cx: &mut App,
) {
    let (text, urls) = markup_of(&terms.text);
    let weak = layers.downgrade();
    let view = cx.new(|cx| TermsBox {
        layers: weak,
        terms,
        text,
        urls,
        mode,
        age: Checked::No,
        is_age_error: false,
        focus: cx.focus_handle(),
    });
    layers.update(cx, |layers, cx| {
        layers.show(view, Dismiss::ButtonsOnly, None, window, cx)
    });
}

/// The text with bold spans and links, and each link's URL in order.
// shortcut: italics, code and mentions render plain; tdesktop's mention toast comes with M10 bots.
pub(crate) fn markup_of(text: &FormattedText) -> (Markup, Vec<String>) {
    let mut entities: Vec<_> = text
        .entities
        .iter()
        .filter(|entity| {
            matches!(
                entity.kind,
                EntityKind::Bold | EntityKind::Url | EntityKind::TextUrl { .. }
            )
        })
        .collect();
    entities.sort_by_key(|entity| entity.range.start);
    let mut markup = Markup::default();
    let mut urls = Vec::new();
    let mut end = 0;
    for entity in entities {
        let range = entity.range.clone();
        if range.start < end || range.end > text.text.len() {
            continue;
        }
        markup = markup.text(&text.text[end..range.start]);
        let part = &text.text[range.clone()];
        markup = match &entity.kind {
            EntityKind::Bold => markup.bold(part),
            EntityKind::TextUrl { url } => {
                urls.push(url.clone());
                markup.link(part)
            }
            _ => {
                urls.push(part.to_owned());
                markup.link(part)
            }
        };
        end = range.end;
    }
    (markup.text(&text.text[end..]), urls)
}

impl TermsBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
    }

    fn agree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.terms.min_age > 0 && self.age == Checked::No {
            self.is_age_error = true;
            cx.notify();
            return;
        }
        self.close(window, cx);
        if let Mode::Accept { accepted, .. } = &self.mode {
            accepted(window, cx);
        }
    }

    fn decline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Mode::Accept { declined, .. } = &self.mode {
            let declined = declined.clone();
            declined(window, cx);
        }
    }
}

impl Render for TermsBox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = px_of(BOX_WIDE_WIDTH);
        let (left, top, right, bottom) = PADDING;
        let urls = self.urls.clone();
        let label = FlatLabel::new("terms-text", &DEFAULT_FLAT_LABEL, self.text.clone())
            .width(width - px(left) - px(right))
            .on_link(move |index, _, cx| {
                if let Some(url) = urls.get(index) {
                    cx.open_url(url);
                }
            });
        let text = div()
            .id("terms-scroll")
            .max_h(window.viewport_size().height * MAX_TEXT_SHARE)
            .overflow_y_scroll()
            .pl(px(left))
            .pt(px(top))
            .pr(px(right))
            .pb(px(bottom))
            .child(label);
        let entity = cx.entity();
        let age = (self.terms.min_age > 0).then(|| {
            let entity = entity.clone();
            let (left, top) = AGE_PADDING;
            let text = tr_plural(cx, key::lng_terms_age, i64::from(self.terms.min_age), &[]);
            let error = self
                .is_age_error
                .then_some(style::ColorRef::Palette(style::ColorId::BoxTextFgError));
            div().pl(px(left)).pt(px(top)).child(
                Checkbox::new(
                    "terms-age",
                    &DEFAULT_CHECKBOX,
                    &DEFAULT_CHECK,
                    text,
                    self.age,
                )
                .untoggled_override(error)
                .on_toggle(move |_, cx| {
                    entity.update(cx, |this, cx| {
                        this.age = if this.age == Checked::Yes {
                            Checked::No
                        } else {
                            Checked::Yes
                        };
                        this.is_age_error = false;
                        cx.notify();
                    });
                }),
            )
        });
        let mut frame = BoxFrame::new("terms-box", &DEFAULT_BOX)
            .width(width)
            .title(tr(cx, key::lng_terms_header))
            .row(div().flex().flex_col().child(text).children(age));
        let agree_text = match self.mode {
            Mode::Accept { .. } => tr(cx, key::lng_terms_agree),
            Mode::Read => tr(cx, key::lng_box_ok),
        };
        let agree_entity = entity.clone();
        frame = frame.button(
            RoundButton::new("terms-agree", &DEFAULT_BOX.button, agree_text).on_click(
                move |_, window, cx| agree_entity.update(cx, |this, cx| this.agree(window, cx)),
            ),
        );
        if matches!(self.mode, Mode::Accept { .. }) {
            frame = frame.button(
                RoundButton::new(
                    "terms-decline",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_terms_decline),
                )
                .on_click(move |_, window, cx| {
                    entity.update(cx, |this, cx| this.decline(window, cx))
                }),
            );
        }
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Agree, window, cx| this.agree(window, cx)))
            .child(frame)
    }
}

#[cfg(test)]
mod tests {
    use model::text::Entity;

    use super::*;

    #[test]
    fn markup_keeps_bold_and_links_in_order() {
        let text = FormattedText {
            text: "Read the terms at telegram.org now".to_owned(),
            entities: vec![
                Entity {
                    range: 18..30,
                    kind: EntityKind::Url,
                },
                Entity {
                    range: 9..14,
                    kind: EntityKind::Bold,
                },
                Entity {
                    range: 0..4,
                    kind: EntityKind::TextUrl {
                        url: "https://example.org".to_owned(),
                    },
                },
            ],
        };
        let (markup, urls) = markup_of(&text);
        assert_eq!(markup.as_str(), text.text);
        assert_eq!(urls, ["https://example.org", "telegram.org"]);
    }
}
