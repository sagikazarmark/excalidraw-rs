//! Workspace invitation operations.
use crate::{
    Error, InviteId, JsonOperation, Method, Page, PageRequest, Request,
    model::{Ack, Invite, InvitePatch, NewInvite},
    request::segment,
};

fn invite_path(invite: &InviteId) -> String {
    format!("/workspaces/invites/{}", segment(invite.as_str()))
}

/// `GET /workspaces/invites`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListInvites {
    pub page: PageRequest,
}

impl JsonOperation for ListInvites {
    type Output = Page<Invite>;
    fn request(&self) -> Result<Request, Error> {
        self.page.validate()?;
        Ok(Request::new(Method::Get, "/workspaces/invites")
            .maybe_query("limit", self.page.limit)
            .maybe_query("offset", self.page.offset))
    }
}

/// `POST /workspaces/invites` — an email invitation or a shareable link.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateInvite {
    pub invite: NewInvite,
}

impl JsonOperation for CreateInvite {
    type Output = Invite;
    fn request(&self) -> Result<Request, Error> {
        self.invite.validate()?;
        Request::new(Method::Post, "/workspaces/invites").json(&self.invite)
    }
}

/// `GET /workspaces/invites/{inviteId}`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetInvite {
    pub invite: InviteId,
}

impl JsonOperation for GetInvite {
    type Output = Invite;
    fn request(&self) -> Result<Request, Error> {
        Ok(Request::new(Method::Get, invite_path(&self.invite)))
    }
}

/// `PATCH /workspaces/invites/{inviteId}`.
#[derive(Clone, Debug, PartialEq)]
pub struct UpdateInvite {
    pub invite: InviteId,
    pub patch: InvitePatch,
}

impl JsonOperation for UpdateInvite {
    type Output = Invite;
    fn request(&self) -> Result<Request, Error> {
        self.patch.validate()?;
        Request::new(Method::Patch, invite_path(&self.invite)).json(&self.patch)
    }
}

/// `DELETE /workspaces/invites/{inviteId}` — revokes the invitation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteInvite {
    pub invite: InviteId,
}

impl JsonOperation for DeleteInvite {
    type Output = Ack;
    fn request(&self) -> Result<Request, Error> {
        Ok(Request::new(Method::Delete, invite_path(&self.invite)))
    }
}
