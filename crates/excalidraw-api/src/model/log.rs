use crate::{Timestamp, de, model::Extra};
use serde::Deserialize;

de::open_string_enum! {
    AppSource {
        Admin => "admin",
        ExcalidrawPlus => "excalidraw-plus",
    }
}

de::open_string_enum! {
    SourceType {
        User => "user",
        Api => "api",
        Crontab => "crontab",
        Webhook => "webhook",
        Guest => "guest",
    }
}

/// One audit log entry.
///
/// This is the only model with snake_case wire keys. It deliberately carries no
/// `rename_all = "camelCase"`: adding one would silently fail to decode.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct LogEntry {
    /// `format: uuid`.
    pub id: String,
    /// Unconstrained on the entry, unlike the `action` query filter.
    pub action: String,
    /// Unconstrained on the entry, unlike the `operation` query filter.
    pub operation: String,
    /// Arbitrary JSON, or null: scalar, object or array.
    pub details: Option<serde_json::Value>,
    pub created_at: Timestamp,
    pub ip_address: Option<String>,
    pub user_id: Option<String>,
    pub workspace_id: Option<String>,
    pub app_source: AppSource,
    pub source_type: SourceType,
    pub source_id: Option<String>,
    /// A signed 32-bit range in the published schema, not an HTTP status range.
    pub status: i32,

    #[serde(default)]
    pub user_email: Option<String>,
    #[serde(default)]
    pub user_full_name: Option<String>,
    #[serde(default)]
    pub user_picture: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Response of `GET /logs`, which does not use offset pagination.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogPage {
    pub logs: Vec<LogEntry>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub available_actions: Vec<String>,

    /// The schema marks these required while their descriptions say "only when
    /// using page parameter". Treated as optional; the conflict is unresolved.
    #[serde(default, deserialize_with = "de::opt_count")]
    pub total_count: Option<u64>,
    #[serde(default, deserialize_with = "de::opt_count")]
    pub total_pages: Option<u64>,
    #[serde(default, deserialize_with = "de::opt_count")]
    pub current_page: Option<u64>,
    #[serde(flatten)]
    pub extra: Extra,
}
