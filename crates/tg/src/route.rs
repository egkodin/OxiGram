//! Finds where an object from `td_receive` belongs without parsing it.
//!
//! TDLib writes `@type` first and appends `"@extra"` (when the request had one)
//! and `"@client_id"` as the last fields (`from_response` in ClientJson.cpp):
//! `{"@type":"ok","@extra":7,"@client_id":1}`. A quote inside a JSON string is
//! always escaped, so these byte patterns cannot occur inside string values.

use serde::Deserialize;
use td_sys::ClientId;

/// The routing fields of one object.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Route {
    pub(crate) client: ClientId,
    /// Set on responses; `None` on updates.
    pub(crate) extra: Option<u64>,
}

impl Route {
    /// Reads the routing fields from the tail of `object`, falling back to a
    /// full parse if TDLib's layout ever changes.
    pub(crate) fn of(object: &str) -> Option<Self> {
        Self::from_tail(object).or_else(|| Self::from_envelope(object))
    }

    fn from_tail(object: &str) -> Option<Self> {
        let body = object.strip_suffix('}')?;
        let (body, client) = body.rsplit_once(r#","@client_id":"#)?;
        let client = ClientId(client.parse().ok()?);
        let extra = match body.rsplit_once(r#","@extra":"#) {
            Some((_, extra)) => Some(extra.parse().ok()?),
            None => None,
        };
        Some(Self { client, extra })
    }

    fn from_envelope(object: &str) -> Option<Self> {
        #[derive(Deserialize)]
        struct Envelope {
            #[serde(rename = "@client_id")]
            client: Option<i32>,
            #[serde(rename = "@extra")]
            extra: Option<u64>,
        }
        let envelope: Envelope = serde_json::from_str(object).ok()?;
        tracing::warn!(
            object = type_name(object),
            "TDLib object layout changed; routed by a full parse"
        );
        Some(Self {
            client: ClientId(envelope.client?),
            extra: envelope.extra,
        })
    }
}

/// The `@type` of an object, for logs. Never returns field values.
pub(crate) fn type_name(object: &str) -> &str {
    object
        .strip_prefix(r#"{"@type":""#)
        .and_then(|rest| rest.split_once('"'))
        .map_or("<unknown>", |(name, _)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responses_carry_extra_and_client() {
        let route = Route::of(r#"{"@type":"ok","@extra":42,"@client_id":3}"#).unwrap();
        assert_eq!(
            route,
            Route {
                client: ClientId(3),
                extra: Some(42)
            }
        );
    }

    #[test]
    fn updates_carry_only_the_client() {
        let object = r#"{"@type":"updateOption","name":"version","value":{"@type":"optionValueString","value":"1.8.69"},"@client_id":1}"#;
        let route = Route::of(object).unwrap();
        assert_eq!(
            route,
            Route {
                client: ClientId(1),
                extra: None
            }
        );
    }

    #[test]
    fn escaped_routing_text_inside_strings_is_ignored() {
        let object = r#"{"@type":"updateNewMessage","text":"x\",\"@extra\":5","@client_id":2}"#;
        assert_eq!(Route::of(object).unwrap().extra, None);
    }

    #[test]
    fn falls_back_to_parsing_when_fields_are_reordered() {
        let object = r#"{"@client_id":4,"@extra":9,"@type":"ok"}"#;
        assert_eq!(
            Route::of(object).unwrap(),
            Route {
                client: ClientId(4),
                extra: Some(9)
            }
        );
    }

    #[test]
    fn objects_without_a_client_are_not_routed() {
        assert_eq!(Route::of(r#"{"@type":"ok"}"#), None);
    }

    #[test]
    fn type_name_reads_only_the_tag() {
        assert_eq!(
            type_name(r#"{"@type":"updateNewMessage","text":"secret"}"#),
            "updateNewMessage"
        );
        assert_eq!(type_name("garbage"), "<unknown>");
    }
}
