//! Workspace member operations.
use crate::{
    Error, JsonOperation, Method, Page, PageRequest, Request, UserId,
    model::{Ack, UserPatch, WorkspaceUser},
    request::segment,
};

fn user_path(user: &UserId) -> String {
    format!("/workspaces/users/{}", segment(user.as_str()))
}

/// `GET /workspaces/users`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListUsers {
    pub page: PageRequest,
}

impl JsonOperation for ListUsers {
    type Output = Page<WorkspaceUser>;
    fn request(&self) -> Result<Request, Error> {
        self.page.validate()?;
        Ok(Request::new(Method::Get, "/workspaces/users")
            .maybe_query("limit", self.page.limit)
            .maybe_query("offset", self.page.offset))
    }
}

/// `GET /workspaces/users/{userId}`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetUser {
    pub user: UserId,
}

impl JsonOperation for GetUser {
    type Output = WorkspaceUser;
    fn request(&self) -> Result<Request, Error> {
        Ok(Request::new(Method::Get, user_path(&self.user)))
    }
}

/// `PATCH /workspaces/users/{userId}`.
///
/// `preferences` is sent as a whole object: no merge semantics are published for
/// it, so this crate does not read-modify-write on the caller's behalf.
#[derive(Clone, Debug, PartialEq)]
pub struct UpdateUser {
    pub user: UserId,
    pub patch: UserPatch,
}

impl JsonOperation for UpdateUser {
    type Output = WorkspaceUser;
    fn request(&self) -> Result<Request, Error> {
        self.patch.validate()?;
        Request::new(Method::Patch, user_path(&self.user)).json(&self.patch)
    }
}

/// `DELETE /workspaces/users/{userId}` — removes the member from the workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteUser {
    pub user: UserId,
}

impl JsonOperation for DeleteUser {
    type Output = Ack;
    fn request(&self) -> Result<Request, Error> {
        Ok(Request::new(Method::Delete, user_path(&self.user)))
    }
}
