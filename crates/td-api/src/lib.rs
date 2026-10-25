//! TDLib's JSON API as Rust types, generated at build time from
//! `tl/td_api.tl` (a verbatim copy of the schema at the commit in `tdlib/COMMIT`).
//!
//! See `td-api-gen` for the mapping rules. Apart from the [`Function`] trait and
//! the `int64` serde adapters, everything here is generated.

use serde::Serialize;
use serde::de::DeserializeOwned;

/// A TDLib function: a request whose JSON form carries its `@type`.
pub trait Function: Serialize {
    /// The function's name in the schema, such as `getOption`.
    const NAME: &'static str;
    /// What TDLib returns on success.
    type Response: DeserializeOwned;
}

#[allow(
    missing_docs,
    clippy::all,
    reason = "generated from td_api.tl; the schema's own descriptions are the docs"
)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/td_api.rs"));
}

pub use generated::{enums, functions, types};

/// TDLib writes `int64` as a JSON string; it also accepts and may send numbers.
mod int64 {
    use std::fmt;

    use serde::de::{self, Deserializer, Visitor};
    use serde::{Deserialize, Serializer};

    pub(crate) fn serialize<S: Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(value)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
        deserializer.deserialize_any(Int64Visitor)
    }

    struct Int64Visitor;

    impl Visitor<'_> for Int64Visitor {
        type Value = i64;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an int64 as a string or a number")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<i64, E> {
            value.parse().map_err(E::custom)
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<i64, E> {
            Ok(value)
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<i64, E> {
            i64::try_from(value).map_err(E::custom)
        }
    }

    /// One `int64` inside a list.
    struct Int64(i64);

    impl<'de> Deserialize<'de> for Int64 {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            deserializer.deserialize_any(Int64Visitor).map(Int64)
        }
    }

    /// `vector<int64>`.
    pub(crate) mod vec {
        use serde::{Deserialize, Deserializer, Serializer};

        pub(crate) fn serialize<S: Serializer>(
            values: &[i64],
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            serializer.collect_seq(values.iter().map(ToString::to_string))
        }

        pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Vec<i64>, D::Error> {
            let values = Vec::<super::Int64>::deserialize(deserializer)?;
            Ok(values.into_iter().map(|value| value.0).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn functions_serialize_with_their_type_tag() {
        let request = functions::GetOption {
            name: "version".to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&request).unwrap(),
            r#"{"@type":"getOption","name":"version"}"#
        );
        assert_eq!(functions::GetOption::NAME, "getOption");
    }

    #[test]
    fn enums_decode_tdlib_responses() {
        let value: enums::OptionValue = serde_json::from_str(
            r#"{"@type":"optionValueString","value":"1.8.69","@extra":7,"@client_id":1}"#,
        )
        .unwrap();
        assert_eq!(
            value,
            enums::OptionValue::String(types::OptionValueString {
                value: "1.8.69".to_owned()
            })
        );
    }

    #[test]
    fn int64_round_trips_as_a_string() {
        let value: enums::OptionValue =
            serde_json::from_str(r#"{"@type":"optionValueInteger","value":"-9007199254740993"}"#)
                .unwrap();
        assert_eq!(
            value,
            enums::OptionValue::Integer(types::OptionValueInteger {
                value: -9_007_199_254_740_993
            })
        );
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            r#"{"@type":"optionValueInteger","value":"-9007199254740993"}"#
        );
    }

    #[test]
    fn single_constructor_results_ignore_their_type_tag() {
        let ok: types::Ok = serde_json::from_str(r#"{"@type":"ok","@extra":1}"#).unwrap();
        assert_eq!(ok, types::Ok {});
    }

    #[test]
    fn unknown_update_types_fail_to_decode() {
        let result = serde_json::from_str::<enums::Update>(r#"{"@type":"updateFromTheFuture"}"#);
        assert!(result.is_err());
    }
}
