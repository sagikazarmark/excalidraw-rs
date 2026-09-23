use crate::{CollectionId, Error, SceneVersion, Timestamp, de, model::Extra};
use serde::{Deserialize, Serialize};

/// A scene, its metadata and its sharing links.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneRecord {
    pub metadata: SceneMetadata,
    pub read_only_links: Vec<ReadOnlyLink>,
    pub shared_slides_links: Vec<SlidesLink>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneMetadata {
    pub id: String,
    /// Minimum length 1 in the published schema.
    pub name: String,
    pub workspace: String,
    pub collection: String,
    /// A bare string here, unlike the collection and workspace timestamps.
    pub created: Timestamp,
    pub updated: Option<Timestamp>,
    pub creator: Option<String>,
    pub updater: Option<String>,
    pub scene_version: SceneVersion,
    /// Published as an integer in `0..=9007199254740991`, default 0. Its meaning
    /// is not documented.
    #[serde(deserialize_with = "de::count")]
    pub content_epoch: u64,
    pub link_sharing: LinkSharing,
    /// The published pattern excludes `.html`, `.js`, `.php`, `.css` and `.exe`.
    pub preview_url: Option<String>,
    pub preview_filename: Option<String>,
    pub preview_background: String,
    pub is_deleted: bool,
    pub is_private: bool,
    pub pinned: bool,
    #[serde(deserialize_with = "de::count")]
    pub update_count: u64,
    #[serde(deserialize_with = "de::count")]
    pub revision_count: u64,
    #[serde(deserialize_with = "de::count")]
    pub total_elements: u64,
    #[serde(deserialize_with = "de::count")]
    pub deleted_elements: u64,
    pub last_revision: Option<String>,

    #[serde(default)]
    pub is_untitled: Option<bool>,
    /// May be negative in the published schema.
    #[serde(default)]
    pub last_acknowledged_version: Option<i64>,
    #[serde(default)]
    pub read_comments: Option<bool>,
    #[serde(default)]
    pub write_comments: Option<bool>,
    #[serde(default)]
    pub allow_calls: Option<bool>,
    #[serde(default)]
    pub has_non_deleted_frames: Option<bool>,
    #[serde(default)]
    pub is_slides_sharing_enabled: Option<bool>,
    #[serde(default)]
    pub is_welcome_scene: Option<bool>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Published as `anyOf[0 | 1 | 3]`. The gap at 2 is real and undocumented, so an
/// unlisted value is retained rather than rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkSharing {
    Off,
    ReadOnly,
    Editable,
    Unknown(i64),
}

impl LinkSharing {
    pub fn as_i64(self) -> i64 {
        match self {
            Self::Off => 0,
            Self::ReadOnly => 1,
            Self::Editable => 3,
            Self::Unknown(value) => value,
        }
    }
}

impl<'de> Deserialize<'de> for LinkSharing {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match i64::deserialize(deserializer)? {
            0 => Self::Off,
            1 => Self::ReadOnly,
            3 => Self::Editable,
            other => Self::Unknown(other),
        })
    }
}

impl Serialize for LinkSharing {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(self.as_i64())
    }
}

/// A read-only share link.
///
/// This and [`SlidesLink`] share nine leading fields, but the artifact does not
/// publish that prefix as a shared schema, so they stay two types: merging them
/// would invent a contract.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadOnlyLink {
    pub id: String,
    pub scene: String,
    pub workspace: String,
    pub created: Timestamp,
    pub updated: Timestamp,
    pub updated_by: Option<String>,
    pub creator: String,
    pub preview: Option<String>,
    pub preview_path: Option<String>,
    /// Always `readonly`.
    pub r#type: String,
    /// `active` or `inactive`.
    pub status: String,
    pub name: Option<String>,
    pub data: ReadOnlyLinkData,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Embed geometry and display toggles for a read-only link.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadOnlyLinkData {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub show_title: Option<bool>,
    #[serde(default)]
    pub show_menu: Option<bool>,
    #[serde(default)]
    pub dark_mode: Option<bool>,
    #[serde(default)]
    pub disable_interaction: Option<bool>,
    #[serde(default)]
    pub show_dark_mode_toggle: Option<bool>,
    #[serde(default)]
    pub show_frame_outlines: Option<bool>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A shared-slides presentation link.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SlidesLink {
    pub id: String,
    pub scene: String,
    pub workspace: String,
    pub created: Timestamp,
    pub updated: Timestamp,
    pub updated_by: Option<String>,
    pub creator: String,
    pub preview: Option<String>,
    pub preview_path: Option<String>,
    pub is_active: bool,
    /// Always `slides`.
    pub r#type: String,
    pub data: SlidesLinkData,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SlidesLinkData {
    pub name: String,
    pub show_slide_title: bool,
    pub animate_slides: bool,
    #[serde(default)]
    pub disallow_download: Option<bool>,
    /// Published range 0..=100.
    #[serde(default)]
    pub auto_play_slides: Option<f64>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Body of `POST /scenes`, and of `POST /collections/{id}/scenes` without the
/// collection field. All fields are required by the published schema: `pinned`
/// has no server-side default.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewScene {
    pub name: String,
    pub pinned: bool,
    /// Serialised only by `POST /scenes`; the collection-scoped create takes it
    /// from the path instead.
    #[serde(skip_serializing_if = "Option::is_none", rename = "collectionId")]
    pub collection: Option<CollectionId>,
}

impl NewScene {
    pub fn new(name: impl Into<String>, collection: CollectionId) -> Self {
        Self {
            name: name.into(),
            pinned: false,
            collection: Some(collection),
        }
    }
    /// For `POST /collections/{collectionId}/scenes`, where the collection is in
    /// the path.
    pub fn in_path(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pinned: false,
            collection: None,
        }
    }
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.name.is_empty() {
            return Err(Error::invalid("scene name", "must not be empty"));
        }
        Ok(())
    }
}

/// Body of `PATCH /scenes/{sceneId}`. Every field is optional.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenePatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "collectionId")]
    pub collection: Option<CollectionId>,
}

impl ScenePatch {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = Some(pinned);
        self
    }
    pub fn collection(mut self, collection: CollectionId) -> Self {
        self.collection = Some(collection);
        self
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.name.is_none() && self.pinned.is_none() && self.collection.is_none() {
            return Err(Error::invalid(
                "scene patch",
                "set at least one of name, pinned or collection",
            ));
        }
        if self.name.as_deref() == Some("") {
            return Err(Error::invalid("scene name", "must not be empty"));
        }
        Ok(())
    }
}
