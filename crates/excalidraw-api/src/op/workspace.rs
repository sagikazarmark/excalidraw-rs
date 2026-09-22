//! Workspace-level operations.
use crate::{
    Error, JsonOperation, Method, Request,
    model::{Workspace, WorkspacePatch},
};

/// `GET /workspaces` — the workspace the API key belongs to.
///
/// A key is scoped to one workspace and cannot reach another.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GetWorkspace;

impl JsonOperation for GetWorkspace {
    type Output = Workspace;
    fn request(&self) -> Result<Request, Error> {
        Ok(Request::new(Method::Get, "/workspaces"))
    }
}

/// `PATCH /workspaces` — name and picture only.
#[derive(Clone, Debug, PartialEq)]
pub struct UpdateWorkspace {
    pub patch: WorkspacePatch,
}

impl JsonOperation for UpdateWorkspace {
    type Output = Workspace;
    fn request(&self) -> Result<Request, Error> {
        self.patch.validate()?;
        Request::new(Method::Patch, "/workspaces").json(&self.patch)
    }
}
