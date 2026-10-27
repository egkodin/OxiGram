//! What phone numbers and bank cards in messages resolve to
//! (`PhoneClickHandler`, `BankCardClickHandler`).

use std::future::Future;

use futures::FutureExt as _;
use td_api::functions::{GetBankCardInfo, SearchUserByPhoneNumber};
use tg::TdError;

use crate::ids::UserId;
use crate::session::{RequestError, Session};

/// TDLib's answer when no account has the number.
const NOT_FOUND: i32 = 400;

/// The bank behind a card number and what it offers to open.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BankCard {
    pub title: String,
    pub actions: Vec<BankCardAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankCardAction {
    pub text: String,
    pub url: String,
}

impl Session {
    /// Who has a phone number; none when nobody on Telegram does.
    pub fn user_by_phone(
        &self,
        phone: &str,
    ) -> impl Future<Output = Result<Option<UserId>, RequestError>> + Send + use<> {
        self.client()
            .request(&SearchUserByPhoneNumber {
                phone_number: phone.to_owned(),
                only_local: false,
            })
            .map(|result| match result {
                Ok(user) => Ok(Some(UserId(user.id))),
                Err(TdError::Td {
                    code: NOT_FOUND, ..
                }) => Ok(None),
                Err(error) => Err(RequestError::from(error)),
            })
    }

    /// The bank of a card number and its links.
    pub fn bank_card(
        &self,
        number: &str,
    ) -> impl Future<Output = Result<BankCard, RequestError>> + Send + use<> {
        self.client()
            .request(&GetBankCardInfo {
                bank_card_number: number.to_owned(),
            })
            .map(|result| {
                result
                    .map(|info| BankCard {
                        title: info.title,
                        actions: info
                            .actions
                            .into_iter()
                            .map(|action| BankCardAction {
                                text: action.text,
                                url: action.url,
                            })
                            .collect(),
                    })
                    .map_err(RequestError::from)
            })
    }
}
