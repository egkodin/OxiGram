//! The account's active sessions (`Api::Authorizations`): the list, ending
//! one or all others, and how long an unused session lives.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::SessionDeviceType;
use td_api::functions::{
    GetActiveSessions, SetInactiveSessionTtl, TerminateAllOtherSessions, TerminateSession,
    ToggleSessionCanAcceptCalls,
};
use td_api::types as td;

use crate::{RequestError, Session};

/// TDLib's id of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub i64);

/// The kinds tdesktop draws an icon for (`DeviceType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    Windows,
    Mac,
    Ubuntu,
    Linux,
    IPhone,
    IPad,
    Android,
    Web,
    Chrome,
    Edge,
    Firefox,
    Safari,
    Other,
}

impl From<&SessionDeviceType> for Device {
    fn from(kind: &SessionDeviceType) -> Self {
        match kind {
            SessionDeviceType::Android(_) => Self::Android,
            SessionDeviceType::Apple(_) | SessionDeviceType::Iphone(_) => Self::IPhone,
            SessionDeviceType::Ipad(_) => Self::IPad,
            SessionDeviceType::Mac(_) => Self::Mac,
            SessionDeviceType::Windows(_) => Self::Windows,
            SessionDeviceType::Linux(_) => Self::Linux,
            SessionDeviceType::Ubuntu(_) => Self::Ubuntu,
            SessionDeviceType::Chrome(_) => Self::Chrome,
            SessionDeviceType::Edge(_) => Self::Edge,
            SessionDeviceType::Firefox(_) => Self::Firefox,
            SessionDeviceType::Safari(_) => Self::Safari,
            SessionDeviceType::Brave(_)
            | SessionDeviceType::Opera(_)
            | SessionDeviceType::Vivaldi(_) => Self::Web,
            SessionDeviceType::Xbox(_) | SessionDeviceType::Unknown(_) => Self::Other,
        }
    }
}

/// One signed-in device or app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSession {
    pub id: SessionId,
    pub is_current: bool,
    /// Signed in with the code but not the cloud password yet.
    pub is_password_pending: bool,
    /// Incoming calls ring here.
    pub can_accept_calls: bool,
    pub device: Device,
    pub device_model: String,
    pub application: String,
    pub platform: String,
    pub system: String,
    pub ip: String,
    pub location: String,
    pub last_active: i32,
    pub logged_in: i32,
}

impl From<td::Session> for ActiveSession {
    fn from(session: td::Session) -> Self {
        let application = if session.application_version.is_empty() {
            session.application_name
        } else {
            format!(
                "{} {}",
                session.application_name, session.application_version
            )
        };
        Self {
            id: SessionId(session.id),
            is_current: session.is_current,
            is_password_pending: session.is_password_pending,
            can_accept_calls: session.can_accept_calls,
            device: Device::from(session.device_type.as_ref()),
            device_model: session.device_model,
            application,
            platform: session.platform,
            system: session.system_version,
            ip: session.ip_address,
            location: session.location,
            last_active: if session.last_active_date != 0 {
                session.last_active_date
            } else {
                session.log_in_date
            },
            logged_in: session.log_in_date,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActiveSessions {
    pub sessions: Vec<ActiveSession>,
    /// Days an unused session lives.
    pub inactive_ttl_days: i32,
}

impl Session {
    pub fn active_sessions(
        &self,
    ) -> impl Future<Output = Result<ActiveSessions, RequestError>> + Send + use<> {
        self.client().request(&GetActiveSessions {}).map(|result| {
            result
                .map(|sessions| ActiveSessions {
                    sessions: sessions
                        .sessions
                        .into_iter()
                        .map(ActiveSession::from)
                        .collect(),
                    inactive_ttl_days: sessions.inactive_session_ttl_days,
                })
                .map_err(RequestError::from)
        })
    }

    pub fn terminate_session(
        &self,
        id: SessionId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&TerminateSession { session_id: id.0 })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }

    pub fn terminate_other_sessions(
        &self,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&TerminateAllOtherSessions {})
            .map(|result| result.map(drop).map_err(RequestError::from))
    }

    /// Lets calls ring in session `id` or not (`toggleCallsDisabledHere`).
    pub fn set_session_accepts_calls(
        &self,
        id: SessionId,
        can_accept_calls: bool,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&ToggleSessionCanAcceptCalls {
                session_id: id.0,
                can_accept_calls,
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }

    pub fn set_inactive_session_ttl(
        &self,
        days: i32,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetInactiveSessionTtl {
                inactive_session_ttl_days: days,
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }
}
