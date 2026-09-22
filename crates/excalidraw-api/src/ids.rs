//! Opaque server identities. No character class is invented; none is published.
use crate::Error;
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! opaque_id {
    ($name:ident, $what:literal) => {
        #[doc = concat!("An opaque ", $what, " identifier.")]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Rejects only the empty string. Everything else is percent-encoded
            /// when it reaches a path, so no format check is imposed.
            pub fn new(id: impl Into<String>) -> Result<Self, Error> {
                let id = id.into();
                if id.is_empty() {
                    return Err(Error::invalid(concat!($what, " id"), "must not be empty"));
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
