//! The three scene-content operations.
//!
//! Request and response bodies are `excalidraw_document::plus` types, unchanged.
//! Conversion between them and preserving documents lives in
//! [`crate::scene_content`].
use crate::{
    Error, HeaderLookup, Method, Operation, Request, SceneId, error::expect_ok, plus,
    request::segment,
};

fn content_path(scene: &SceneId) -> String {
    format!("/scenes/{}/content", segment(scene.as_str()))
}

fn decode_content(
    status: u16,
    headers: &dyn HeaderLookup,
    body: &[u8],
) -> Result<plus::SceneContent, Error> {
    let body = expect_ok(status, headers, body)?;
    plus::SceneContent::from_slice(body).map_err(Error::Content)
}

/// `GET /scenes/{sceneId}/content`.
///
/// The response may carry `filesFailedToEmbed`, meaning it is **not**
/// self-contained. Check [`plus::SceneContent::files_failed_to_embed`] before
/// using it as an upload source, or let
/// [`crate::scene_content::into_replacement`] check for you.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetSceneContent {
    pub scene: SceneId,
}

impl Operation for GetSceneContent {
    type Output = plus::SceneContent;
    fn request(&self) -> Result<Request, Error> {
        Ok(Request::new(Method::Get, content_path(&self.scene)))
    }
    fn decode(
        &self,
        status: u16,
        headers: &dyn HeaderLookup,
        body: &[u8],
    ) -> Result<Self::Output, Error> {
        decode_content(status, headers, body)
    }
}

/// `PUT /scenes/{sceneId}/content` — authoritative full replacement.
///
/// Elements absent from the body are removed, connected editors are forced to
/// reload rather than reconciling, and any `sceneVersion` in the body is ignored
/// and recomputed by the server. Use [`PatchSceneContent`] to add or update
/// without replacing.
#[derive(Clone, Debug)]
pub struct ReplaceSceneContent {
    pub scene: SceneId,
    pub body: plus::ReplaceSceneContent,
}

impl Operation for ReplaceSceneContent {
    type Output = plus::SceneContent;
    fn request(&self) -> Result<Request, Error> {
        let bytes = self.body.to_vec().map_err(Error::Content)?;
        Ok(Request::new(Method::Put, content_path(&self.scene)).bytes(bytes))
    }
    fn decode(
        &self,
        status: u16,
        headers: &dyn HeaderLookup,
        body: &[u8],
    ) -> Result<Self::Output, Error> {
        decode_content(status, headers, body)
    }
    /// Never. The same body leaves the same content, but each `PUT` is a new
    /// authoritative replacement: it advances `contentEpoch` and forces
    /// connected editors to reload. After an ambiguous `5xx` the first attempt
    /// may already have landed, and a replay after the backoff would also
    /// erase whatever a collaborator wrote in between — a concurrent write the
    /// caller never sees and an epoch check cannot attribute.
    fn replayable(&self, _: &Request) -> bool {
        false
    }
}

/// `PATCH /scenes/{sceneId}/content` — server-side merge.
///
/// Elements merge by id, higher `version` winning with `versionNonce` breaking
/// ties; omitted elements survive; `isDeleted: true` soft-deletes. `appState`
/// shallow-merges. Files are added, or replaced by id. The merge starts from the
/// current stored content and may temporarily diverge under concurrent edits.
///
/// The tie-break direction and equal-version equal-nonce behaviour are not
/// published, and this crate implements no local reconciliation.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchSceneContent {
    pub scene: SceneId,
    pub body: plus::PatchSceneContent,
}

impl Operation for PatchSceneContent {
    type Output = plus::SceneContent;
    fn request(&self) -> Result<Request, Error> {
        let bytes = self.body.to_vec().map_err(Error::Content)?;
        Ok(Request::new(Method::Patch, content_path(&self.scene)).bytes(bytes))
    }
    fn decode(
        &self,
        status: u16,
        headers: &dyn HeaderLookup,
        body: &[u8],
    ) -> Result<Self::Output, Error> {
        decode_content(status, headers, body)
    }
}
