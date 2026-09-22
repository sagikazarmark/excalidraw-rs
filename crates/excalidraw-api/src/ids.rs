//! Opaque server identities. No character class is invented; none is published.
use crate::Error;
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! opaque_id {
    ($name:ident, $what:literal) => {
        #[doc = concat!("An opaque ", $what, " identifier.")]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// Rejects the empty string and the dot segments `.` and `..`.
            /// Everything else is percent-encoded when it reaches a path, so no
            /// format check is imposed. The dot segments cannot be encoded
            /// away: URL parsing resolves `%2E%2E` exactly like `..`, so an id
            /// of `..` would lift its request out of its route.
            pub fn new(id: impl Into<String>) -> Result<Self, Error> {
                let id = id.into();
                if id.is_empty() {
                    return Err(Error::invalid(concat!($what, " id"), "must not be empty"));
                }
                if id == "." || id == ".." {
                    return Err(Error::invalid(
                        concat!($what, " id"),
                        format!("must not be the dot segment {id:?}"),
                    ));
                }
                Ok(Self(id))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = Error;
            fn from_str(s: &str) -> Result<Self, Error> {
                Self::new(s)
            }
        }

        // Decoding goes through `new`, so no value of this type skips its check.
        impl TryFrom<String> for $name {
            type Error = Error;
            fn try_from(id: String) -> Result<Self, Error> {
                Self::new(id)
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> String {
                id.0
            }
        }
    };
}

opaque_id!(SceneId, "scene");
opaque_id!(UserId, "user");
opaque_id!(InviteId, "invite");
opaque_id!(CollectionId, "collection");

impl CollectionId {
    /// The key owner's virtual private collection.
    ///
    /// Personal API keys may use it. Workspace API keys cannot, and receive
    /// `403 Forbidden`, which is the documented behaviour rather than a fault.
    pub const PRIVATE: &'static str = "private";

    pub fn private() -> Self {
        Self(Self::PRIVATE.to_owned())
    }

    pub fn is_private(&self) -> bool {
        self.as_str() == Self::PRIVATE
    }
}

/// Opaque scene-level reconciliation token.
///
/// Deliberately supports equality only: no ordering, no arithmetic, no parsing.
/// `PUT /scenes/{id}/content` ignores a supplied value and recomputes it from
/// the submitted content, so this is not a compare-and-swap precondition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SceneVersion(String);

impl SceneVersion {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SceneVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
