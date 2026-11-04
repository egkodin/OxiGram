//! Websites the account logged in to with Telegram (`Api::Websites`): the
//! list and disconnecting one or all.

use std::future::Future;

use futures::FutureExt as _;
use td_api::functions::{DisconnectAllWebsites, DisconnectWebsite, GetConnectedWebsites};
use td_api::types as td;

use crate::{RequestError, Session, UserId};

/// TDLib's id of a connected website.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WebsiteId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Website {
    pub id: WebsiteId,
    pub domain: String,
    /// The bot that logged the account in.
    pub bot: UserId,
    pub browser: String,
    pub platform: String,
    pub ip: String,
    pub location: String,
    pub last_active: i32,
}

impl From<td::ConnectedWebsite> for Website {
    fn from(site: td::ConnectedWebsite) -> Self {
        Self {
            id: WebsiteId(site.id),
            domain: site.domain_name,
            bot: UserId(site.bot_user_id),
            browser: site.browser,
            platform: site.platform,
            ip: site.ip_address,
            location: site.location,
            last_active: site.last_active_date,
        }
    }
}

impl Session {
    /// The websites, most recently active first.
    pub fn connected_websites(
        &self,
    ) -> impl Future<Output = Result<Vec<Website>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetConnectedWebsites {})
            .map(|result| {
                result
                    .map(|sites| {
                        let mut sites: Vec<Website> =
                            sites.websites.into_iter().map(Website::from).collect();
                        sites.sort_by_key(|site| std::cmp::Reverse(site.last_active));
                        sites
                    })
                    .map_err(RequestError::from)
            })
    }

    pub fn disconnect_website(
        &self,
        id: WebsiteId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&DisconnectWebsite { website_id: id.0 })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }

    pub fn disconnect_all_websites(
        &self,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&DisconnectAllWebsites {})
            .map(|result| result.map(drop).map_err(RequestError::from))
    }
}
