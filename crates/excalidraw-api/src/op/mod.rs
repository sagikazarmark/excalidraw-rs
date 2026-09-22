//! One type per documented operation.
//!
//! Each is a plain struct of its inputs, implementing [`crate::Operation`] —
//! most by way of [`crate::JsonOperation`], which supplies the shared "expect a
//! 2xx, then parse JSON" decode. `Output` is the decoded 200 body. Status
//! dispatch lives in the operation rather than the transport, so a custom
//! transport gets identical error mapping to the bundled clients. The three
//! content operations implement [`crate::Operation`] directly, because their
//! bodies are hand-parsed rather than deserialised.
mod collection;
mod content;
mod invite;
mod log;
mod scene;
mod user;
mod workspace;

pub use collection::{
    CreateCollection, CreateSceneInCollection, DeleteCollection, GetCollection,
    ListCollectionScenes, ListCollections, UpdateCollection,
};
pub use content::{GetSceneContent, PatchSceneContent, ReplaceSceneContent};
pub use invite::{CreateInvite, DeleteInvite, GetInvite, ListInvites, UpdateInvite};
pub use log::GetLogs;
pub use scene::{CreateScene, DeleteScene, GetScene, ListScenes, UpdateScene};
pub use user::{DeleteUser, GetUser, ListUsers, UpdateUser};
pub use workspace::{GetWorkspace, UpdateWorkspace};
