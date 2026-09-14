use crate::{Document, Error, Object, Profile, wire::pointer};
use serde_json::Value;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExportMode {
    Local,
    Database,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub path: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ExportResult {
    pub document: Document,
    pub changes: Vec<Change>,
}

impl Document {
    /// Apply only the pinned native serializer policy, not editor restoration.
    /// Requires object elements/appState/files and JS-roundtrippable numbers.
    /// Returns all removals/replacements/additions without modifying the source.
    pub fn project_native(
        &self,
        profile: Profile,
        mode: ExportMode,
        source: &str,
    ) -> Result<ExportResult, Error> {
        compatible_numbers(&Value::Object(self.root.clone()), "")?;
        let elements = self
            .root
            .get("elements")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::at("/elements", "projection requires array"))?;
        let state = self
            .root
            .get("appState")
            .and_then(Value::as_object)
            .ok_or_else(|| Error::at("/appState", "projection requires object"))?;
        let files = self
            .root
            .get("files")
            .and_then(Value::as_object)
            .ok_or_else(|| Error::at("/files", "projection requires object"))?;
        let mut next = Vec::new();
        let mut retained_files = Object::new();
        for (i, element) in elements.iter().enumerate() {
            let mut object = element
                .as_object()
                .cloned()
                .ok_or_else(|| Error::at(format!("/elements/{i}"), "projection requires object"))?;
            let deleted = object.get("isDeleted").is_some_and(truthy);
            if mode == ExportMode::Local
                && !deleted
                && let Some(file_id) = object.get("fileId").filter(|v| truthy(v))
            {
                let id = property_key(file_id).map_err(|_| {
                    Error::at(
                        format!("/elements/{i}/fileId"),
                        "JavaScript property-key conversion fails",
                    )
                })?;
                if id != "__proto__"
                    && let Some(file) = files.get(&id)
                    && truthy(file)
                {
                    // Multiple images may share a large data URL. Clone each
                    // resource once rather than once per referencing element.
                    retained_files.entry(id).or_insert_with(|| file.clone());
                }
            }
            if profile == Profile::V0_18_1 {
                if deleted {
                    continue;
                }
                if matches!(
                    object.get("type").and_then(Value::as_str),
                    Some("line" | "arrow")
                ) {
                    object.insert("lastCommittedPoint".into(), Value::Null);
                }
            }
            next.push(Value::Object(object));
        }
        let mut allowed = vec![
            "gridSize",
            "gridStep",
            "gridModeEnabled",
            "viewBackgroundColor",
        ];
        if profile == Profile::SnapshotAfa3a653 {
            allowed.push("lockedMultiSelections");
        }
        let app_state: Object = state
            .iter()
            .filter(|(key, _)| allowed.contains(&key.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut root = serde_json::json!({"type":"excalidraw","version":2,"source":source,"elements":next,"appState":app_state}).as_object().unwrap().clone();
        if mode == ExportMode::Local {
            root.insert("files".into(), Value::Object(retained_files));
        }
        let mut changes = Vec::new();
        differences(
            Some(&Value::Object(self.root.clone())),
            Some(&Value::Object(root.clone())),
            "",
            &mut changes,
        );
        Ok(ExportResult {
            document: Document { root },
            changes,
        })
    }
}
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64().is_some_and(|n| n != 0.),
        Value::String(v) => !v.is_empty(),
        _ => true,
    }
}
fn property_key(v: &Value) -> Result<String, ()> {
    Ok(match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => ryu_js::Buffer::new()
            .format_finite(n.as_f64().unwrap())
            .to_owned(),
        Value::Object(o) => {
            if o.contains_key("toString") {
                return Err(());
            }
            "[object Object]".into()
        }
        Value::Array(a) => a
            .iter()
            .map(|v| {
                if v.is_null() {
                    Ok(String::new())
                } else {
                    property_key(v)
                }
            })
            .collect::<Result<Vec<_>, _>>()?
            .join(","),
    })
}

// Compare decimal mathematical values without casting the original to f64.
// Exponents larger than i64 cannot fit a finite JS value and are rejected.
pub(crate) fn decimal(text: &str) -> Option<(bool, String, i64)> {
    let negative = text.starts_with('-');
    let text = text.trim_start_matches('-');
    let (mantissa, exponent) = text.split_once(['e', 'E']).unwrap_or((text, "0"));
    let exponent: i64 = exponent.parse().ok()?;
    let fractional = mantissa.split_once('.').map_or(0, |(_, f)| f.len());
    let digits = mantissa.replace('.', "");
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some((false, "0".into(), 0));
    }
    let stripped = digits.trim_end_matches('0');
    let exponent = exponent
        .checked_sub(i64::try_from(fractional).ok()?)?
        .checked_add(i64::try_from(digits.len() - stripped.len()).ok()?)?;
    Some((negative, stripped.into(), exponent))
}
pub(crate) fn compatible_numbers(value: &Value, path: &str) -> Result<(), Error> {
    match value {
        Value::Number(n) => {
            let converted = n
                .as_f64()
                .filter(|n| n.is_finite())
                .ok_or_else(|| Error::at(path, "number outside JavaScript range"))?;
            let mut buffer = ryu_js::Buffer::new();
            if decimal(&n.to_string()).is_none()
                || decimal(&n.to_string()) != decimal(buffer.format_finite(converted))
            {
                return Err(Error::at(path, "number changes in JavaScript"));
            }
        }
        Value::Array(a) => {
            for (i, v) in a.iter().enumerate() {
                compatible_numbers(v, &format!("{path}/{i}"))?;
            }
        }
        Value::Object(o) => {
            for (k, v) in o {
                compatible_numbers(v, &format!("{path}{}", pointer(k)))?;
            }
        }
        _ => {}
    }
    Ok(())
}
pub(crate) fn differences(
    before: Option<&Value>,
    after: Option<&Value>,
    path: &str,
    changes: &mut Vec<Change>,
) {
    if before == after {
        return;
    }
    match (before, after) {
        (Some(Value::Object(a)), Some(Value::Object(b))) => {
            let keys: std::collections::BTreeSet<_> = a.keys().chain(b.keys()).collect();
            for key in keys {
                differences(
                    a.get(key),
                    b.get(key),
                    &format!("{path}{}", pointer(key)),
                    changes,
                );
            }
        }
        (Some(Value::Array(a)), Some(Value::Array(b))) => {
            for i in 0..a.len().max(b.len()) {
                differences(a.get(i), b.get(i), &format!("{path}/{i}"), changes);
            }
        }
        _ => changes.push(Change {
            path: path.into(),
            before: before.cloned(),
            after: after.cloned(),
        }),
    }
}
