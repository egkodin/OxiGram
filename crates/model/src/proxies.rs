//! Proxies TDLib connects through (`MTP::ProxyData`): the saved list,
//! adding, editing, turning on or off, removing and pinging them.

use std::fmt;
use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::ProxyType;
use td_api::functions::{
    AddProxy, DisableProxy, EditProxy, EnableProxy, GetProxies, PingProxy, RemoveProxy,
};
use td_api::types as td;

use crate::chat_actions::done;
use crate::session::{RequestError, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProxyId(pub i32);

/// How the proxy is reached and its credentials.
#[derive(Clone, PartialEq, Eq)]
pub enum ProxyKind {
    Mtproto { secret: String },
    Socks5 { username: String, password: String },
    Http { username: String, password: String },
}

/// The credentials stay out of any `Debug` output.
impl fmt::Debug for ProxyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Mtproto { .. } => "Mtproto",
            Self::Socks5 { .. } => "Socks5",
            Self::Http { .. } => "Http",
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Proxy {
    pub server: String,
    pub port: u16,
    pub kind: ProxyKind,
}

impl fmt::Debug for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Proxy")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

/// A saved proxy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedProxy {
    pub id: ProxyId,
    pub proxy: Proxy,
    pub is_enabled: bool,
}

impl Proxy {
    fn to_td(&self) -> td::Proxy {
        let kind = match &self.kind {
            ProxyKind::Mtproto { secret } => ProxyType::Mtproto(td::ProxyTypeMtproto {
                secret: secret.clone(),
            }),
            ProxyKind::Socks5 { username, password } => ProxyType::Socks5(td::ProxyTypeSocks5 {
                username: username.clone(),
                password: password.clone(),
            }),
            ProxyKind::Http { username, password } => ProxyType::Http(td::ProxyTypeHttp {
                username: username.clone(),
                password: password.clone(),
                http_only: false,
            }),
        };
        td::Proxy {
            server: self.server.clone(),
            port: i32::from(self.port),
            r#type: Box::new(kind),
        }
    }

    /// None for kinds rgram does not edit (web proxies).
    fn from_td(proxy: td::Proxy) -> Option<Self> {
        let kind = match *proxy.r#type {
            ProxyType::Mtproto(kind) => ProxyKind::Mtproto {
                secret: kind.secret,
            },
            ProxyType::Socks5(kind) => ProxyKind::Socks5 {
                username: kind.username,
                password: kind.password,
            },
            ProxyType::Http(kind) => ProxyKind::Http {
                username: kind.username,
                password: kind.password,
            },
            ProxyType::Web(_) => return None,
        };
        Some(Self {
            server: proxy.server,
            port: u16::try_from(proxy.port).ok()?,
            kind,
        })
    }
}

fn saved(added: td::AddedProxy) -> Option<SavedProxy> {
    Some(SavedProxy {
        id: ProxyId(added.id),
        proxy: Proxy::from_td(*added.proxy)?,
        is_enabled: added.is_enabled,
    })
}

impl Session {
    // shortcut: web proxies (`proxyTypeWeb`) saved elsewhere are left out of the list; add them with the WEB type.
    pub fn proxies(
        &self,
    ) -> impl Future<Output = Result<Vec<SavedProxy>, RequestError>> + Send + use<> {
        self.client().request(&GetProxies {}).map(|result| {
            result
                .map(|list| list.proxies.into_iter().filter_map(saved).collect())
                .map_err(RequestError::from)
        })
    }

    /// Saves a proxy and turns it on.
    pub fn add_proxy(
        &self,
        proxy: &Proxy,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&AddProxy {
                proxy: Box::new(proxy.to_td()),
                enable: true,
                comment: String::new(),
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }

    pub fn edit_proxy(
        &self,
        id: ProxyId,
        proxy: &Proxy,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&EditProxy {
                proxy_id: id.0,
                proxy: Box::new(proxy.to_td()),
                enable: true,
                comment: String::new(),
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }

    pub fn enable_proxy(
        &self,
        id: ProxyId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&EnableProxy { proxy_id: id.0 })
            .map(done)
    }

    /// Connects directly again.
    pub fn disable_proxy(&self) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client().request(&DisableProxy {}).map(done)
    }

    pub fn remove_proxy(
        &self,
        id: ProxyId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&RemoveProxy { proxy_id: id.0 })
            .map(done)
    }

    /// The round trip to Telegram through `proxy`, in seconds.
    pub fn ping_proxy(
        &self,
        proxy: &Proxy,
    ) -> impl Future<Output = Result<f64, RequestError>> + Send + use<> {
        self.client()
            .request(&PingProxy {
                proxy: Some(Box::new(proxy.to_td())),
            })
            .map(|result| {
                result
                    .map(|seconds| seconds.seconds)
                    .map_err(RequestError::from)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxies_round_trip_and_hide_their_secrets() {
        let proxy = Proxy {
            server: "proxy.example".to_owned(),
            port: 443,
            kind: ProxyKind::Socks5 {
                username: "user".to_owned(),
                password: "hidden-value".to_owned(),
            },
        };
        assert_eq!(Proxy::from_td(proxy.to_td()).as_ref(), Some(&proxy));
        let shown = format!("{proxy:?}");
        assert!(!shown.contains("hidden-value") && !shown.contains("proxy.example"));
    }
}
