use crate::{Document, ElementId, ElementKind, Error, FileId, GroupId, Object, wire::pointer};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Explicit substitutions, applied simultaneously. Unlisted identities stay unchanged.
#[derive(Clone, Debug, Default)]
pub struct IdMap {
    pub elements: BTreeMap<ElementId, ElementId>,
    pub groups: BTreeMap<GroupId, GroupId>,
    pub files: BTreeMap<FileId, FileId>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OpaquePolicy {
    Reject,
    Preserve,
}
#[derive(Clone, Debug)]
pub struct RemapResult {
    pub document: Document,
    pub mapping: IdMap,
    pub unassessed_paths: Vec<String>,
}

fn text<'a>(v: &'a Value, path: &str) -> Result<&'a str, Error> {
    v.as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::at(path, "expected nonempty identity"))
}
fn substitutions<'a>(
    ids: &BTreeSet<String>,
    mapping: impl Iterator<Item = (&'a String, &'a String)>,
    path: &str,
) -> Result<BTreeMap<String, String>, Error> {
    let mut result: BTreeMap<_, _> = ids.iter().map(|id| (id.clone(), id.clone())).collect();
    for (old, new) in mapping {
        if !ids.contains(old) {
            return Err(Error::at(path, format!("unknown mapping source {old}")));
        }
        if new.is_empty() {
            return Err(Error::at(path, "empty mapping destination"));
        }
        result.insert(old.clone(), new.clone());
    }
    if result.values().collect::<BTreeSet<_>>().len() != result.len() {
        return Err(Error::at(path, "mapping collides with another identity"));
    }
    Ok(result)
}
fn reference(
    object: &mut Object,
    key: &str,
    ids: &BTreeMap<String, String>,
    path: &str,
) -> Result<(), Error> {
    if let Some(v) = object.get_mut(key)
        && !v.is_null()
    {
        let p = format!("{path}{}", pointer(key));
        let id = text(v, &p)?;
        let replacement = ids
            .get(id)
            .ok_or_else(|| Error::at(&p, format!("unresolved reference {id}")))?;
        *v = Value::String(replacement.clone());
    }
    Ok(())
}

impl Document {
    /// Relabel known identities/references atomically, without translating geometry
    /// or resetting revisions. Opaque metadata and links require an explicit policy.
    pub fn remap_ids(&self, mapping: &IdMap, policy: OpaquePolicy) -> Result<RemapResult, Error> {
        let elements = self.elements()?;
        let mut ids = BTreeSet::new();
        let mut groups = BTreeSet::new();
        let mut file_ids = BTreeSet::new();
        let mut opaque = Vec::new();
        for key in self.root.keys() {
            if !["type", "version", "source", "elements", "appState", "files"]
                .contains(&key.as_str())
            {
                opaque.push(pointer(key));
            }
        }
        for (i, e) in elements.iter().enumerate() {
            let p = format!("/elements/{i}");
            let o = e.as_object();
            let id = text(
                o.get("id")
                    .ok_or_else(|| Error::at(&p, "missing identity"))?,
                &format!("{p}/id"),
            )?;
            if !ids.insert(id.to_owned()) {
                return Err(Error::at(
                    format!("{p}/id"),
                    "duplicate identity is ambiguous",
                ));
            }
            let kind = o.get("type").and_then(Value::as_str).unwrap_or("");
            if !ElementKind::KNOWN.contains(&kind) {
                opaque.push(format!("{p}/type"));
                continue;
            }
            for (k, v) in o {
                // Classification, not vocabulary membership: a field this crate
                // can name is not automatically a field remapping understands.
                // `None` covers both genuinely unknown keys and known keys added
                // to the vocabulary without a reference classification.
                let unassessed = match crate::model::element_reference_kind(k) {
                    None | Some(crate::model::ReferenceKind::Opaque) => true,
                    Some(_) => !crate::validation::applies(kind, k, o),
                };
                if unassessed || (k == "link" && !v.is_null()) {
                    opaque.push(format!("{p}{}", pointer(k)));
                }
            }
            if let Some(v) = o.get("groupIds") {
                for (j, g) in v
                    .as_array()
                    .ok_or_else(|| Error::at(format!("{p}/groupIds"), "expected array"))?
                    .iter()
                    .enumerate()
                {
                    groups.insert(text(g, &format!("{p}/groupIds/{j}"))?.to_owned());
                }
            }
            // Unknown nested keys can contain additional host-owned references.
            for (key, known) in [
                ("startBinding", crate::binding::FIELDS),
                ("endBinding", crate::binding::FIELDS),
                ("crop", crate::image_crop::FIELDS),
                ("roundness", crate::roundness::FIELDS),
                ("strokeOptions", crate::stroke_options::FIELDS),
            ] {
                if let Some(Value::Object(nested)) = o.get(key) {
                    for k in nested.keys() {
                        if !known.contains(&k.as_str()) {
                            opaque.push(format!("{p}/{key}{}", pointer(k)));
                        }
                    }
                }
            }
            for (key, known) in [
                ("boundElements", crate::bound_element::FIELDS),
                ("fixedSegments", crate::fixed_segment::FIELDS),
            ] {
                if let Some(Value::Array(a)) = o.get(key) {
                    for (j, v) in a.iter().enumerate() {
                        if let Some(o) = v.as_object() {
                            for k in o.keys() {
                                if !known.contains(&k.as_str()) {
                                    opaque.push(format!("{p}/{key}/{j}{}", pointer(k)));
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(Value::Object(state)) = self.root.get("appState") {
            for k in state.keys() {
                if !crate::app_state::FIELDS.contains(&k.as_str()) {
                    opaque.push(format!("/appState{}", pointer(k)));
                }
            }
            if let Some(Value::Object(locks)) = state.get("lockedMultiSelections") {
                groups.extend(locks.keys().cloned());
            }
        }
        if let Some(v) = self.root.get("files")
            && !v.is_null()
        {
            for (id, file) in v
                .as_object()
                .ok_or_else(|| Error::at("/files", "expected object"))?
            {
                let p = format!("/files{}", pointer(id));
                let o = file
                    .as_object()
                    .ok_or_else(|| Error::at(&p, "expected file object"))?;
                if o.get("id").and_then(Value::as_str) != Some(id) {
                    return Err(Error::at(
                        format!("{p}/id"),
                        "file identity disagrees with map key",
                    ));
                }
                file_ids.insert(id.clone());
                for k in o.keys() {
                    if !crate::binary_file::FIELDS.contains(&k.as_str()) {
                        opaque.push(format!("{p}{}", pointer(k)));
                    }
                }
            }
        }
        if policy == OpaquePolicy::Reject && !opaque.is_empty() {
            return Err(Error::at(
                &opaque[0],
                "opaque data may contain references; select Preserve to leave it untouched",
            ));
        }
        let ids = substitutions(
            &ids,
            mapping.elements.iter().map(|(a, b)| (&a.0, &b.0)),
            "/elements",
        )?;
        let groups = substitutions(
            &groups,
            mapping.groups.iter().map(|(a, b)| (&a.0, &b.0)),
            "/groupIds",
        )?;
        let files = substitutions(
            &file_ids,
            mapping.files.iter().map(|(a, b)| (&a.0, &b.0)),
            "/files",
        )?;
        let mut document = self.clone();
        for (i, v) in document
            .root
            .get_mut("elements")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            let p = format!("/elements/{i}");
            let o = v.as_object_mut().unwrap();
            reference(o, "id", &ids, &p)?;
            let kind = o
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            // Unknown kinds have an identity for ordering/duplication, but all
            // remaining properties have host-defined semantics.
            if !ElementKind::KNOWN.contains(&kind.as_str()) {
                continue;
            }
            for k in ["frameId", "containerId"] {
                if crate::validation::applies(&kind, k, o) {
                    reference(o, k, &ids, &p)?;
                }
            }
            if kind == "image" {
                reference(o, "fileId", &files, &p)?;
            }
            if let Some(Value::Array(a)) = o.get_mut("groupIds") {
                for g in a {
                    *g = Value::String(groups[text(g, &p)?].clone());
                }
            }
            for key in ["startBinding", "endBinding"] {
                if !crate::validation::applies(&kind, key, o) {
                    continue;
                }
                if let Some(v) = o.get_mut(key)
                    && !v.is_null()
                {
                    reference(
                        v.as_object_mut().ok_or_else(|| {
                            Error::at(format!("{p}/{key}"), "expected binding object")
                        })?,
                        "elementId",
                        &ids,
                        &format!("{p}/{key}"),
                    )?;
                }
            }
            if let Some(v) = o.get_mut("boundElements")
                && !v.is_null()
            {
                for (j, b) in v
                    .as_array_mut()
                    .ok_or_else(|| Error::at(format!("{p}/boundElements"), "expected array"))?
                    .iter_mut()
                    .enumerate()
                {
                    reference(
                        b.as_object_mut()
                            .ok_or_else(|| Error::at(&p, "expected bound element object"))?,
                        "id",
                        &ids,
                        &format!("{p}/boundElements/{j}"),
                    )?;
                }
            }
        }
        if let Some(Value::Object(state)) = document.root.get_mut("appState")
            && let Some(v) = state.get_mut("lockedMultiSelections")
        {
            let locks = v
                .as_object()
                .ok_or_else(|| Error::at("/appState/lockedMultiSelections", "expected object"))?;
            *v = Value::Object(
                locks
                    .iter()
                    .map(|(id, v)| (groups[id].clone(), v.clone()))
                    .collect(),
            );
        }
        if let Some(Value::Object(old)) = document.root.get_mut("files") {
            let mut next = Object::new();
            for (id, mut v) in std::mem::take(old) {
                v.as_object_mut()
                    .unwrap()
                    .insert("id".into(), Value::String(files[&id].clone()));
                next.insert(files[&id].clone(), v);
            }
            *old = next;
        }
        Ok(RemapResult {
            document,
            mapping: mapping.clone(),
            unassessed_paths: opaque,
        })
    }
}
