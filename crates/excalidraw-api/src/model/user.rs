use crate::{
    Error, Timestamp, de,
    model::{Extra, Role},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

de::open_string_enum! {
    SceneOrder {
        Manual => "manual",
        Created => "created",
        Updated => "updated",
        Name => "name",
    }
}

de::open_string_enum! {
    InitialRedirect {
        Dashboard => "dashboard",
        LastEditedScene => "lastEditedScene",
        LastVisitedScene => "lastVisitedScene",
        PrivateCollection => "privateCollection",
    }
}

de::open_string_enum! {
    Theme {
        Light => "light",
        Dark => "dark",
        System => "system",
    }
}

de::open_string_enum! {
    /// The twelve locales the published schema lists.
    Locale {
        En => "en", Cs => "cs", De => "de", Es => "es", Fr => "fr", Hu => "hu",
        It => "it", Ja => "ja", Nl => "nl", Pl => "pl", Pt => "pt", Ru => "ru",
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceUser {
    pub id: String,
    pub uid: String,
    pub name: String,
    /// `format: email`.
    pub email: String,
    pub picture: Option<String>,
    pub created: Timestamp,
    pub updated: Option<Timestamp>,
    /// Team name to member ids.
    pub workspace_teams: BTreeMap<String, Vec<String>>,
    pub role: Role,

    #[serde(default)]
    pub is_initialized: Option<bool>,
    /// A plain string here, unlike the workspace's enumerated status.
    #[serde(default)]
    pub subscription_status: Option<String>,
    #[serde(default)]
    pub last_active: Option<Timestamp>,
    #[serde(default)]
    pub last_active_refreshed_at: Option<Timestamp>,
    #[serde(default)]
    pub preferences: Option<UserPreferences>,
    #[serde(default)]
    pub last_visited_scenes: Option<Vec<String>>,
    #[serde(default)]
    pub last_edited_scenes: Option<Vec<String>>,
    #[serde(default)]
    pub rate_limits: Option<UserRateLimits>,
    #[serde(default, deserialize_with = "de::opt_count")]
    pub library_version: Option<u64>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Every field is optional. Sent as a whole object by `PATCH`: the artifact
/// publishes no merge semantics for it, so this crate does not read-modify-write
/// on the caller's behalf.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferences {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oss_auto_redirect: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_order: Option<SceneOrder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_redirect: Option<InitialRedirect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_deprecated_fonts: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_auto_scene_naming_for_private_collections: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_comment_email_notifications: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_to_talk_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<Locale>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale_suggestion_dismissed: Option<bool>,
}

impl UserPreferences {
    /// The open enums decode an unlisted value so a read never fails, but the
    /// `PATCH` schema pins each to its published list: one read back as
    /// `Unknown` must not be written.
    pub(crate) fn validate(&self) -> Result<(), Error> {
        for (what, unlisted) in [
            (
                "user preference sceneOrder",
                self.scene_order
                    .as_ref()
                    .filter(|v| v.wire_name().is_none())
                    .map(|v| v.as_str()),
            ),
            (
                "user preference initialRedirect",
                self.initial_redirect
                    .as_ref()
                    .filter(|v| v.wire_name().is_none())
                    .map(|v| v.as_str()),
            ),
            (
                "user preference theme",
                self.theme
                    .as_ref()
                    .filter(|v| v.wire_name().is_none())
                    .map(|v| v.as_str()),
            ),
            (
                "user preference locale",
                self.locale
                    .as_ref()
                    .filter(|v| v.wire_name().is_none())
                    .map(|v| v.as_str()),
            ),
        ] {
            if let Some(value) = unlisted {
                return Err(Error::invalid(
                    what,
                    format!("{value:?} is not a published value"),
                ));
            }
        }
        Ok(())
    }
}

/// AI feature usage, keyed by an opaque window identifier.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserRateLimits {
    #[serde(default)]
    pub text_to_diagram: Option<BTreeMap<String, RateWindow>>,
    #[serde(default)]
    pub diagram_to_code: Option<BTreeMap<String, RateWindow>>,
    #[serde(default)]
    pub name_scene: Option<BTreeMap<String, RateWindow>>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RateWindow {
    #[serde(deserialize_with = "de::count")]
    pub last_used: u64,
    #[serde(deserialize_with = "de::count")]
    pub first_used: u64,
    #[serde(deserialize_with = "de::count")]
    pub count: u64,
}

/// Body of `PATCH /workspaces/users/{userId}`.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_teams: Option<BTreeMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferences: Option<UserPreferences>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
}

impl UserPatch {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
    pub fn picture(mut self, picture: impl Into<String>) -> Self {
        self.picture = Some(Some(picture.into()));
        self
    }
    pub fn clear_picture(mut self) -> Self {
        self.picture = Some(None);
        self
    }
    pub fn role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }
    pub fn preferences(mut self, preferences: UserPreferences) -> Self {
        self.preferences = Some(preferences);
        self
    }
    pub fn teams(mut self, teams: BTreeMap<String, Vec<String>>) -> Self {
        self.workspace_teams = Some(teams);
        self
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.name.is_none()
            && self.picture.is_none()
            && self.workspace_teams.is_none()
            && self.preferences.is_none()
            && self.role.is_none()
        {
            return Err(Error::invalid(
                "user patch",
                "set at least one field to update",
            ));
        }
        // A role read back as an undocumented value must not be written.
        if let Some(role) = &self.role
            && role.wire_name().is_none()
        {
            return Err(Error::invalid(
                "user role",
                format!("must be member or admin, got {role}"),
            ));
        }
        if let Some(preferences) = &self.preferences {
            preferences.validate()?;
        }
        Ok(())
    }
}
