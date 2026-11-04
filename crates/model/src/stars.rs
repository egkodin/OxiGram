//! Telegram Stars (`Data::Credits`): the balance, the top-up packages and
//! the transaction history.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{MessageSender, StarTransactionType, TransactionDirection};
use td_api::functions::{GetStarPaymentOptions, GetStarTransactions};
use td_api::types as td;

use crate::ids::{ChatId, UserId};
use crate::premium::PaymentLink;
use crate::session::{RequestError, Session};

/// How many transactions one page brings (`kTransactionsLimit`).
const PAGE: i32 = 50;

/// A package of Stars to buy (`Data::CreditTopupOption`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopUpOption {
    pub star_count: i64,
    pub currency: String,
    /// In the currency's minor units.
    pub amount: i64,
    /// Shown only after "More Options".
    pub is_extended: bool,
}

impl TopUpOption {
    #[must_use]
    pub fn link(&self) -> PaymentLink {
        PaymentLink::Stars {
            currency: self.currency.clone(),
            amount: self.amount,
            star_count: self.star_count,
        }
    }
}

/// Who or what a transaction was with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Party {
    User(UserId),
    Chat(ChatId),
    PremiumBot,
    AppStore,
    GooglePlay,
    Fragment,
    Ads,
    /// Anything else, under the generic title.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub id: String,
    /// Whole Stars, negative when spent.
    pub stars: i64,
    pub is_refund: bool,
    pub date: i32,
    pub party: Party,
}

/// Which side of the history a tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    All,
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    /// The balance in whole Stars.
    pub balance: i64,
    pub transactions: Vec<Transaction>,
}

impl Session {
    /// The packages TDLib offers (`Api::CreditsTopupOptions`).
    pub fn star_top_up_options(
        &self,
    ) -> impl Future<Output = Result<Vec<TopUpOption>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetStarPaymentOptions {})
            .map(|result| {
                result
                    .map(|options| {
                        options
                            .options
                            .into_iter()
                            .map(|option| TopUpOption {
                                star_count: option.star_count,
                                currency: option.currency,
                                amount: option.amount,
                                is_extended: option.is_additional,
                            })
                            .collect()
                    })
                    .map_err(RequestError::from)
            })
    }

    /// The first page of `me`'s transactions in `direction`, with the
    /// balance (`Api::CreditsHistory`).
    pub fn star_history(
        &self,
        me: UserId,
        direction: Direction,
    ) -> impl Future<Output = Result<History, RequestError>> + Send + use<> {
        let direction = match direction {
            Direction::All => None,
            Direction::Incoming => Some(TransactionDirection::Incoming(
                td::TransactionDirectionIncoming {},
            )),
            Direction::Outgoing => Some(TransactionDirection::Outgoing(
                td::TransactionDirectionOutgoing {},
            )),
        };
        self.client()
            .request(&GetStarTransactions {
                owner_id: Box::new(MessageSender::User(td::MessageSenderUser { user_id: me.0 })),
                subscription_id: String::new(),
                direction: direction.map(Box::new),
                offset: String::new(),
                limit: PAGE,
            })
            .map(|result| {
                result
                    .map(|page| History {
                        balance: page.star_amount.star_count,
                        transactions: page.transactions.into_iter().map(transaction).collect(),
                    })
                    .map_err(RequestError::from)
            })
    }
}

fn transaction(item: td::StarTransaction) -> Transaction {
    Transaction {
        stars: item.star_amount.star_count,
        is_refund: item.is_refund,
        date: item.date,
        party: party(&item.r#type),
        id: item.id,
    }
}

fn party(kind: &StarTransactionType) -> Party {
    match kind {
        StarTransactionType::PremiumBotDeposit(_) => Party::PremiumBot,
        StarTransactionType::AppStoreDeposit(_) => Party::AppStore,
        StarTransactionType::GooglePlayDeposit(_) => Party::GooglePlay,
        StarTransactionType::FragmentDeposit(_) | StarTransactionType::FragmentWithdrawal(_) => {
            Party::Fragment
        }
        StarTransactionType::TelegramAdsWithdrawal(_) => Party::Ads,
        other => counterpart(other).unwrap_or(Party::Other),
    }
}

/// The user or chat a transaction names. Some forty kinds carry one under
/// `user_id`, `chat_id`, `owner_id` or `sender_id`; reading the field by
/// name keeps new kinds working without a case each.
fn counterpart(kind: &StarTransactionType) -> Option<Party> {
    let value = serde_json::to_value(kind).ok()?;
    let id = |field: &serde_json::Value, key: &str| field.get(key)?.as_i64().filter(|id| *id != 0);
    if let Some(user) = id(&value, "user_id") {
        return Some(Party::User(UserId(user)));
    }
    if let Some(chat) = id(&value, "chat_id") {
        return Some(Party::Chat(ChatId(chat)));
    }
    let sender = value.get("owner_id").or_else(|| value.get("sender_id"))?;
    id(sender, "user_id")
        .map(|user| Party::User(UserId(user)))
        .or_else(|| id(sender, "chat_id").map(|chat| Party::Chat(ChatId(chat))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transactions_name_their_counterpart() {
        let deposit =
            StarTransactionType::PremiumBotDeposit(td::StarTransactionTypePremiumBotDeposit {});
        assert_eq!(party(&deposit), Party::PremiumBot);
        let reaction = StarTransactionType::ChannelPaidReactionSend(
            td::StarTransactionTypeChannelPaidReactionSend {
                chat_id: -1_007,
                message_id: 1,
            },
        );
        assert_eq!(party(&reaction), Party::Chat(ChatId(-1_007)));
        let search =
            StarTransactionType::PublicPostSearch(td::StarTransactionTypePublicPostSearch {});
        assert_eq!(party(&search), Party::Other);
    }
}
