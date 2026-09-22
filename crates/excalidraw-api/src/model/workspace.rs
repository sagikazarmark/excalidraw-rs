use crate::{Error, Timestamp, de, model::Extra};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

de::open_string_enum! {
    /// A workspace membership role.
    Role {
        Member => "member",
        Admin => "admin",
    }
}

de::open_string_enum! {
    /// Billing state of the workspace.
    SubscriptionStatus {
        PaymentFailed => "payment_failed",
        Active => "active",
        Canceled => "canceled",
        Trialing => "trialing",
        Overdue => "overdue",
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub users: Vec<String>,
    /// User id to role.
    pub roles: BTreeMap<String, Role>,
    pub created: Timestamp,
    /// `format: uri`.
    pub picture: Option<String>,
    pub subscription_status: SubscriptionStatus,
    pub creator: Option<String>,
    pub updater: Option<String>,
    pub will_cancel_at: Option<Timestamp>,

    #[serde(default)]
    pub api_version: Option<String>,
    #[serde(default)]
    pub blocked_users: Option<Vec<String>>,
    /// Published as `enum[null, 1, 2]` with no documented meaning.
    #[serde(default)]
    pub r#type: Option<i64>,
    #[serde(default)]
    pub preferences: Option<WorkspacePreferences>,
    #[serde(default, deserialize_with = "de::opt_count")]
    pub flags: Option<u64>,
    #[serde(default)]
    pub is_deleted: Option<bool>,
    #[serde(default, deserialize_with = "de::opt_count")]
    pub deleted_at: Option<u64>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePreferences {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_ai: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_auto_scene_naming: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_email_editing: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_scene_sharing: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_api_keys_enabled: Option<bool>,
}

/// Body of `PATCH /workspaces`.
///
/// `picture` is a double option because the field is optional *and* nullable:
/// `Some(None)` clears it, `None` leaves it alone.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<Option<String>>,
}

impl WorkspacePatch {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
    /// Set the picture URL.
    pub fn picture(mut self, picture: impl Into<String>) -> Self {
        self.picture = Some(Some(picture.into()));
        self
    }
    /// Send an explicit null, clearing the picture.
    pub fn clear_picture(mut self) -> Self {
        self.picture = Some(None);
        self
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.name.is_none() && self.picture.is_none() {
            return Err(Error::invalid(
                "workspace patch",
                "set at least one of name or picture",
            ));
        }
        Ok(())
    }
}
