//! Collection operations, including the two collection-scoped scene routes.
use crate::{
    CollectionId, Error, JsonOperation, Method, Page, PageRequest, Request,
    model::{Ack, Collection, CollectionPatch, NewCollection, NewScene, SceneRecord},
    request::segment,
};

fn collection_path(collection: &CollectionId) -> String {
    format!("/collections/{}", segment(collection.as_str()))
}

/// `GET /collections`.
///
/// A personal API key also receives the owner's virtual private collection,
/// with id [`CollectionId::PRIVATE`]. A workspace key never does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListCollections {
    pub page: PageRequest,
}

impl JsonOperation for ListCollections {
    type Output = Page<Collection>;
    fn request(&self) -> Result<Request, Error> {
        self.page.validate()?;
        Ok(Request::new(Method::Get, "/collections")
            .maybe_query("limit", self.page.limit)
            .maybe_query("offset", self.page.offset))
    }
}

/// `POST /collections` — always creates a regular shared collection.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateCollection {
    pub collection: NewCollection,
}

impl JsonOperation for CreateCollection {
    type Output = Collection;
    fn request(&self) -> Result<Request, Error> {
        self.collection.validate()?;
        Request::new(Method::Post, "/collections").json(&self.collection)
    }
}

/// `GET /collections/{collectionId}`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetCollection {
    pub collection: CollectionId,
}

impl JsonOperation for GetCollection {
    type Output = Collection;
    fn request(&self) -> Result<Request, Error> {
        Ok(Request::new(Method::Get, collection_path(&self.collection)))
    }
}

/// `PATCH /collections/{collectionId}`.
///
/// The private collection cannot be renamed.
#[derive(Clone, Debug, PartialEq)]
pub struct UpdateCollection {
    pub collection: CollectionId,
    pub patch: CollectionPatch,
}

impl JsonOperation for UpdateCollection {
    type Output = Collection;
    fn request(&self) -> Result<Request, Error> {
        self.patch.validate()?;
        if self.collection.is_private() {
            return Err(Error::invalid(
                "collection",
                "the virtual private collection cannot be renamed",
            ));
        }
        Request::new(Method::Patch, collection_path(&self.collection)).json(&self.patch)
    }
}

/// `DELETE /collections/{collectionId}` — a trash move, not a permanent delete.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteCollection {
    pub collection: CollectionId,
}

impl JsonOperation for DeleteCollection {
    type Output = Ack;
    fn request(&self) -> Result<Request, Error> {
        if self.collection.is_private() {
            return Err(Error::invalid(
                "collection",
                "the virtual private collection cannot be deleted",
            ));
        }
        Ok(Request::new(
            Method::Delete,
            collection_path(&self.collection),
        ))
    }
}

/// `GET /collections/{collectionId}/scenes`.
#[derive(Clone, Debug, PartialEq)]
pub struct ListCollectionScenes {
    pub collection: CollectionId,
    pub page: PageRequest,
}

impl JsonOperation for ListCollectionScenes {
    type Output = Page<SceneRecord>;
    fn request(&self) -> Result<Request, Error> {
        self.page.validate()?;
        Ok(Request::new(
            Method::Get,
            format!("{}/scenes", collection_path(&self.collection)),
        )
        .maybe_query("limit", self.page.limit)
        .maybe_query("offset", self.page.offset))
    }
}

/// `POST /collections/{collectionId}/scenes`.
///
/// The collection comes from the path, so the body carries only `name` and
/// `pinned`. Build it with [`NewScene::in_path`].
#[derive(Clone, Debug, PartialEq)]
pub struct CreateSceneInCollection {
    pub collection: CollectionId,
    pub scene: NewScene,
}

impl JsonOperation for CreateSceneInCollection {
    type Output = SceneRecord;
    fn request(&self) -> Result<Request, Error> {
        self.scene.validate()?;
        if self.scene.collection.is_some() {
            return Err(Error::invalid(
                "scene collection",
                "this route takes the collection from the path; build the body \
                 with NewScene::in_path",
            ));
        }
        Request::new(
            Method::Post,
            format!("{}/scenes", collection_path(&self.collection)),
        )
        .json(&self.scene)
    }
}
