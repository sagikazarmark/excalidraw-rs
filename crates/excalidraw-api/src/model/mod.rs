//! Response and request bodies.
//!
//! Reads are forward-tolerant: every response type keeps unrecognised fields in
//! an `extra` map, because the API is public beta and adding a field must not
//! break a reader. Writes are exact: request bodies carry only documented
//! fields, and a field the artifact marks required is never skipped.
//!
//! Fields the artifact marks *required but nullable* are `Option<T>`. Note that
//! serde cannot distinguish them from absent: a missing `Option` field decodes
//! to `None` whether or not `#[serde(default)]` is present. A field the server
//! stops sending therefore reads as `null` rather than failing. Detecting that
//! kind of removal is the job of the pinned-artifact drift check against
//! [`crate::OPENAPI_SNAPSHOT`], not of per-response decoding.
mod collection;
mod invite;
mod log;
mod scene;
mod user;
mod workspace;

pub use collection::{Collection, CollectionPatch, Emoji, NewCollection};
pub use invite::{Invite, InvitePatch, InviteStatus, InviteType, MaxUses, NewInvite};
pub use log::{AppSource, LogEntry, LogPage, SourceType};
pub use scene::{
    LinkSharing, NewScene, ReadOnlyLink, ReadOnlyLinkData, SceneMetadata, ScenePatch, SceneRecord,
    SlidesLink, SlidesLinkData,
};
pub use user::{
    InitialRedirect, Locale, RateWindow, SceneOrder, Theme, UserPatch, UserPreferences,
    UserRateLimits, WorkspaceUser,
};
pub use workspace::{Role, SubscriptionStatus, Workspace, WorkspacePatch, WorkspacePreferences};

/// Body of a soft-delete response.
///
/// The artifact declares an open string-keyed object with no defined values.
/// The payload is retained rather than discarded so an unexpected one is visible.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
#[serde(transparent)]
pub struct Ack(pub serde_json::Map<String, serde_json::Value>);

impl Ack {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Unrecognised response fields, preserved verbatim.
pub type Extra = serde_json::Map<String, serde_json::Value>;
