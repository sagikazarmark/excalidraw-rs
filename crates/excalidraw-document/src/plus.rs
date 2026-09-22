//! Data-only adapters for the public Plus schema observed 2026-09-12. These do
//! not send HTTP, reconcile versions, or certify unpublished server element rules.
//! Root/appState/file fields outside the documented transport surface cause
//! errors, not loss. Element validation is explicitly incomplete; request adapters
//! expose support diagnostics and cannot certify live server acceptance.
use crate::{Document, Error, Number, Object, wire};
use serde_json::Value;

/// The `appState` keys the published Plus transport schema carries. Any other
/// key is rejected, not stripped.
pub const APP_STATE_KEYS: [&str; 2] = ["viewBackgroundColor", "lockedMultiSelections"];

fn closed(o: &Object, keys: &[&str], path: &str) -> Result<(), Error> {
    for key in o.keys() {
        if !keys.contains(&key.as_str()) {
            return Err(Error::at(
                format!("{path}{}", wire::pointer(key)),
                "field is outside documented Plus transport schema",
            ));
        }
    }
    Ok(())
}
fn state(value: &Value, partial: bool) -> Result<(), Error> {
    let o = value
        .as_object()
        .ok_or_else(|| Error::at("/appState", "expected object"))?;
    closed(o, &APP_STATE_KEYS, "/appState")?;
    if (!partial || o.contains_key("viewBackgroundColor"))
        && !o.get("viewBackgroundColor").is_some_and(Value::is_string)
    {
        return Err(Error::at(
            "/appState/viewBackgroundColor",
            "expected string",
        ));
    }
    if let Some(v) = o.get("lockedMultiSelections") {
        for (key, v) in v
            .as_object()
            .ok_or_else(|| Error::at("/appState/lockedMultiSelections", "expected object"))?
        {
            if v != &Value::Bool(true) {
                return Err(Error::at(
                    format!("/appState/lockedMultiSelections{}", wire::pointer(key)),
                    "expected true",
                ));
            }
        }
    }
    Ok(())
}
fn elements(value: &Value) -> Result<(), Error> {
    let a = value
        .as_array()
        .ok_or_else(|| Error::at("/elements", "expected array"))?;
    for (i, v) in a.iter().enumerate() {
        if !v.is_object() {
            return Err(Error::at(
                format!("/elements/{i}"),
                "expected persisted element object; published OpenAPI item schema is incomplete",
            ));
        }
        if !v.get("type").is_some_and(Value::is_string) {
            return Err(Error::at(
                format!("/elements/{i}/type"),
                "expected element discriminator",
            ));
        }
        for key in ["startBinding", "endBinding"] {
            if let Some(binding) = v.get(key).filter(|v| !v.is_null()) {
                if !binding.is_object() {
                    return Err(Error::at(
                        format!("/elements/{i}/{key}"),
                        "expected binding object",
                    ));
                }
                if binding
                    .get("mode")
                    .is_some_and(|v| !matches!(v.as_str(), Some("inside" | "orbit" | "skip")))
                {
                    return Err(Error::at(
                        format!("/elements/{i}/{key}/mode"),
                        "unsupported binding mode",
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Paths to element features whose Plus support is not established by the dated
/// public reference. Empty output still does not certify unpublished server rules.
///
/// # What a live run observed
///
/// Against one workspace on 2026-09-21, every feature reported here was in fact
/// accepted by the service: `stickynote` (given a numeric `baseHeight`), and
/// `created`, `baseFontSize` and `labelPosition` all round-tripped unchanged.
/// One did not: **`freedraw.strokeOptions` was accepted and then silently
/// stripped** — the write succeeded and the field was absent from the response.
///
/// These paths are still reported. A single workspace on a single date is not a
/// published contract, and the `strokeOptions` result shows the failure mode
/// this list exists to warn about: acceptance is not retention. Callers who have
/// verified their own deployment can ignore the paths; callers who have not are
/// better served by a warning that proves unnecessary than by silent loss.
fn unconfirmed(root: &Object) -> Vec<String> {
    let mut result = vec![];
    if let Some(Value::Array(elements)) = root.get("elements") {
        for (i, e) in elements.iter().enumerate() {
            if !matches!(
                e.get("type").and_then(Value::as_str),
                Some(
                    "rectangle"
                        | "diamond"
                        | "ellipse"
                        | "text"
                        | "line"
                        | "arrow"
                        | "freedraw"
                        | "image"
                        | "frame"
                        | "magicframe"
                        | "iframe"
                        | "embeddable"
                )
            ) {
                result.push(format!("/elements/{i}/type"));
            }
            for key in ["created", "baseFontSize", "labelPosition", "strokeOptions"] {
                if e.get(key).is_some() {
                    result.push(format!("/elements/{i}/{key}"));
                }
            }
        }
    }
    result
}
fn files(value: &Value) -> Result<(), Error> {
    let o = value
        .as_object()
        .ok_or_else(|| Error::at("/files", "expected object"))?;
    for (id, v) in o {
        let p = format!("/files{}", wire::pointer(id));
        let f = v
            .as_object()
            .ok_or_else(|| Error::at(&p, "expected file record"))?;
        closed(
            f,
            &[
                "id",
                "mimeType",
                "dataURL",
                "created",
                "lastRetrieved",
                "version",
            ],
            &p,
        )?;
        let id = f
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::at(format!("{p}/id"), "expected string"))?;
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err(Error::at(format!("{p}/id"), "invalid Plus file ID"));
        }
        for key in ["mimeType", "dataURL"] {
            if !f.get(key).is_some_and(Value::is_string) {
                return Err(Error::at(format!("{p}/{key}"), "expected string"));
            }
        }
        if f["dataURL"].as_str().unwrap().chars().count() > 20_971_520 {
            return Err(Error::at(
                format!("{p}/dataURL"),
                "data URL exceeds published character limit",
            ));
        }
        // A record whose data URL does not carry what its `mimeType` promises
        // is self-contradictory, and sending it stores an unusable image under
        // a type nothing can read. That is a fact about the record rather than
        // about this envelope, so the predicate is the one `Document::validate`
        // applies, shared rather than respelled here.
        //
        // Whether the type is one this crate *recognises* is deliberately not
        // asked. The published Plus schema constrains `mimeType` to "string" —
        // no enum, no pattern — so `MimeType::KNOWN` is this crate's
        // vocabulary, not the transport's. Rejecting on it would refuse
        // records the service accepts: a format newer than the crate, or a
        // self-consistent uppercase spelling that RFC 2045 §5.1 makes
        // equivalent but `KNOWN` does not contain. This adapter reports what
        // it cannot confirm; it does not invent constraints the reference
        // never published.
        if !crate::validation::data_url_carries(
            f["dataURL"].as_str().unwrap(),
            f["mimeType"].as_str().unwrap(),
        ) {
            return Err(Error::at(
                format!("{p}/dataURL"),
                "expected nonempty data URL carrying the declared mimeType",
            ));
        }
        for key in ["created", "lastRetrieved", "version"] {
            if key == "created" || f.contains_key(key) {
                let value = match f.get(key) {
                    Some(Value::Number(n)) => Number(n.clone()).as_safe_integer().ok(),
                    _ => None,
                };
                if value.is_none_or(|n| n <= 0) {
                    return Err(Error::at(
                        format!("{p}/{key}"),
                        "expected positive JavaScript-safe integer",
                    ));
                }
            }
        }
    }
    Ok(())
}
fn document(document: &Document) -> Result<(), Error> {
    let o = document.as_object();
    closed(
        o,
        &["type", "version", "source", "elements", "appState", "files"],
        "",
    )?;
    if !o.get("version").is_some_and(Value::is_number) {
        return Err(Error::at("/version", "expected number"));
    }
    if !o.get("source").is_some_and(Value::is_string) {
        return Err(Error::at("/source", "expected string"));
    }
    elements(o.get("elements").unwrap_or(&Value::Null))?;
    state(o.get("appState").unwrap_or(&Value::Null), false)?;
    files(o.get("files").unwrap_or(&Value::Null))?;
    crate::projection::compatible_numbers(&Value::Object(o.clone()), "")
}
#[derive(Clone, Debug, PartialEq)]
pub struct SceneContent {
    document: Document,
    scene_version: String,
    failed: Option<Vec<String>>,
}
impl SceneContent {
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        let Value::Object(mut o) = wire::parse(bytes)? else {
            return Err(Error::at("", "expected object"));
        };
        let version = o
            .remove("sceneVersion")
            .and_then(|v| v.as_str().map(str::to_owned))
            .ok_or_else(|| Error::at("/sceneVersion", "expected opaque string"))?;
        let failed = o
            .remove("filesFailedToEmbed")
            .map(|v| {
                v.as_array()
                    .ok_or_else(|| Error::at("/filesFailedToEmbed", "expected array"))?
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| Error::at("/filesFailedToEmbed", "expected string IDs"))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let doc = Document::from_value(Value::Object(o))?;
        document(&doc)?;
        Ok(Self {
            document: doc,
            scene_version: version,
            failed,
        })
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn scene_version(&self) -> &str {
        &self.scene_version
    }
    pub fn files_failed_to_embed(&self) -> Option<&[String]> {
        self.failed.as_deref()
    }
    pub fn to_vec(&self) -> Result<Vec<u8>, Error> {
        let mut o = self.document.as_object().clone();
        o.insert(
            "sceneVersion".into(),
            Value::String(self.scene_version.clone()),
        );
        if let Some(f) = &self.failed {
            o.insert(
                "filesFailedToEmbed".into(),
                Value::Array(f.iter().cloned().map(Value::String).collect()),
            );
        }
        Ok(serde_json::to_vec(&o)?)
    }
}
/// Authoritative replacement body. No sceneVersion precondition is invented.
#[derive(Clone, Debug)]
pub struct ReplaceSceneContent {
    document: Document,
}
impl ReplaceSceneContent {
    pub fn unconfirmed_element_paths(&self) -> Vec<String> {
        unconfirmed(self.document.as_object())
    }
    pub fn new(value: Document) -> Result<Self, Error> {
        document(&value)?;
        Ok(Self { document: value })
    }
    pub fn to_vec(&self) -> Result<Vec<u8>, Error> {
        self.document.to_vec()
    }
}
/// Scene-level PATCH: omission differs from empty arrays/maps. Element records
/// are sent intact; reconciliation belongs to the server, not this adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchSceneContent {
    root: Object,
}
impl PatchSceneContent {
    pub fn unconfirmed_element_paths(&self) -> Vec<String> {
        unconfirmed(&self.root)
    }
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        Self::from_value(wire::parse(bytes)?)
    }
    pub fn from_value(value: Value) -> Result<Self, Error> {
        let o = value
            .as_object()
            .ok_or_else(|| Error::at("", "expected object"))?;
        closed(o, &["elements", "appState", "files"], "")?;
        if o.is_empty() {
            return Err(Error::at("", "PATCH needs elements, appState or files"));
        }
        if let Some(v) = o.get("elements") {
            elements(v)?;
        }
        if let Some(v) = o.get("appState") {
            state(v, true)?;
        }
        if let Some(v) = o.get("files") {
            files(v)?;
        }
        crate::projection::compatible_numbers(&value, "")?;
        Ok(Self { root: o.clone() })
    }
    pub fn as_object(&self) -> &Object {
        &self.root
    }
    pub fn to_vec(&self) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec(&self.root)?)
    }
}
