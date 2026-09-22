use crate::{
    Error, Timestamp, de,
    model::{Extra, Role},
};
use serde::{Deserialize, Serialize};

de::open_string_enum! {
    InviteType {
        Link => "link",
        Email => "email",
    }
}

de::open_string_enum! {
    InviteStatus {
        Pending => "pending",
        Redeemed => "redeemed",
        Rejected => "rejected",
    }
}

/// Published as `anyOf[integer | "unlimited"]`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaxUses {
    Limited(u64),
    Unlimited,
}

impl<'de> Deserialize<'de> for MaxUses {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::String(value) if value == "unlimited" => Ok(Self::Unlimited),
            serde_json::Value::String(value) => Err(D::Error::custom(format!(
                "expected a number or \"unlimited\", got {value:?}"
            ))),
            serde_json::Value::Number(number) => match number.as_u64() {
                Some(value) if value <= de::MAX_SAFE_INTEGER => Ok(Self::Limited(value)),
                _ => Err(D::Error::custom(format!(
                    "expected a nonnegative integer within the published maximum, got {number}"
                ))),
            },
            other => Err(D::Error::custom(format!(
                "expected a number or \"unlimited\", got {other}"
            ))),
        }
    }
}

impl Serialize for MaxUses {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Limited(value) => serializer.serialize_u64(*value),
            Self::Unlimited => serializer.serialize_str("unlimited"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    pub id: String,
    pub created: Timestamp,
    pub r#type: InviteType,
    pub status: InviteStatus,
    pub email: Option<String>,
    pub role: Role,
    pub resolved_at: Option<Timestamp>,
    pub redeemed_by: Option<String>,

    #[serde(default)]
    pub max_uses: Option<MaxUses>,
    #[serde(default, deserialize_with = "de::opt_count")]
    pub uses: Option<u64>,
    #[serde(default)]
    pub restricted_domains: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Body of `POST /workspaces/invites`.
///
/// The published schema is an `anyOf` of two closed objects. They are modelled as
/// explicit constructors rather than an untagged enum so the intended arm is
/// chosen by the caller, not inferred from which fields happen to be set.
#[derive(Clone, Debug, PartialEq)]
pub enum NewInvite {
    /// An invitation addressed to one email.
    Email { role: Role, email: String },
    /// A shareable link invitation.
    Link {
        role: Role,
        max_uses: Option<MaxUses>,
        restricted_domains: Option<Vec<String>>,
    },
}

impl NewInvite {
    pub fn email(role: Role, email: impl Into<String>) -> Self {
        Self::Email {
            role,
            email: email.into(),
        }
    }
    pub fn link(role: Role) -> Self {
        Self::Link {
            role,
            max_uses: None,
            restricted_domains: None,
        }
    }
    pub fn max_uses(mut self, uses: MaxUses) -> Self {
        if let Self::Link { max_uses, .. } = &mut self {
            *max_uses = Some(uses);
        }
        self
    }
    pub fn restricted_domains(mut self, domains: Vec<String>) -> Self {
        if let Self::Link {
            restricted_domains, ..
        } = &mut self
        {
            *restricted_domains = Some(domains);
        }
        self
    }
    fn role(&self) -> &Role {
        match self {
            Self::Email { role, .. } | Self::Link { role, .. } => role,
        }
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.role().wire_name().is_none() {
            return Err(Error::invalid(
                "invite role",
                format!("must be member or admin, got {}", self.role()),
            ));
        }
        if let Self::Email { email, .. } = self
            && email.is_empty()
        {
            return Err(Error::invalid("invite email", "must not be empty"));
        }
        Ok(())
    }
}

impl Serialize for NewInvite {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        match self {
            Self::Email { role, email } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("role", role)?;
                map.serialize_entry("email", email)?;
                map.end()
            }
            Self::Link {
                role,
                max_uses,
                restricted_domains,
            } => {
                let len = 1 + max_uses.is_some() as usize + restricted_domains.is_some() as usize;
                let mut map = serializer.serialize_map(Some(len))?;
                map.serialize_entry("role", role)?;
                if let Some(max_uses) = max_uses {
                    map.serialize_entry("maxUses", max_uses)?;
                }
                if let Some(domains) = restricted_domains {
                    map.serialize_entry("restrictedDomains", domains)?;
                }
                map.end()
            }
        }
    }
}

/// Body of `PATCH /workspaces/invites/{inviteId}`.
///
/// `email` and `restricted_domains` are double options: `Some(None)` sends an
/// explicit null, `None` omits the key.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitePatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<MaxUses>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restricted_domains: Option<Option<Vec<String>>>,
}

impl InvitePatch {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(Some(email.into()));
        self
    }
    pub fn clear_email(mut self) -> Self {
        self.email = Some(None);
        self
    }
    pub fn role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }
    pub fn max_uses(mut self, uses: MaxUses) -> Self {
        self.max_uses = Some(uses);
        self
    }
    pub fn restricted_domains(mut self, domains: Vec<String>) -> Self {
        self.restricted_domains = Some(Some(domains));
        self
    }
    pub fn clear_restricted_domains(mut self) -> Self {
        self.restricted_domains = Some(None);
        self
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.email.is_none()
            && self.role.is_none()
            && self.max_uses.is_none()
            && self.restricted_domains.is_none()
        {
            return Err(Error::invalid(
                "invite patch",
                "set at least one field to update",
            ));
        }
        if let Some(role) = &self.role
            && role.wire_name().is_none()
        {
            return Err(Error::invalid(
                "invite role",
                format!("must be member or admin, got {role}"),
            ));
        }
        Ok(())
    }
}
