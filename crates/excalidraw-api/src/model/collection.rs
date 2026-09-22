use crate::{Error, Timestamp, de, model::Extra};
use serde::{Deserialize, Serialize};

/// A collection of scenes.
///
/// `creator`, `teams` and `updated` are required *and* nullable in the published
/// schema. They decode to `None` both from an explicit `null` and from an absent
/// key: serde treats a missing `Option` field as `None`, so this crate cannot
/// report a field the server stopped sending. The drift check catches that.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub workspace: String,
    /// `format: date-time` with a strict pattern, unlike scene timestamps.
    pub created: Timestamp,
    pub updated: Option<Timestamp>,
    pub creator: Option<String>,
    pub teams: Option<Vec<String>>,

    #[serde(default)]
    pub updater: Option<String>,
    #[serde(default)]
    pub is_default: Option<bool>,
    #[serde(default)]
    pub sort_order: Option<String>,
    #[serde(default)]
    pub is_deleted: Option<bool>,
    #[serde(default)]
    pub emoji: Option<Emoji>,
    #[serde(default, deserialize_with = "de::opt_count")]
    pub deleted_at: Option<u64>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// All three fields are required when an emoji is present.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Emoji {
    pub id: String,
    pub native: String,
    pub unified: String,
}

/// Body of `POST /collections`. Always creates a regular shared collection: the
/// virtual private collection cannot be created, renamed or deleted.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NewCollection {
    pub name: String,
}

impl NewCollection {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.name.is_empty() {
            return Err(Error::invalid("collection name", "must not be empty"));
        }
        Ok(())
    }
}

/// Body of `PATCH /collections/{collectionId}`.
///
/// `name` is required by the published schema despite the verb, so it is a
/// `String` rather than an `Option<String>`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CollectionPatch {
    pub name: String,
}

impl CollectionPatch {
    pub fn name(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.name.is_empty() {
            return Err(Error::invalid("collection name", "must not be empty"));
        }
        Ok(())
    }
}
