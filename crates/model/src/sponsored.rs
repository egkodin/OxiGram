//! Ads TDLib shows after the last post of a channel
//! (`Data::SponsoredMessages`).

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{self, MessageContent};
use td_api::functions::{
    ClickChatSponsoredMessage, GetChatSponsoredMessages, ReportChatSponsoredMessage,
};

use crate::chat_actions::done;
use crate::ids::{ChatId, MessageId};
use crate::session::{RequestError, Session};
use crate::text::FormattedText;

/// One ad: its "Ad" box with the title, the text and a button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sponsored {
    pub id: MessageId,
    pub is_recommended: bool,
    pub can_be_reported: bool,
    pub text: FormattedText,
    pub title: String,
    pub button_text: String,
    pub url: String,
    pub accent_color: i32,
    /// The sponsor's and the ad's details, for "Advertiser info".
    pub info: Vec<String>,
}

/// What reporting an ad came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportResult {
    Reported,
    Failed,
    /// The user picks one and reports again with its id.
    OptionRequired {
        title: String,
        options: Vec<ReportOption>,
    },
    AdsHidden,
    PremiumRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportOption {
    pub id: String,
    pub text: String,
}

impl Sponsored {
    // shortcut: text ads only; photo, video and animation ads need media (M5) and are skipped.
    fn from_td(message: td_api::types::SponsoredMessage) -> Option<Self> {
        let MessageContent::MessageText(content) = *message.content else {
            return None;
        };
        let info = [message.sponsor.info, message.additional_info]
            .into_iter()
            .filter(|info| !info.is_empty())
            .collect();
        Some(Self {
            id: MessageId(message.message_id),
            is_recommended: message.is_recommended,
            can_be_reported: message.can_be_reported,
            text: FormattedText::from(*content.text),
            title: message.title,
            button_text: message.button_text,
            url: message.sponsor.url,
            accent_color: message.accent_color_id,
            info,
        })
    }
}

impl From<enums::ReportSponsoredResult> for ReportResult {
    fn from(result: enums::ReportSponsoredResult) -> Self {
        use enums::ReportSponsoredResult as R;
        match result {
            R::Ok(_) => Self::Reported,
            R::Failed(_) => Self::Failed,
            R::OptionRequired(required) => Self::OptionRequired {
                title: required.title,
                options: required
                    .options
                    .into_iter()
                    .map(|option| ReportOption {
                        id: option.id,
                        text: option.text,
                    })
                    .collect(),
            },
            R::AdsHidden(_) => Self::AdsHidden,
            R::PremiumRequired(_) => Self::PremiumRequired,
        }
    }
}

impl Session {
    /// The ads to show in a channel, those oxigram can draw.
    pub fn sponsored_messages(
        &self,
        chat: ChatId,
    ) -> impl Future<Output = Result<Vec<Sponsored>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetChatSponsoredMessages { chat_id: chat.0 })
            .map(|result| {
                let list = result.map_err(RequestError::from)?;
                Ok(list
                    .messages
                    .into_iter()
                    .filter_map(Sponsored::from_td)
                    .collect())
            })
    }

    /// Tells TDLib the user opened the ad's link.
    pub fn click_sponsored(
        &self,
        chat: ChatId,
        message: MessageId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&ClickChatSponsoredMessage {
                chat_id: chat.0,
                message_id: message.0,
                is_media_click: false,
                from_fullscreen: false,
            })
            .map(done)
    }

    /// Reports an ad; `option` is empty first, then the chosen option's id.
    pub fn report_sponsored(
        &self,
        chat: ChatId,
        message: MessageId,
        option: String,
    ) -> impl Future<Output = Result<ReportResult, RequestError>> + Send + use<> {
        self.client()
            .request(&ReportChatSponsoredMessage {
                chat_id: chat.0,
                message_id: message.0,
                option_id: option,
            })
            .map(|result| result.map(ReportResult::from).map_err(RequestError::from))
    }
}

#[cfg(test)]
mod tests {
    use td_api::types::{AdvertisementSponsor, FormattedText as TdText, MessageText};

    use super::*;

    fn ad(content: MessageContent) -> td_api::types::SponsoredMessage {
        td_api::types::SponsoredMessage {
            message_id: 7,
            is_recommended: false,
            can_be_reported: true,
            content: Box::new(content),
            sponsor: Box::new(AdvertisementSponsor {
                url: "https://t.me/trails".to_owned(),
                photo: None,
                info: "Trails Ltd".to_owned(),
            }),
            title: "Trail Maps".to_owned(),
            button_text: "Open".to_owned(),
            accent_color_id: 3,
            background_custom_emoji_id: 0,
            additional_info: String::new(),
        }
    }

    #[test]
    fn text_ads_keep_their_details_and_media_ads_are_skipped() {
        let text = MessageContent::MessageText(MessageText {
            text: Box::new(TdText {
                text: "Every route, offline.".to_owned(),
                entities: Vec::new(),
            }),
            link_preview: None,
            link_preview_options: None,
        });
        let sponsored = Sponsored::from_td(ad(text)).expect("a text ad is shown");
        assert_eq!(sponsored.id, MessageId(7));
        assert_eq!(sponsored.text.text, "Every route, offline.");
        assert_eq!(sponsored.info, ["Trails Ltd"]);
        let unsupported = MessageContent::MessageUnsupported(td_api::types::MessageUnsupported {});
        assert_eq!(Sponsored::from_td(ad(unsupported)), None);
    }
}
