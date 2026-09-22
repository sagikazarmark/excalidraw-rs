//! Scene metadata operations.
use crate::{
    CollectionId, Error, JsonOperation, Method, Page, PageRequest, Request, SceneId,
    model::{Ack, NewScene, ScenePatch, SceneRecord},
    request::segment,
};

fn scene_path(scene: &SceneId) -> String {
    format!("/scenes/{}", segment(scene.as_str()))
}

/// `GET /scenes` — a paginated list, optionally narrowed to one collection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListScenes {
    pub page: PageRequest,
    pub collection: Option<CollectionId>,
}

impl JsonOperation for ListScenes {
    type Output = Page<SceneRecord>;
    fn request(&self) -> Result<Request, Error> {
        self.page.validate()?;
        Ok(Request::new(Method::Get, "/scenes")
            .maybe_query("limit", self.page.limit)
            .maybe_query("offset", self.page.offset)
            .maybe_query("collectionId", self.collection.as_ref().map(|c| c.as_str())))
    }
}

/// `POST /scenes` — create a scene in the named collection.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateScene {
    pub scene: NewScene,
}

impl JsonOperation for CreateScene {
    type Output = SceneRecord;
    fn request(&self) -> Result<Request, Error> {
        self.scene.validate()?;
        if self.scene.collection.is_none() {
            return Err(Error::invalid(
                "scene collection",
                "POST /scenes requires a collection; use NewScene::new, or \
                 CreateSceneInCollection for the collection-scoped route",
            ));
        }
        Request::new(Method::Post, "/scenes").json(&self.scene)
    }
}

/// `GET /scenes/{sceneId}` — metadata and sharing links, not content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetScene {
    pub scene: SceneId,
}

impl JsonOperation for GetScene {
    type Output = SceneRecord;
    fn request(&self) -> Result<Request, Error> {
        Ok(Request::new(Method::Get, scene_path(&self.scene)))
    }
}

/// `PATCH /scenes/{sceneId}` — rename, pin, or move between collections.
#[derive(Clone, Debug, PartialEq)]
pub struct UpdateScene {
    pub scene: SceneId,
    pub patch: ScenePatch,
}

impl JsonOperation for UpdateScene {
    type Output = SceneRecord;
    fn request(&self) -> Result<Request, Error> {
        self.patch.validate()?;
        Request::new(Method::Patch, scene_path(&self.scene)).json(&self.patch)
    }
}

/// `DELETE /scenes/{sceneId}` — a trash move, not a permanent delete.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteScene {
    pub scene: SceneId,
}

impl JsonOperation for DeleteScene {
    type Output = Ack;
    fn request(&self) -> Result<Request, Error> {
        Ok(Request::new(Method::Delete, scene_path(&self.scene)))
    }
}
