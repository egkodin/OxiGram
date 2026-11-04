//! Telegram Business (`Settings::Business`): its features in the server's
//! order and the Premium switch that hides sponsored messages.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::BusinessFeature as TdFeature;
use td_api::functions::{GetBusinessFeatures, GetUserFullInfo, ToggleHasSponsoredMessagesEnabled};

use crate::ids::UserId;
use crate::session::{RequestError, Session};

/// A feature tdesktop lists on the Business page (`EntryMap`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    Location,
    Hours,
    QuickReplies,
    Greeting,
    Away,
    Links,
    Intro,
    Bots,
    FolderTags,
}

impl Feature {
    /// tdesktop's `FallbackOrder`, for when the server sends none.
    pub const FALLBACK: [Self; 9] = [
        Self::Greeting,
        Self::Away,
        Self::QuickReplies,
        Self::Hours,
        Self::Location,
        Self::Links,
        Self::Intro,
        Self::Bots,
        Self::FolderTags,
    ];

    /// None for what the page leaves off (emoji status, stories).
    fn of(feature: &TdFeature) -> Option<Self> {
        Some(match feature {
            TdFeature::Location(_) => Self::Location,
            TdFeature::OpeningHours(_) => Self::Hours,
            TdFeature::QuickReplies(_) => Self::QuickReplies,
            TdFeature::GreetingMessage(_) => Self::Greeting,
            TdFeature::AwayMessage(_) => Self::Away,
            TdFeature::AccountLinks(_) => Self::Links,
            TdFeature::StartPage(_) => Self::Intro,
            TdFeature::Bots(_) => Self::Bots,
            TdFeature::ChatFolderTags(_) => Self::FolderTags,
            _ => return None,
        })
    }
}

/// Whether a Premium account sees sponsored messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SponsoredMessages {
    Shown,
    Hidden,
}

impl Session {
    /// The features in the server's order (`business_promo_order`).
    pub fn business_features(
        &self,
    ) -> impl Future<Output = Result<Vec<Feature>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetBusinessFeatures { source: None })
            .map(|result| {
                result
                    .map(|list| ordered(list.features.iter().filter_map(Feature::of).collect()))
                    .map_err(RequestError::from)
            })
    }

    /// `me`'s choice (`Api::SponsoredToggle::toggled`).
    pub fn sponsored_setting(
        &self,
        me: UserId,
    ) -> impl Future<Output = Result<SponsoredMessages, RequestError>> + Send + use<> {
        self.client()
            .request(&GetUserFullInfo { user_id: me.0 })
            .map(|result| {
                result
                    .map(|info| {
                        if info.has_sponsored_messages_enabled {
                            SponsoredMessages::Shown
                        } else {
                            SponsoredMessages::Hidden
                        }
                    })
                    .map_err(RequestError::from)
            })
    }

    pub fn set_sponsored_setting(
        &self,
        value: SponsoredMessages,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&ToggleHasSponsoredMessagesEnabled {
                has_sponsored_messages_enabled: value == SponsoredMessages::Shown,
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }
}

/// The server's order, or the fallback when it lists nothing the page shows.
fn ordered(features: Vec<Feature>) -> Vec<Feature> {
    if features.is_empty() {
        Feature::FALLBACK.to_vec()
    } else {
        features
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use td_api::types as td;

    #[test]
    fn the_page_keeps_its_own_features_in_the_server_order() {
        let listed = [
            TdFeature::Bots(td::BusinessFeatureBots {}),
            TdFeature::EmojiStatus(td::BusinessFeatureEmojiStatus {}),
            TdFeature::Location(td::BusinessFeatureLocation {}),
        ];
        let features = ordered(listed.iter().filter_map(Feature::of).collect());
        assert_eq!(features, [Feature::Bots, Feature::Location]);
        assert_eq!(ordered(Vec::new()), Feature::FALLBACK);
    }
}
