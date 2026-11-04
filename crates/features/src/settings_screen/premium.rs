//! "Telegram Premium" (`Settings::Premium`): the cover, the subscription
//! periods, the features in the server's order and the subscribe button,
//! which stops at TDLib's payment form.
// shortcut: no feature preview box with its video (`ShowPremiumPreviewToBuy`), no sticker or mini stars on the cover, the button is not pinned below the list and has no glare; with M13.

use std::rc::Rc;

use gpui::{
    App, Context, InteractiveElement as _, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Task, Window, div, linear_color_stop, linear_gradient,
    px,
};
use i18n::{Key, key};
use model::premium::{Feature, PremiumOffer};
use style::layers::BOX_TITLE_FONT;
use style::premium::{DEFAULT_PREMIUM_COVER, PREMIUM_PREVIEW_BOX};
use style::settings::{
    SETTINGS_BUTTON, SETTINGS_ICON_RADIUS, SETTINGS_PREMIUM_ARROW, SETTINGS_PREMIUM_BUTTON_PADDING,
    SETTINGS_PREMIUM_ICON_DOUBLE, SETTINGS_PREMIUM_ICON_TAGS, SETTINGS_PREMIUM_OPTIONS_PADDING,
    SETTINGS_PREMIUM_ROW_ABOUT_PADDING, SETTINGS_PREMIUM_ROW_TITLE_PADDING,
    SETTINGS_PREMIUM_TOP_HEIGHT,
};
use style::settings_premium as icons;
use style::widgets::BOX_DIVIDER_LABEL;
use style::{Color, ColorId, ColorRef, Font, FontFlags, Icon, IconMask, Margins, MonoIcon};
use ui::{FlatLabel, Markup, Theme, color, font, font_height};

use super::premium_options::{self, OptionTexts};
use super::{Env, rows};
use crate::lang::{tr, tr_with};
use crate::px_of;

/// `Ui::Premium::ColorizedSvg`'s star, white on the cover.
const COVER_STAR: Icon = Icon::new(&[MonoIcon::new(
    IconMask::File {
        name: "settings/star",
        modifiers: &[],
    },
    ColorRef::Palette(ColorId::PremiumButtonFg),
    Margins::new(0, 0, 0, 0),
)]);
const COVER_FG: ColorRef = ColorRef::Palette(ColorId::PremiumButtonFg);
const FEATURE_TITLE_FONT: Font = Font::new(13, FontFlags::BOLD, "");
const FEATURE_TITLE_FG: ColorRef = ColorRef::Palette(ColorId::WindowBoldFg);
/// `FullHeightGradientStops`, down the feature icons.
const ICON_STOPS: [(f32, ColorId); 5] = [
    (0.0, ColorId::PremiumIconBg1),
    (0.28, ColorId::PremiumIconBg2),
    (0.55, ColorId::PremiumButtonBg2),
    (0.75, ColorId::PremiumButtonBg1),
    (1.0, ColorId::PremiumIconBg3),
];

/// `EntryMap`: each feature's icon, title and about.
fn entry(feature: Feature) -> (Icon, Key, Key) {
    use key::*;
    match feature {
        Feature::Stories => (
            icons::SETTINGS_PREMIUM_ICON_STORIES,
            lng_premium_summary_subtitle_stories,
            lng_premium_summary_about_stories,
        ),
        Feature::MoreUpload => (
            icons::SETTINGS_PREMIUM_ICON_FILES,
            lng_premium_summary_subtitle_more_upload,
            lng_premium_summary_about_more_upload,
        ),
        Feature::DoubleLimits => (
            SETTINGS_PREMIUM_ICON_DOUBLE,
            lng_premium_summary_subtitle_double_limits,
            lng_premium_summary_about_double_limits,
        ),
        Feature::LastSeen => (
            icons::SETTINGS_PREMIUM_ICON_LAST_SEEN,
            lng_premium_summary_subtitle_last_seen,
            lng_premium_summary_about_last_seen,
        ),
        Feature::VoiceToText => (
            icons::SETTINGS_PREMIUM_ICON_VOICE,
            lng_premium_summary_subtitle_voice_to_text,
            lng_premium_summary_about_voice_to_text,
        ),
        Feature::FasterDownload => (
            icons::SETTINGS_PREMIUM_ICON_SPEED,
            lng_premium_summary_subtitle_faster_download,
            lng_premium_summary_about_faster_download,
        ),
        Feature::Translations => (
            icons::SETTINGS_PREMIUM_ICON_TRANSLATIONS,
            lng_premium_summary_subtitle_translation,
            lng_premium_summary_about_translation,
        ),
        Feature::AnimatedEmoji => (
            icons::SETTINGS_ICON_EMOJI,
            lng_premium_summary_subtitle_animated_emoji,
            lng_premium_summary_about_animated_emoji,
        ),
        Feature::EmojiStatus => (
            icons::SETTINGS_PREMIUM_ICON_STATUS,
            lng_premium_summary_subtitle_emoji_status,
            lng_premium_summary_about_emoji_status,
        ),
        Feature::SavedTags => (
            SETTINGS_PREMIUM_ICON_TAGS,
            lng_premium_summary_subtitle_tags_for_messages,
            lng_premium_summary_about_tags_for_messages,
        ),
        Feature::PeerColors => (
            icons::SETTINGS_PREMIUM_ICON_PEER_COLORS,
            lng_premium_summary_subtitle_peer_colors,
            lng_premium_summary_about_peer_colors,
        ),
        Feature::Wallpapers => (
            icons::SETTINGS_PREMIUM_ICON_WALLPAPERS,
            lng_premium_summary_subtitle_wallpapers,
            lng_premium_summary_about_wallpapers,
        ),
        Feature::ProfileBadge => (
            icons::SETTINGS_PREMIUM_ICON_STAR,
            lng_premium_summary_subtitle_profile_badge,
            lng_premium_summary_about_profile_badge,
        ),
        Feature::MessagePrivacy => (
            icons::SETTINGS_PREMIUM_ICON_PRIVACY,
            lng_premium_summary_subtitle_message_privacy,
            lng_premium_summary_about_message_privacy,
        ),
        Feature::AdvancedChatManagement => (
            icons::SETTINGS_ICON_CHAT,
            lng_premium_summary_subtitle_advanced_chat_management,
            lng_premium_summary_about_advanced_chat_management,
        ),
        Feature::NoAds => (
            icons::SETTINGS_PREMIUM_ICON_CHANNELS_OFF,
            lng_premium_summary_subtitle_no_ads,
            lng_premium_summary_about_no_ads,
        ),
        Feature::InfiniteReactions => (
            icons::SETTINGS_PREMIUM_ICON_LIKE,
            lng_premium_summary_subtitle_infinite_reactions,
            lng_premium_summary_about_infinite_reactions,
        ),
        Feature::AnimatedUserpics => (
            icons::SETTINGS_PREMIUM_ICON_PLAY,
            lng_premium_summary_subtitle_animated_userpics,
            lng_premium_summary_about_animated_userpics,
        ),
        Feature::Stickers => (
            icons::SETTINGS_ICON_STICKERS,
            lng_premium_summary_subtitle_premium_stickers,
            lng_premium_summary_about_premium_stickers,
        ),
        Feature::Business => (
            icons::SETTINGS_PREMIUM_ICON_BUSINESS,
            lng_premium_summary_subtitle_business,
            lng_premium_summary_about_business,
        ),
        Feature::Effects => (
            icons::SETTINGS_PREMIUM_ICON_EFFECTS,
            lng_premium_summary_subtitle_effects,
            lng_premium_summary_about_effects,
        ),
        Feature::Checklists => (
            icons::SETTINGS_PREMIUM_ICON_CHECKLIST,
            lng_premium_summary_subtitle_todo_lists,
            lng_premium_summary_about_todo_lists,
        ),
        Feature::NoForwards => (
            icons::SETTINGS_PREMIUM_ICON_NO_FORWARDS,
            lng_premium_summary_subtitle_no_forwards,
            lng_premium_summary_about_no_forwards,
        ),
        Feature::AiCompose => (
            icons::SETTINGS_PREMIUM_ICON_AI_COMPOSE,
            lng_premium_summary_subtitle_ai_compose,
            lng_premium_summary_about_ai_compose,
        ),
        Feature::RichFormatting => (
            icons::SETTINGS_PREMIUM_ICON_RICH,
            lng_premium_summary_subtitle_rich_formatting,
            lng_premium_summary_about_rich_formatting,
        ),
    }
}

/// `anim::gradient_color_at` over `ICON_STOPS`.
fn icon_bg(share: f32, cx: &App) -> Color {
    let palette = &cx.global::<Theme>().palette;
    let at = |id: ColorId| ColorRef::Palette(id).resolve(palette);
    let share = share.clamp(0.0, 1.0);
    ICON_STOPS
        .windows(2)
        .find(|pair| share <= pair[1].0)
        .map_or(at(ICON_STOPS[4].1), |pair| {
            let (from, to) = (pair[0], pair[1]);
            ui::anim::mix(at(from.1), at(to.1), (share - from.0) / (to.0 - from.0))
        })
}

/// An offer shown instead of TDLib's, as the snapshots do.
struct PresetOffer(Rc<PremiumOffer>);

impl gpui::Global for PresetOffer {}

/// Shows `offer` on the Premium page instead of asking TDLib.
pub fn preset_premium_offer(offer: PremiumOffer, cx: &mut App) {
    cx.set_global(PresetOffer(Rc::new(offer)));
}

/// Asks TDLib for the offer, or takes the preset one, and hands it to
/// `set` on the page.
pub(super) fn load_offer<T: 'static>(
    env: &Env,
    cx: &mut Context<T>,
    set: impl FnOnce(&mut T, Rc<PremiumOffer>) + 'static,
) -> Task<()> {
    if let Some(preset) = cx.try_global::<PresetOffer>() {
        let offer = preset.0.clone();
        return cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |this, cx| {
                set(this, offer);
                cx.notify();
            });
        });
    }
    let task = crate::store::spawn(env.targets.store.read(cx).session().premium_offer(), cx);
    cx.spawn(async move |this, cx| match task.await {
        Ok(offer) => {
            let _ = this.update(cx, |this, cx| {
                set(this, Rc::new(offer));
                cx.notify();
            });
        }
        Err(error) => tracing::warn!(%error, "reading the Premium offer failed"),
    })
}

/// Whether the signed-in account has Premium.
pub(super) fn is_premium(env: &Env, cx: &App) -> bool {
    let state = env.targets.store.read(cx).state();
    state
        .me
        .and_then(|me| state.chats.user(me))
        .is_some_and(|user| user.is_premium)
}

/// The gradient cover (`Ui::Premium::TopBar`) with `icon` over the title.
pub(super) fn cover(
    icon: Icon,
    title: SharedString,
    about: Markup,
    cx: &App,
) -> impl IntoElement + use<> {
    let st = &DEFAULT_PREMIUM_COVER;
    let palette = &cx.global::<Theme>().palette;
    let stop = |id: ColorId, at: f32| {
        linear_color_stop(ui::hsla(ColorRef::Palette(id).resolve(palette)), at)
    };
    div()
        .min_h(px_of(SETTINGS_PREMIUM_TOP_HEIGHT))
        .pb(px_of(st.title_padding.bottom))
        .flex()
        .flex_col()
        .items_center()
        .bg(linear_gradient(
            135.0,
            stop(ColorId::PremiumButtonBg1, 0.0),
            stop(ColorId::PremiumButtonBg2, 1.0),
        ))
        .child(div().h(px_of(st.star_top_skip)))
        .child(ui::icon(icon).side(px_of(st.star_size.width)))
        .child(
            div()
                .pt(px_of(st.title_padding.top))
                .pb(px_of(st.title_padding.bottom))
                .font(font(BOX_TITLE_FONT))
                .text_size(px_of(BOX_TITLE_FONT.size))
                .line_height(font_height(BOX_TITLE_FONT))
                .text_color(color(cx, COVER_FG))
                .child(title),
        )
        .child(
            FlatLabel::new("premium-about", &st.about, about)
                .width(px_of(st.about.min_width) * 1.6),
        )
}

/// Where row `index` of `count` sits down the icons' gradient.
pub(super) fn share(index: usize, count: usize) -> f32 {
    #[expect(clippy::cast_precision_loss, reason = "a few dozen rows")]
    let share = index as f32 / count.saturating_sub(1).max(1) as f32;
    share
}

/// A feature with its gradient icon, bold title and gray about
/// (`AddSummaryPremium`).
pub(super) fn feature_row(
    share: f32,
    (icon, title, about): (Icon, Key, Key),
    cx: &App,
) -> impl IntoElement + use<> {
    let (title_padding, about_padding) = (
        SETTINGS_PREMIUM_ROW_TITLE_PADDING,
        SETTINGS_PREMIUM_ROW_ABOUT_PADDING,
    );
    let size = ui::icon_size(icon);
    div()
        .relative()
        .child(
            div()
                .absolute()
                .left(px_of(SETTINGS_BUTTON.icon_left))
                .top(px_of(title_padding.top))
                .size(size.width)
                .rounded(px_of(SETTINGS_ICON_RADIUS))
                .bg(ui::hsla(icon_bg(share, cx)))
                .child(ui::icon(icon)),
        )
        .child(
            div()
                .absolute()
                .right(px(5.0))
                .top(px_of(title_padding.top) + px(9.0))
                .child(ui::icon(SETTINGS_PREMIUM_ARROW)),
        )
        .child(
            div()
                .pl(px_of(title_padding.left))
                .pr(px_of(title_padding.right))
                .pt(px_of(title_padding.top))
                .pb(px_of(title_padding.bottom))
                .font(font(FEATURE_TITLE_FONT))
                .text_size(px_of(FEATURE_TITLE_FONT.size))
                .line_height(font_height(FEATURE_TITLE_FONT))
                .text_color(color(cx, FEATURE_TITLE_FG))
                .child(tr(cx, title)),
        )
        .child(
            rows::text(tr(cx, about), &BOX_DIVIDER_LABEL, cx)
                .pl(px_of(about_padding.left))
                .pr(px_of(about_padding.right))
                .pb(px_of(about_padding.bottom)),
        )
}

/// The gradient "Subscribe for…" button (`CreateSubscribeButton`).
pub(super) fn subscribe_button<F: Fn(&mut Window, &mut App) + 'static>(
    text: String,
    on_click: F,
    cx: &App,
) -> impl IntoElement + use<F> {
    let st = &PREMIUM_PREVIEW_BOX.button;
    let palette = &cx.global::<Theme>().palette;
    let stop = |id: ColorId, at: f32| {
        linear_color_stop(ui::hsla(ColorRef::Palette(id).resolve(palette)), at)
    };
    let padding = SETTINGS_PREMIUM_BUTTON_PADDING;
    div()
        .pl(px_of(padding.left))
        .pr(px_of(padding.right))
        .pt(px_of(padding.top))
        .pb(px_of(padding.bottom))
        .child(
            div()
                .id("premium-subscribe")
                .h(px_of(st.height))
                .pt(px_of(st.text_top))
                .rounded(px_of(style::widgets::BUTTON_RADIUS))
                .bg(linear_gradient(
                    90.0,
                    stop(ColorId::PremiumButtonBg1, 0.0),
                    stop(ColorId::PremiumButtonBg3, 1.0),
                ))
                .cursor_pointer()
                .flex()
                .justify_center()
                .font(font(st.style.font))
                .text_size(px_of(st.style.font.size))
                .line_height(font_height(st.style.font))
                .text_color(color(cx, COVER_FG))
                .child(text)
                .on_click(move |_, window, cx| on_click(window, cx)),
        )
}

pub(super) struct PremiumPage {
    env: Env,
    /// None while TDLib is asked.
    offer: Option<Rc<PremiumOffer>>,
    chosen: usize,
    _load: Task<()>,
}

impl PremiumPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let load = load_offer(&env, cx, |this: &mut Self, offer| this.offer = Some(offer));
        Self {
            env,
            offer: None,
            chosen: 0,
            _load: load,
        }
    }

    fn subscribe(&self, window: &mut Window, cx: &mut App) {
        let Some(option) = self
            .offer
            .as_ref()
            .and_then(|offer| offer.options.get(self.chosen))
        else {
            return;
        };
        crate::payments::open(option.link.clone(), &self.env.targets, window, cx);
    }

    fn cover(&self, is_premium: bool, cx: &App) -> impl IntoElement + use<> {
        let (title, about) = match self.offer.as_ref().filter(|_| is_premium) {
            Some(offer) => (
                tr(cx, key::lng_premium_summary_title_subscribed),
                crate::intro::terms::markup_of(&offer.status).0,
            ),
            None => (
                tr(cx, key::lng_premium_summary_title),
                Markup::rich(&tr(cx, key::lng_premium_summary_top_about)),
            ),
        };
        cover(COVER_STAR, title, about, cx)
    }

    fn options(&self, offer: &PremiumOffer, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let texts = premium_options::texts(&offer.options, cx);
        let rows = texts.iter().enumerate().map(|(index, texts)| {
            let weak = cx.weak_entity();
            premium_options::row(
                index,
                texts,
                index == self.chosen,
                move |cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.chosen = index;
                        cx.notify();
                    });
                },
                cx,
            )
            .into_any_element()
        });
        let padding = SETTINGS_PREMIUM_OPTIONS_PADDING;
        std::iter::once(rows::skip_by(padding.top))
            .chain(rows)
            .chain([
                rows::skip_by(padding.bottom),
                rows::divider(cx),
                rows::skip(),
            ])
            .collect()
    }

    fn subscribe_text(&self, texts: &OptionTexts, cx: &App) -> String {
        tr_with(
            cx,
            key::lng_premium_subscribe_button,
            &[("cost", &texts.per_month)],
        )
    }
}

impl Render for PremiumPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_premium = is_premium(&self.env, cx);
        let mut page = div().flex().flex_col().child(self.cover(is_premium, cx));
        let Some(offer) = self.offer.clone() else {
            return page;
        };
        if !is_premium && offer.options.len() > 1 {
            page = page.children(self.options(&offer, cx));
        } else {
            page = page.child(rows::skip());
        }
        let count = offer.features.len();
        let features: Vec<_> = offer
            .features
            .iter()
            .enumerate()
            .map(|(index, feature)| feature_row(share(index, count), entry(*feature), cx))
            .collect();
        page = page.children(features).child(rows::skip());
        let chosen = premium_options::texts(&offer.options, cx)
            .into_iter()
            .nth(self.chosen);
        if let Some(texts) = chosen.filter(|_| !is_premium) {
            let weak = cx.weak_entity();
            page = page.child(subscribe_button(
                self.subscribe_text(&texts, cx),
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
    fn icon_stops_rise_from_zero_to_one() {
        assert!(ICON_STOPS.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!((ICON_STOPS[0].0, ICON_STOPS[4].0) == (0.0, 1.0));
    }
}
