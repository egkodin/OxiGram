//! "Telegram Business" (`Settings::Business`): the cover, the features in
//! the server's order, the switch for sponsored messages that Premium
//! accounts get and, without Premium, the subscribe button, which stops at
//! TDLib's payment form.
// shortcut: feature rows open nothing yet (tdesktop opens the preview box without Premium, the away/greeting/hours/location/replies/bots/intro/links pages with it); with M13.

use std::rc::Rc;

use gpui::{App, Context, IntoElement, ParentElement, Render, Styled, Task, Window, div};
use i18n::{Key, key};
use model::business::{Feature, SponsoredMessages};
use model::premium::PremiumOffer;
use style::settings::{
    SETTINGS_BUSINESS_ICON_AWAY, SETTINGS_BUSINESS_ICON_CHAT_INTRO,
    SETTINGS_BUSINESS_ICON_CHAT_LINKS, SETTINGS_BUSINESS_ICON_CHATBOTS,
    SETTINGS_BUSINESS_ICON_GREETING, SETTINGS_BUSINESS_ICON_HOURS, SETTINGS_BUSINESS_ICON_LOCATION,
    SETTINGS_BUSINESS_ICON_REPLIES, SETTINGS_FROM_FILE_TOP, SETTINGS_PREMIUM_ICON_TAGS,
};
use style::settings_premium::SETTINGS_PREMIUM_ICON_BUSINESS;
use style::widgets::{BOX_DIVIDER_LABEL, DEFAULT_BOX_DIVIDER_LABEL_PADDING};
use style::{ColorId, ColorRef, Icon};
use ui::{FlatLabel, Markup};

use super::premium;
use super::premium_options;
use super::{Env, rows};
use crate::lang::{tr, tr_with};
use crate::px_of;

/// `textMoreIconEmoji` after the link.
const MORE: &str = "\u{203A}";

/// `EntryMap`: each feature's icon, title and about.
fn entry(feature: Feature) -> (Icon, Key, Key) {
    use key::*;
    match feature {
        Feature::Location => (
            SETTINGS_BUSINESS_ICON_LOCATION,
            lng_business_subtitle_location,
            lng_business_about_location,
        ),
        Feature::Hours => (
            SETTINGS_BUSINESS_ICON_HOURS,
            lng_business_subtitle_opening_hours,
            lng_business_about_opening_hours,
        ),
        Feature::QuickReplies => (
            SETTINGS_BUSINESS_ICON_REPLIES,
            lng_business_subtitle_quick_replies,
            lng_business_about_quick_replies,
        ),
        Feature::Greeting => (
            SETTINGS_BUSINESS_ICON_GREETING,
            lng_business_subtitle_greeting_messages,
            lng_business_about_greeting_messages,
        ),
        Feature::Away => (
            SETTINGS_BUSINESS_ICON_AWAY,
            lng_business_subtitle_away_messages,
            lng_business_about_away_messages,
        ),
        Feature::Bots => (
            SETTINGS_BUSINESS_ICON_CHATBOTS,
            lng_business_subtitle_chatbots,
            lng_business_about_chatbots,
        ),
        Feature::Intro => (
            SETTINGS_BUSINESS_ICON_CHAT_INTRO,
            lng_business_subtitle_chat_intro,
            lng_business_about_chat_intro,
        ),
        Feature::Links => (
            SETTINGS_BUSINESS_ICON_CHAT_LINKS,
            lng_business_subtitle_chat_links,
            lng_business_about_chat_links,
        ),
        Feature::FolderTags => (
            SETTINGS_PREMIUM_ICON_TAGS,
            lng_premium_summary_subtitle_filter_tags,
            lng_premium_summary_about_filter_tags,
        ),
    }
}

/// `text` with its `{link}` made a link reading `link`.
fn with_link(text: &str, link: &str) -> Markup {
    match text.split_once("{link}") {
        Some((before, after)) => Markup::plain(before).link(link).text(after),
        None => Markup::plain(text),
    }
}

pub(super) struct BusinessPage {
    env: Env,
    features: Vec<Feature>,
    offer: Option<Rc<PremiumOffer>>,
    /// None while TDLib is asked, and without Premium.
    sponsored: Option<SponsoredMessages>,
    _features: Task<()>,
    _offer: Task<()>,
    _sponsored: Task<()>,
}

impl BusinessPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let session = env.targets.store.read(cx).session();
        let features = crate::store::spawn(session.business_features(), cx);
        let features = cx.spawn(async move |this, cx| match features.await {
            Ok(features) => {
                let _ = this.update(cx, |this, cx| {
                    this.features = features;
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "reading the Business features failed"),
        });
        let offer = premium::load_offer(&env, cx, |this: &mut Self, offer| {
            this.offer = Some(offer);
        });
        let mut page = Self {
            env,
            features: Feature::FALLBACK.to_vec(),
            offer: None,
            sponsored: None,
            _features: features,
            _offer: offer,
            _sponsored: Task::ready(()),
        };
        page.load_sponsored(cx);
        page
    }

    fn load_sponsored(&mut self, cx: &mut Context<Self>) {
        let Some(me) = self.env.targets.store.read(cx).state().me else {
            return;
        };
        if !premium::is_premium(&self.env, cx) {
            return;
        }
        let task = crate::store::spawn(
            self.env
                .targets
                .store
                .read(cx)
                .session()
                .sponsored_setting(me),
            cx,
        );
        self._sponsored = cx.spawn(async move |this, cx| match task.await {
            Ok(value) => {
                let _ = this.update(cx, |this, cx| {
                    this.sponsored = Some(value);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "reading the sponsored messages switch failed"),
        });
    }

    fn set_sponsored(&mut self, value: SponsoredMessages, cx: &mut Context<Self>) {
        let previous = self.sponsored.replace(value);
        let task = crate::store::spawn(
            self.env
                .targets
                .store
                .read(cx)
                .session()
                .set_sponsored_setting(value),
            cx,
        );
        let targets = self.env.targets.clone();
        self._sponsored = cx.spawn(async move |this, cx| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "changing the sponsored messages switch failed");
                let _ = this.update(cx, |this, cx| {
                    this.sponsored = previous;
                    crate::payments::toast(&targets, error.user_message(), cx);
                    cx.notify();
                });
            }
        });
        cx.notify();
    }

    /// `BuildSponsoredSection`.
    fn sponsored_section(
        &self,
        value: SponsoredMessages,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let weak = cx.weak_entity();
        let is_on = value == SponsoredMessages::Shown;
        let toggle = rows::plain(
            "business-sponsored",
            tr(cx, key::lng_business_button_sponsored),
        )
        .toggle(is_on)
        .on_click(move |_, _, cx| {
            let next = if is_on {
                SponsoredMessages::Hidden
            } else {
                SponsoredMessages::Shown
            };
            let _ = weak.update(cx, |this, cx| this.set_sponsored(next, cx));
        });
        let link = tr_with(
            cx,
            key::lng_business_about_sponsored_link,
            &[("emoji", MORE)],
        );
        let about = with_link(&tr(cx, key::lng_business_about_sponsored), &link);
        let url = tr(cx, key::lng_business_about_sponsored_url);
        let padding = DEFAULT_BOX_DIVIDER_LABEL_PADDING;
        let about = div()
            .bg(ui::color(cx, ColorRef::Palette(ColorId::BoxDividerBg)))
            .pl(px_of(padding.left))
            .pr(px_of(padding.right))
            .pt(px_of(padding.top))
            .pb(px_of(padding.bottom))
            .child(
                FlatLabel::new("business-sponsored-about", &BOX_DIVIDER_LABEL, about)
                    .on_link(move |_, _, cx| cx.open_url(&url)),
            );
        vec![
            rows::divider(cx),
            rows::skip(),
            rows::subtitle(tr(cx, key::lng_business_subtitle_sponsored), cx),
            toggle.into_any_element(),
            rows::skip(),
            about.into_any_element(),
        ]
    }

    fn subscribe(&self, window: &mut Window, cx: &mut App) {
        let Some(option) = self.offer.as_ref().and_then(|offer| offer.options.first()) else {
            return;
        };
        crate::payments::open(option.link.clone(), &self.env.targets, window, cx);
    }
}

impl Render for BusinessPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_premium = premium::is_premium(&self.env, cx);
        let about = if is_premium {
            key::lng_business_unlocked
        } else {
            key::lng_business_about
        };
        let count = self.features.len();
        let features: Vec<_> = self
            .features
            .iter()
            .enumerate()
            .map(|(index, feature)| {
                premium::feature_row(premium::share(index, count), entry(*feature), cx)
            })
            .collect();
        let mut page = div()
            .flex()
            .flex_col()
            .child(premium::cover(
                SETTINGS_PREMIUM_ICON_BUSINESS,
                tr(cx, key::lng_business_title),
                Markup::rich(&tr(cx, about)),
                cx,
            ))
            .child(rows::skip_by(SETTINGS_FROM_FILE_TOP))
            .children(features)
            .child(rows::skip());
        if let Some(value) = self.sponsored.filter(|_| is_premium) {
            page = page.children(self.sponsored_section(value, cx));
        }
        let first = self.offer.as_ref().and_then(|offer| {
            premium_options::texts(&offer.options, cx)
                .into_iter()
                .next()
        });
        if let Some(texts) = first.filter(|_| !is_premium) {
            let weak = cx.weak_entity();
            let text = tr_with(
                cx,
                key::lng_premium_subscribe_button,
                &[("cost", &texts.per_month)],
            );
            page = page.child(premium::subscribe_button(
                text,
                move |window, cx| {
                    let _ = weak.update(cx, |this, cx| this.subscribe(window, cx));
                },
                cx,
            ));
        }
        page
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_link_takes_its_place_in_the_text() {
        let markup = with_link("Ads on the {link}.", "platform");
        assert_eq!(markup.as_str(), "Ads on the platform.");
        assert_eq!(with_link("No link", "x").as_str(), "No link");
    }
}
