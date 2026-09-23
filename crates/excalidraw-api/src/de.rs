//! Deserialisation helpers shared by the models.
//!
//! The published schema types several counters as `number` rather than
//! `integer`. Rather than widening every counter to `f64`, these helpers accept
//! integral JSON numbers and reject fractional or negative values.
use serde::{Deserialize, Deserializer, de};

/// Largest integer JavaScript represents exactly, and the published maximum for
/// every bounded integer field in the artifact.
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

fn to_count(number: &serde_json::Number) -> Result<u64, String> {
    // Judged on the exact decimal text (`arbitrary_precision` keeps it), never
    // through `f64`: rounding first would turn `9007199254740990.5` or
    // `0.99999999999999999999` into an integer this helper promises to refuse.
    match excalidraw_document::Number(number.clone()).as_safe_integer() {
        Ok(value) if value >= 0 => Ok(value as u64),
        _ if number
            .as_u64()
            .is_some_and(|value| value > MAX_SAFE_INTEGER) =>
        {
            Err(format!("{number} exceeds the published maximum"))
        }
        _ => Err(format!(
            "expected a nonnegative integral count, got {number}"
        )),
    }
}

pub fn count<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let number = serde_json::Number::deserialize(deserializer)?;
    to_count(&number).map_err(de::Error::custom)
}

pub fn opt_count<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    let number = Option::<serde_json::Number>::deserialize(deserializer)?;
    match number {
        None => Ok(None),
        Some(number) => to_count(&number).map(Some).map_err(de::Error::custom),
    }
}

/// A wire timestamp, preserved exactly as sent.
///
/// The published schema is inconsistent: collection and workspace timestamps
/// declare `format: date-time` with a strict pattern, while scene and log
/// timestamps are bare strings. The value is kept verbatim and parsing is left
/// to the caller, so the crate needs no date dependency and cannot normalise a
/// format the server did not promise.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(pub String);

impl Timestamp {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Declare an open string enum: documented variants plus `Unknown(String)`, so
/// an unlisted future value still decodes.
macro_rules! open_string_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum $name {
            $($variant,)+
            /// A value the pinned artifact does not list.
            Unknown(String),
        }

        impl $name {
            /// The wire spelling, or `None` for an undocumented value.
            pub fn wire_name(&self) -> Option<&str> {
                match self {
                    $(Self::$variant => Some($wire),)+
                    Self::Unknown(_) => None,
                }
            }
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $wire,)+
                    Self::Unknown(value) => value,
                }
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(d)?;
                Ok(match raw.as_str() {
                    $($wire => Self::$variant,)+
                    _ => Self::Unknown(raw),
                })
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

pub(crate) use open_string_enum;

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<u64, String> {
        to_count(&serde_json::from_str(text).unwrap())
    }

    #[test]
    fn counts_are_judged_on_the_exact_decimal_rather_than_a_rounded_f64() {
        for text in [
            "9007199254740990.5",
            "0.99999999999999999999",
            "1.0000000000000000001",
            "9007199254740991.4",
            "0.5",
            "-1",
            "9007199254740992",
            "1e16",
        ] {
            assert!(parse(text).is_err(), "{text} was accepted");
        }
        for (text, value) in [
            ("0", 0),
            ("-0", 0),
            ("5", 5),
            ("5.0", 5),
            ("1e3", 1000),
            ("9007199254740991", MAX_SAFE_INTEGER),
        ] {
            assert_eq!(parse(text), Ok(value), "{text}");
        }
        assert!(
            parse("9007199254740992")
                .unwrap_err()
                .contains("published maximum")
        );
    }
}
