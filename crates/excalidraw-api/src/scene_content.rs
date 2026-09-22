//! Conversion policy between preserving documents and Plus content bodies.
//!
//! This is the crate's only original logic. Every function here is explicit and
//! reporting: none silently repairs, prunes or completes its input.
use crate::Error;
use excalidraw_document::{Change, Document, plus};
use serde_json::Value;

/// The `appState` keys the published Plus transport schema carries. Everything
/// else is rejected by `excalidraw_document::plus` rather than stripped.
///
/// Re-exported from [`excalidraw_document::plus::APP_STATE_KEYS`], the
/// enforcement point that actually rejects other keys, so this crate cannot
/// drift from it.
pub use excalidraw_document::plus::APP_STATE_KEYS as PLUS_APP_STATE_KEYS;

/// Whether an upload may proceed when the download could not embed every file
/// the scene references.
///
/// `GET /scenes/{id}/content` reports unembeddable files in `filesFailedToEmbed`.
/// Replacing a scene with such a response sends a `files` map missing records
/// that elements still reference.
///
/// Confirmed 2026-09-21: the service **rejects** that body with
/// `400 Referenced file <id> is missing from scene files`, and the check covers
/// tombstoned elements too. So this guard prevents a failed request rather than
/// silent image loss: it fails locally, before serialising and uploading a body
/// the server will refuse, and names the files responsible. There is deliberately
/// no `Default`, because proceeding is still a decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Embedding {
    /// Fail when the download reported files it could not embed.
    RequireComplete,
    /// Proceed anyway. Referenced-but-unembedded files will be absent from the
    /// replacement body, which the service is expected to reject unless nothing
    /// references them any more.
    AllowMissing,
}

/// What a projection may change when adapting a native document to the Plus
/// transport profile. There is deliberately no `Default`: each field is a
/// decision about losing or inventing data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlusProjection {
    /// Remove `appState` keys outside [`PLUS_APP_STATE_KEYS`]. Native local
    /// export retains `gridSize`, `gridStep` and `gridModeEnabled`, which the
    /// transport schema does not carry.
    pub prune_app_state: bool,
    /// Supply `appState.viewBackgroundColor` when the document has none.
    pub default_view_background: Option<String>,
    /// Replace the document's `source`. `None` preserves it.
    pub source: Option<String>,
}

impl PlusProjection {
    /// Change nothing. A document that is not already a legal Plus body fails
    /// with the offending path instead of being adjusted.
    pub fn strict() -> Self {
        Self {
            prune_app_state: false,
            default_view_background: None,
            source: None,
        }
    }
    /// Prune undocumented `appState` keys and supply a white background when the
    /// document has none. Every change is still reported.
    pub fn lenient() -> Self {
        Self {
            prune_app_state: true,
            default_view_background: Some("#ffffff".to_owned()),
            source: None,
        }
    }
}

fn change(path: &str, before: Option<Value>, after: Option<Value>) -> Change {
    Change {
        path: path.to_owned(),
        before,
        after,
    }
}

/// Turn a downloaded scene into an authoritative replacement body.
///
/// The document inside a [`plus::SceneContent`] is already exactly the `PUT`
/// root: `from_slice` removed `sceneVersion` and `filesFailedToEmbed`, and
/// nothing else is added, removed or renamed here. That is what makes a
/// download/upload cycle exact.
///
/// # Element ids are not stable across a write
///
/// Confirmed against the live service on 2026-09-21: the server rewrites element
/// ids that are not in Excalidraw's own 21-character form, and rewrites
/// `containerId` and `boundElements` to match, so bound labels survive. An id you
/// sent will often not be the id you read back. Treat the returned content as
/// canonical and re-read rather than assuming your ids persisted.
pub fn into_replacement(
    content: plus::SceneContent,
    embedding: Embedding,
) -> Result<plus::ReplaceSceneContent, Error> {
    if embedding == Embedding::RequireComplete
        && let Some(failed) = content.files_failed_to_embed()
        && !failed.is_empty()
    {
        return Err(Error::invalid(
            "incomplete embedding",
            format!(
                "the download could not embed {} referenced file(s) ({}); a replacement built \
                 from it omits those records, and the service rejects that with \
                 \"Referenced file ... is missing from scene files\". Re-download, or pass \
                 Embedding::AllowMissing to send it anyway",
                failed.len(),
                failed.join(", ")
            ),
        ));
    }
    Ok(plus::ReplaceSceneContent::new(content.document().clone())?)
}

/// Adapt a document that did not come from the API into a replacement body,
/// reporting every change the policy made.
///
/// With [`PlusProjection::strict`] the result either carries no changes or is an
/// error naming the offending path. Nothing is ever changed without a report.
pub fn replacement_from_document(
    document: &Document,
    policy: &PlusProjection,
) -> Result<(plus::ReplaceSceneContent, Vec<Change>), Error> {
    let mut root = document.as_object().clone();
    let mut changes = Vec::new();

    if let Some(source) = &policy.source {
        let before = root.get("source").cloned();
        let after = Value::String(source.clone());
        if before.as_ref() != Some(&after) {
            root.insert("source".to_owned(), after.clone());
            changes.push(change("/source", before, Some(after)));
        }
    }

    let state_is_object = root.get("appState").is_some_and(Value::is_object);
    if let Some(color) = &policy.default_view_background {
        let missing = match root.get("appState") {
            Some(Value::Object(state)) => !state.contains_key("viewBackgroundColor"),
            _ => true,
        };
        if missing {
            if !state_is_object {
                let before = root.get("appState").cloned();
                root.insert("appState".to_owned(), Value::Object(Default::default()));
                changes.push(change(
                    "/appState",
                    before,
                    Some(Value::Object(Default::default())),
                ));
            }
            let after = Value::String(color.clone());
            root["appState"]
                .as_object_mut()
                .expect("materialized above")
                .insert("viewBackgroundColor".to_owned(), after.clone());
            changes.push(change("/appState/viewBackgroundColor", None, Some(after)));
        }
    }

    if policy.prune_app_state
        && let Some(Value::Object(state)) = root.get_mut("appState")
    {
        let doomed: Vec<String> = state
            .keys()
            .filter(|key| !PLUS_APP_STATE_KEYS.contains(&key.as_str()))
            .cloned()
            .collect();
        for key in doomed {
            let before = state.remove(&key);
            changes.push(change(&format!("/appState/{key}"), before, None));
        }
    }

    if !root.contains_key("files") {
        root.insert("files".to_owned(), Value::Object(Default::default()));
        changes.push(change(
            "/files",
            None,
            Some(Value::Object(Default::default())),
        ));
    }

    let candidate = Document::from_value(Value::Object(root))?;
    let body = plus::ReplaceSceneContent::new(candidate)?;
    Ok((body, changes))
}

/// The shape of an identifier the service assigns: 21 characters from
/// `A-Za-z0-9_-`.
///
/// Confirmed 2026-09-21: the server rewrites any element id not already in this
/// form, so an id of another shape is provisional whatever the caller intended.
pub fn is_canonical_element_id(id: &str) -> bool {
    id.len() == 21
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Whether a merge request may carry element ids the service has not assigned.
///
/// `PATCH` reconciles by id, and the server rewrites ids that are not already
/// canonical. Submitting an element with a client-chosen id therefore **inserts a
/// duplicate** rather than updating the element you meant, silently and with a
/// `200`. There is deliberately no `Default`: only the caller knows whether an
/// element is an update or an insert.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElementIds {
    /// Every element must already carry a server-assigned id. Use this when the
    /// elements came from a `GET` and you are updating them.
    RequireCanonical,
    /// Ids may be provisional. Non-canonical ones are replaced by the server and
    /// the elements appear as new. Use this when inserting.
    AllowProvisional,
}

impl ElementIds {
    fn check(self, elements: &Value) -> Result<(), Error> {
        if self == Self::AllowProvisional {
            return Ok(());
        }
        let Some(elements) = elements.as_array() else {
            return Ok(());
        };
        let mut offenders = Vec::new();
        for (i, element) in elements.iter().enumerate() {
            let canonical = match element.get("id") {
                Some(Value::String(id)) => is_canonical_element_id(id),
                // A numeric authoring id, or none at all, is provisional by
                // definition: the server will mint one.
                _ => false,
            };
            if !canonical {
                let shown = element
                    .get("id")
                    .map_or_else(|| "<absent>".to_owned(), ToString::to_string);
                offenders.push(format!("/elements/{i} (id {shown})"));
            }
        }
        if offenders.is_empty() {
            return Ok(());
        }
        Err(Error::invalid(
            "provisional element ids",
            format!(
                "PATCH reconciles by id, and the service rewrites ids that are not its own \
                 21-character form, so {} element(s) would be inserted as duplicates instead of \
                 updating anything: {}. Use ids read back from the service, or pass \
                 ElementIds::AllowProvisional if these really are new elements",
                offenders.len(),
                offenders.join(", ")
            ),
        ))
    }
}

/// Which scene-level fields a `PATCH` body carries. At least one is required.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PatchFields {
    pub elements: bool,
    pub app_state: bool,
    pub files: bool,
}

impl PatchFields {
    pub fn elements() -> Self {
        Self {
            elements: true,
            ..Self::default()
        }
    }
    pub fn app_state() -> Self {
        Self {
            app_state: true,
            ..Self::default()
        }
    }
    pub fn files() -> Self {
        Self {
            files: true,
            ..Self::default()
        }
    }
    pub fn with_elements(mut self) -> Self {
        self.elements = true;
        self
    }
    pub fn with_app_state(mut self) -> Self {
        self.app_state = true;
        self
    }
    pub fn with_files(mut self) -> Self {
        self.files = true;
        self
    }
    fn selected(&self) -> Vec<&'static str> {
        let mut keys = Vec::new();
        if self.elements {
            keys.push("elements");
        }
        if self.app_state {
            keys.push("appState");
        }
        if self.files {
            keys.push("files");
        }
        keys
    }
}

/// Build a merge request carrying only the selected scene-level fields.
///
/// `PATCH`'s published root is closed to `elements`, `appState` and `files`, so
/// a full `GET` envelope is not a legal `PATCH` body. There is deliberately no
/// conversion that forwards a response verbatim.
///
/// `ids` decides whether provisional element ids are allowed. Under
/// [`ElementIds::RequireCanonical`] an element whose id the service would rewrite
/// is rejected here, because the merge would insert a duplicate rather than
/// update anything.
///
/// This guard is deliberately absent from the replacement paths: `PUT` replaces
/// the whole scene and rewrites every id and reference together, so provisional
/// ids are correct there. A generated chart, whose ids are never canonical,
/// publishes through `PUT` without complaint.
pub fn patch_from(
    document: &Document,
    fields: PatchFields,
    ids: ElementIds,
) -> Result<plus::PatchSceneContent, Error> {
    let selected = fields.selected();
    if selected.is_empty() {
        return Err(Error::invalid(
            "patch fields",
            "a merge request needs at least one of elements, appState or files",
        ));
    }
    let root = document.as_object();
    let mut body = serde_json::Map::new();
    for key in selected {
        match root.get(key) {
            Some(value) => {
                body.insert(key.to_owned(), value.clone());
            }
            // Selecting an absent field is a caller error, not an empty
            // collection: sending `{"files": {}}` would mean something else.
            None => {
                return Err(Error::invalid(
                    "patch fields",
                    format!("selected `{key}` but the document has no such field"),
                ));
            }
        }
    }
    if let Some(elements) = body.get("elements") {
        ids.check(elements)?;
    }
    Ok(plus::PatchSceneContent::from_value(Value::Object(body))?)
}

/// A read-then-write intent. It is **weaker than it looks**, and named for that.
///
/// The published artifact declares no `ETag`, `If-Match`, `409` or `412` for any
/// content operation, and `PUT` recomputes `sceneVersion` from the submitted
/// body, so nothing here is compare-and-swap. Two measured facts make it weaker
/// still (2026-09-21):
///
/// - **`sceneVersion` does not track geometry.** Moving an element's `x`, or
///   raising an element's `version`, left the value unchanged; only
///   `versionNonce` and `appState` moved it. Another writer can therefore
///   reposition an entire scene while `sceneVersion` stays equal, and this check
///   passes.
/// - **`contentEpoch` only counts authoritative replacements.** It advanced on
///   every `PUT`, including an identical one, and never on `PATCH`. Comparing it
///   detects a concurrent `PUT` and is blind to a concurrent `PATCH`.
///
/// So a passing [`check`](Self::check) means "no concurrent `PUT`, and no change
/// to `versionNonce` or `appState`". It does not mean the scene is unchanged.
/// Treat it as a cheap sanity check, never as a correctness guarantee.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnguardedCheckThenWrite {
    pub expected: crate::SceneVersion,
    /// `metadata.contentEpoch`, when the caller also wants it compared.
    ///
    /// Measured 2026-09-21: it counts authoritative replacements, advancing on
    /// every `PUT` and never on `PATCH` or a metadata-only update. Comparing it
    /// catches a concurrent replacement and nothing else.
    pub also_expect_epoch: Option<u64>,
}

impl UnguardedCheckThenWrite {
    pub fn new(expected: crate::SceneVersion) -> Self {
        Self {
            expected,
            also_expect_epoch: None,
        }
    }
    /// Compare against freshly read metadata.
    ///
    /// Advisory in both directions: the scene may change again immediately
    /// afterwards, and an `Ok` does not mean it is unchanged — see the type
    /// documentation for what `sceneVersion` and `contentEpoch` actually track.
    pub fn check(&self, metadata: &crate::model::SceneMetadata) -> Result<(), Error> {
        if metadata.scene_version != self.expected {
            return Err(Error::invalid(
                "scene changed",
                format!(
                    "expected sceneVersion {}, found {}",
                    self.expected, metadata.scene_version
                ),
            ));
        }
        if let Some(epoch) = self.also_expect_epoch
            && metadata.content_epoch != epoch
        {
            return Err(Error::invalid(
                "scene changed",
                format!(
                    "expected contentEpoch {epoch}, found {}",
                    metadata.content_epoch
                ),
            ));
        }
        Ok(())
    }
}
