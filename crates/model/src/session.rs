use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::OptionValue;
use td_api::functions::{
    DownloadFile, GetCountries, GetCountryCode, LoadChats, SetOption, SetProfilePhoto,
    SetTdlibParameters,
};
use td_api::types::{InputChatPhotoStatic, InputFileLocal, OptionValueString};
use tg::TdError;

use crate::auth;
use crate::chat::ChatList;
use crate::ids::FileId;
use crate::phone::Country;
use crate::secret::Secret;

/// Sends the user's actions to TDLib. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Session {
    client: tg::Client,
    views: Views,
}

/// Whether showing messages tells Telegram they were seen, which reads them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Views {
    #[default]
    Report,
    /// Browsing reads nothing: for checking the app on a real account.
    Withhold,
}

/// Which Telegram servers to use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Environment {
    #[default]
    Production,
    /// Telegram's test servers, with separate accounts.
    Test,
}

/// What TDLib needs before anything else (`setTdlibParameters`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameters {
    pub environment: Environment,
    pub database_directory: String,
    pub files_directory: String,
    /// Where TDLib keeps downloaded language packs.
    pub language_pack_database: String,
    pub api_id: i32,
    pub api_hash: Secret,
    /// IETF tag of the system language.
    pub system_language: String,
    pub device_model: String,
    pub system_version: String,
    pub application_version: String,
    pub views: Views,
}

/// Whether a chat list has more chats to load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListLoad {
    HasMore,
    Complete,
}

/// A request TDLib did not complete.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(transparent)]
pub struct RequestError(#[from] TdError);

impl Session {
    #[must_use]
    pub fn new(client: tg::Client) -> Self {
        Self {
            client,
            views: Views::default(),
        }
    }

    #[must_use]
    pub fn with_views(self, views: Views) -> Self {
        Self { views, ..self }
    }

    pub(crate) fn views(&self) -> Views {
        self.views
    }

    /// Takes a sign-in step. The future reports whether TDLib accepted it;
    /// the result itself arrives as the next authorization state.
    pub fn auth(
        &self,
        action: auth::Action,
    ) -> impl Future<Output = Result<(), auth::Error>> + Send + use<> {
        action.send(&self.client)
    }

    /// Answers `authorizationStateWaitTdlibParameters`. Every database is on,
    /// so the account and its chats survive restarts. Language packs come
    /// from tdesktop's localization target, so their keys match `i18n`.
    pub fn set_parameters(
        &self,
        parameters: &Parameters,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let target = self.set_option("localization_target", "tdesktop");
        let database = self.set_option(
            "language_pack_database_path",
            &parameters.language_pack_database,
        );
        let tdlib = self
            .client
            .request(&SetTdlibParameters {
                use_test_dc: parameters.environment == Environment::Test,
                database_directory: parameters.database_directory.clone(),
                files_directory: parameters.files_directory.clone(),
                // shortcut: unencrypted like tdesktop without a passcode; the local passcode (M6) sets a key.
                database_encryption_key: String::new(),
                use_file_database: true,
                use_chat_info_database: true,
                use_message_database: true,
                use_secret_chats: true,
                api_id: parameters.api_id,
                api_hash: parameters.api_hash.expose().to_owned(),
                system_language_code: parameters.system_language.clone(),
                device_model: parameters.device_model.clone(),
                system_version: parameters.system_version.clone(),
                application_version: parameters.application_version.clone(),
            })
            .map(|result| result.map(drop).map_err(RequestError));
        async move {
            target.await?;
            database.await?;
            tdlib.await
        }
    }

    /// Sets a string option, such as `language_pack_id`.
    pub fn set_option(
        &self,
        name: &str,
        value: &str,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client
            .request(&SetOption {
                name: name.to_owned(),
                value: Some(Box::new(OptionValue::String(OptionValueString {
                    value: value.to_owned(),
                }))),
            })
            .map(|result| result.map(drop).map_err(RequestError))
    }

    /// Sets the signed-in user's photo from a local image file.
    pub fn set_profile_photo(
        &self,
        path: &str,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client
            .request(&SetProfilePhoto {
                photo: Box::new(td_api::enums::InputChatPhoto::Static(
                    InputChatPhotoStatic {
                        photo: Box::new(td_api::enums::InputFile::Local(InputFileLocal {
                            path: path.to_owned(),
                        })),
                    },
                )),
                is_public: false,
            })
            .map(|result| result.map(drop).map_err(RequestError))
    }

    /// Asks TDLib for `limit` more chats of `list`; they arrive as updates.
    pub fn load_chats(
        &self,
        list: ChatList,
        limit: i32,
    ) -> impl Future<Output = Result<ListLoad, RequestError>> + Send + use<> {
        self.client
            .request(&LoadChats {
                chat_list: Some(Box::new(list.to_td())),
                limit,
            })
            .map(|result| match result {
                Ok(_) => Ok(ListLoad::HasMore),
                // TDLib answers 404 once the whole list is loaded.
                Err(TdError::Td { code: 404, .. }) => Ok(ListLoad::Complete),
                Err(error) => Err(RequestError(error)),
            })
    }

    /// Starts downloading a file; progress arrives as file updates. Priority
    /// runs from 1 (lowest) to 32.
    pub fn download_file(
        &self,
        file: FileId,
        priority: i32,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client
            .request(&DownloadFile {
                file_id: file.0,
                priority,
                offset: 0,
                limit: 0,
                synchronous: false,
            })
            .map(|result| result.map(drop).map_err(RequestError))
    }

    pub(crate) fn client(&self) -> &tg::Client {
        &self.client
    }

    /// The countries for the phone number screen, localized to the current
    /// language pack.
    pub fn countries(
        &self,
    ) -> impl Future<Output = Result<Vec<Country>, RequestError>> + Send + use<> {
        self.client.request(&GetCountries {}).map(|result| {
            result
                .map(|countries| countries.countries.into_iter().map(Country::from).collect())
                .map_err(RequestError)
        })
    }

    /// The ISO 3166-1 code of the country the user connects from, by IP.
    pub fn country_code(
        &self,
    ) -> impl Future<Output = Result<String, RequestError>> + Send + use<> {
        self.client
            .request(&GetCountryCode {})
            .map(|result| result.map(|code| code.text).map_err(RequestError))
    }
}
