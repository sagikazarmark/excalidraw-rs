//! Every documented operation, built once, for the suites that need all of them.
//!
//! Two suites hand-wrote the same facts and never met: `openapi_snapshot.rs`
//! lists what the pinned artifact publishes, `requests.rs` pins what each op
//! builds. Neither asserted the crate implements what the artifact documents.
//! This is the one list both now read, so an operation cannot be added, dropped
//! or re-pathed in only one of them.
//!
//! Identities are sentinels chosen so a concrete path can be turned back into
//! the artifact's template. `CollectionId::PRIVATE` is deliberately avoided:
//! two operations refuse it, and this list is about paths, not guards.
#![allow(dead_code)]

use excalidraw_api::{
    CollectionId, InviteId, LogQuery, Method, Operation, PageRequest, SceneId, UserId, model, op,
    scene_content::{self, ElementIds, PatchFields, PlusProjection},
};
use excalidraw_document::Document;
use serde_json::json;

pub const SCENE: &str = "sentinelscene";
pub const COLLECTION: &str = "sentinelcollection";
pub const INVITE: &str = "sentinelinvite";
pub const USER: &str = "sentineluser";

/// One operation as the crate builds it.
pub struct Built {
    pub name: &'static str,
    pub method: Method,
    /// Request path only, sentinels still in place. Query is excluded: the
    /// artifact documents parameters separately from the path.
    pub path: String,
    pub body: Option<Vec<u8>>,
}

impl Built {
    /// The path as the artifact spells it: base prefix restored, sentinel
    /// identities turned back into their template names.
    pub fn template(&self) -> String {
        format!("/api/v1{}", self.path)
            .replace(SCENE, "{sceneId}")
            .replace(COLLECTION, "{collectionId}")
            .replace(INVITE, "{inviteId}")
            .replace(USER, "{userId}")
    }
}

fn scene() -> SceneId {
    SceneId::new(SCENE).unwrap()
}
fn collection() -> CollectionId {
    CollectionId::new(COLLECTION).unwrap()
}
fn invite() -> InviteId {
    InviteId::new(INVITE).unwrap()
}
fn user() -> UserId {
    UserId::new(USER).unwrap()
}

fn document() -> Document {
    Document::from_value(json!({
        "type": "excalidraw", "version": 2, "source": "all-ops",
        "elements": [], "appState": { "viewBackgroundColor": "#ffffff" }, "files": {}
    }))
    .unwrap()
}

/// All 27 documented operations, in artifact order.
///
/// Formatting is suppressed: this is a table, and one operation per line
/// is what makes a missing or duplicated row visible at a glance.
#[rustfmt::skip]
pub fn all() -> Vec<Built> {
    let document = document();
    let (replacement, _) =
        scene_content::replacement_from_document(&document, &PlusProjection::strict()).unwrap();
    let patch =
        scene_content::patch_from(&document, PatchFields::elements(), ElementIds::AllowProvisional)
            .unwrap();

    let mut built = Vec::new();
    macro_rules! add {
        ($name:literal, $op:expr) => {{
            let request = $op.request().expect(concat!($name, " builds a request"));
            built.push(Built {
                name: $name,
                method: request.method,
                path: request.path,
                body: request.body,
            });
        }};
    }

    add!("ListCollections", op::ListCollections { page: PageRequest::new() });
    add!("CreateCollection", op::CreateCollection { collection: model::NewCollection::new("Design") });
    add!("GetCollection", op::GetCollection { collection: collection() });
    add!("UpdateCollection", op::UpdateCollection { collection: collection(), patch: model::CollectionPatch::name("Renamed") });
    add!("DeleteCollection", op::DeleteCollection { collection: collection() });
    add!("ListCollectionScenes", op::ListCollectionScenes { collection: collection(), page: PageRequest::new() });
    add!("CreateSceneInCollection", op::CreateSceneInCollection { collection: collection(), scene: model::NewScene::in_path("Nested") });

    add!("ListScenes", op::ListScenes { page: PageRequest::new(), collection: None });
    add!("CreateScene", op::CreateScene { scene: model::NewScene::new("Q3 plan", collection()) });
    add!("GetScene", op::GetScene { scene: scene() });
    add!("UpdateScene", op::UpdateScene { scene: scene(), patch: model::ScenePatch::new().name("Renamed") });
    add!("DeleteScene", op::DeleteScene { scene: scene() });

    add!("GetSceneContent", op::GetSceneContent { scene: scene() });
    add!("ReplaceSceneContent", op::ReplaceSceneContent { scene: scene(), body: replacement });
    add!("PatchSceneContent", op::PatchSceneContent { scene: scene(), body: patch });

    add!("GetWorkspace", op::GetWorkspace);
    add!("UpdateWorkspace", op::UpdateWorkspace { patch: model::WorkspacePatch::new().name("Studio") });

    add!("ListUsers", op::ListUsers { page: PageRequest::new() });
    add!("GetUser", op::GetUser { user: user() });
    add!("UpdateUser", op::UpdateUser { user: user(), patch: model::UserPatch::new().role(model::Role::Admin) });
    add!("DeleteUser", op::DeleteUser { user: user() });

    add!("ListInvites", op::ListInvites { page: PageRequest::new() });
    add!("CreateInvite", op::CreateInvite { invite: model::NewInvite::email(model::Role::Member, "a@example.com") });
    add!("GetInvite", op::GetInvite { invite: invite() });
    add!("UpdateInvite", op::UpdateInvite { invite: invite(), patch: model::InvitePatch::new().max_uses(model::MaxUses::Limited(5)) });
    add!("DeleteInvite", op::DeleteInvite { invite: invite() });

    add!("GetLogs", op::GetLogs { query: LogQuery::new() });

    built
}
