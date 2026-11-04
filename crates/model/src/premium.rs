//! Telegram Premium's offer (`Api::Premium`): the subscription's state,
//! the ways to buy it and the features in the server's order.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{
    InputInvoice, InternalLinkType, PremiumFeature, PremiumSource, TelegramPaymentPurpose,
};
use td_api::functions::{
    GetPaymentForm, GetPremiumFeatures, GetPremiumGiftPaymentOptions, GetPremiumState,
};
use td_api::types as td;

use crate::ids::UserId;
use crate::session::{RequestError, Session};
use crate::text::FormattedText;

/// A feature tdesktop lists on the Premium page (`PremiumFeature`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    Stories,
    MoreUpload,
    DoubleLimits,
    LastSeen,
    VoiceToText,
    FasterDownload,
    Translations,
    AnimatedEmoji,
    EmojiStatus,
    SavedTags,
    PeerColors,
    Wallpapers,
    ProfileBadge,
    MessagePrivacy,
    AdvancedChatManagement,
    NoAds,
    InfiniteReactions,
    AnimatedUserpics,
    Stickers,
    Business,
    Effects,
    Checklists,
    NoForwards,
    AiCompose,
    RichFormatting,
}

impl Feature {
    /// None for what tdesktop leaves off the page (topic icons, app icons,
    /// boosts, paid messages).
    fn of(feature: &PremiumFeature) -> Option<Self> {
        Some(match feature {
            PremiumFeature::UpgradedStories(_) => Self::Stories,
            PremiumFeature::IncreasedUploadFileSize(_) => Self::MoreUpload,
            PremiumFeature::IncreasedLimits(_) => Self::DoubleLimits,
            PremiumFeature::LastSeenTimes(_) => Self::LastSeen,
            PremiumFeature::VoiceRecognition(_) => Self::VoiceToText,
            PremiumFeature::ImprovedDownloadSpeed(_) => Self::FasterDownload,
            PremiumFeature::RealTimeChatTranslation(_) => Self::Translations,
            PremiumFeature::CustomEmoji(_) => Self::AnimatedEmoji,
            PremiumFeature::EmojiStatus(_) => Self::EmojiStatus,
            PremiumFeature::SavedMessagesTags(_) => Self::SavedTags,
            PremiumFeature::AccentColor(_) => Self::PeerColors,
            PremiumFeature::BackgroundForBoth(_) => Self::Wallpapers,
            PremiumFeature::ProfileBadge(_) => Self::ProfileBadge,
            PremiumFeature::MessagePrivacy(_) => Self::MessagePrivacy,
            PremiumFeature::AdvancedChatManagement(_) => Self::AdvancedChatManagement,
            PremiumFeature::DisabledAds(_) => Self::NoAds,
            PremiumFeature::UniqueReactions(_) => Self::InfiniteReactions,
            PremiumFeature::AnimatedProfilePhoto(_) => Self::AnimatedUserpics,
            PremiumFeature::UniqueStickers(_) => Self::Stickers,
            PremiumFeature::Business(_) => Self::Business,
            PremiumFeature::MessageEffects(_) => Self::Effects,
            PremiumFeature::Checklists(_) => Self::Checklists,
            PremiumFeature::ProtectPrivateChatContent(_) => Self::NoForwards,
            PremiumFeature::TextComposition(_) => Self::AiCompose,
            PremiumFeature::RichMessages(_) => Self::RichFormatting,
            _ => return None,
        })
    }
}

/// Where a purchase goes on: an invoice TDLib can open, a Stars package,
/// or a link (a bot start) rgram does not follow, as it would message the
/// bot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentLink {
    Invoice(String),
    Stars {
        currency: String,
        amount: i64,
        star_count: i64,
    },
    /// Premium for `user` (`Api::PremiumGiftCodeOptions`).
    PremiumGift {
        currency: String,
        amount: i64,
        user: UserId,
        months: i32,
    },
    Other,
}

/// A Premium period to gift (`PremiumGiftPaymentOption`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftOption {
    pub currency: String,
    /// In the currency's minor units.
    pub amount: i64,
    pub months: i32,
    pub discount_percent: i32,
}

impl GiftOption {
    #[must_use]
    pub fn link(&self, user: UserId) -> PaymentLink {
        PaymentLink::PremiumGift {
            currency: self.currency.clone(),
            amount: self.amount,
            user,
            months: self.months,
        }
    }
}

impl PaymentLink {
    fn of(link: Option<&InternalLinkType>) -> Self {
        match link {
            Some(InternalLinkType::Invoice(invoice)) => Self::Invoice(invoice.invoice_name.clone()),
            _ => Self::Other,
        }
    }
}

/// One subscription period (`PremiumSubscriptionOption`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionOption {
    pub currency: String,
    /// In the currency's minor units, for the whole period.
    pub amount: i64,
    pub months: i32,
    pub link: PaymentLink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PremiumOffer {
    /// "Your subscription renews on…"; empty without Premium.
    pub status: FormattedText,
    pub options: Vec<SubscriptionOption>,
    pub features: Vec<Feature>,
}

impl Session {
    /// The Premium page's data (`Api::Premium::reload` and
    /// `premium_promo_order`).
    pub fn premium_offer(
        &self,
    ) -> impl Future<Output = Result<PremiumOffer, RequestError>> + Send + use<> {
        let state = self.client().request(&GetPremiumState {});
        let features = self.client().request(&GetPremiumFeatures {
            source: Some(Box::new(PremiumSource::Settings(
                td::PremiumSourceSettings {},
            ))),
        });
        futures::future::try_join(state, features).map(|result| {
            let (state, features) = result.map_err(RequestError::from)?;
            Ok(offer(state, features))
        })
    }

    /// The periods Premium can be gifted for.
    pub fn premium_gift_options(
        &self,
    ) -> impl Future<Output = Result<Vec<GiftOption>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetPremiumGiftPaymentOptions {})
            .map(|result| {
                result
                    .map(|list| {
                        list.options
                            .into_iter()
                            .map(|option| GiftOption {
                                currency: option.currency,
                                amount: option.amount,
                                months: option.month_count,
                                discount_percent: option.discount_percentage,
                            })
                            .collect()
                    })
                    .map_err(RequestError::from)
            })
    }

    /// Asks TDLib for the payment form of `link`; nothing is paid here.
    /// None for a link that has no form.
    pub fn open_payment_form(
        &self,
        link: PaymentLink,
    ) -> Option<impl Future<Output = Result<(), RequestError>> + Send + use<>> {
        let invoice = match link {
            PaymentLink::Invoice(name) => InputInvoice::Name(td::InputInvoiceName { name }),
            PaymentLink::Stars {
                currency,
                amount,
                star_count,
            } => InputInvoice::Telegram(td::InputInvoiceTelegram {
                purpose: Box::new(TelegramPaymentPurpose::Stars(
                    td::TelegramPaymentPurposeStars {
                        currency,
                        amount,
                        star_count,
                        chat_id: 0,
                    },
                )),
            }),
            PaymentLink::PremiumGift {
                currency,
                amount,
                user,
                months,
            } => InputInvoice::Telegram(td::InputInvoiceTelegram {
                purpose: Box::new(TelegramPaymentPurpose::PremiumGift(
                    td::TelegramPaymentPurposePremiumGift {
                        currency,
                        amount,
                        user_id: user.0,
                        month_count: months,
                        text: Box::new(td::FormattedText {
                            text: String::new(),
                            entities: Vec::new(),
                        }),
                    },
                )),
            }),
            PaymentLink::Other => return None,
        };
        let request = self.client().request(&GetPaymentForm {
            input_invoice: Box::new(invoice),
            theme: None,
        });
        Some(request.map(|result| result.map(drop).map_err(RequestError::from)))
    }
}

fn offer(state: td::PremiumState, features: td::PremiumFeatures) -> PremiumOffer {
    let options = state
        .payment_options
        .into_iter()
        .map(|option| {
            let option = option.payment_option;
            SubscriptionOption {
                link: PaymentLink::of(option.payment_link.as_deref()),
                currency: option.currency,
                amount: option.amount,
                months: option.month_count,
            }
        })
        .collect();
    PremiumOffer {
        status: FormattedText::from(*state.state),
        options,
        features: features.features.iter().filter_map(Feature::of).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_invoice_links_open_a_form() {
        let invoice = InternalLinkType::Invoice(td::InternalLinkTypeInvoice {
            invoice_name: "premium".to_owned(),
        });
        assert_eq!(
            PaymentLink::of(Some(&invoice)),
            PaymentLink::Invoice("premium".to_owned())
        );
        assert_eq!(PaymentLink::of(None), PaymentLink::Other);
        let topic = PremiumFeature::ForumTopicIcon(td::PremiumFeatureForumTopicIcon {});
        assert_eq!(Feature::of(&topic), None);
    }
}
