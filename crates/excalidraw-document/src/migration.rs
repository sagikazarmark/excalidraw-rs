use crate::{Change, Document, Error, Profile};
use serde_json::{Value, json};

#[derive(Clone, Debug)]
pub struct MigrationResult {
    pub document: Document,
    pub changes: Vec<Change>,
}
impl Document {
    /// Exact, non-geometric profile conversions only. Unsupported semantic losses
    /// (sticky notes, active polygon editing, bindings, custom stroke behavior) fail
    /// with a path; the original document remains untouched.
    pub fn migrate(&self, from: Profile, to: Profile) -> Result<MigrationResult, Error> {
        if from == to {
            return Ok(MigrationResult {
                document: self.clone(),
                changes: vec![],
            });
        }
        let mut document = self.clone();
        let elements = document
            .root
            .get_mut("elements")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| Error::at("/elements", "migration requires elements array"))?;
        for (i, v) in elements.iter_mut().enumerate() {
            let p = format!("/elements/{i}");
            let o = v
                .as_object_mut()
                .ok_or_else(|| Error::at(&p, "expected element object"))?;
            let kind = o
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            if !crate::ElementKind::KNOWN.contains(&kind.as_str()) {
                return Err(Error::at(
                    format!("{p}/type"),
                    "unknown kind cannot be certified across profiles",
                ));
            }
            for k in ["startBinding", "endBinding"] {
                if o.get(k).is_some_and(|v| !v.is_null()) {
                    return Err(Error::at(
                        format!("{p}/{k}"),
                        "binding conversion requires target geometry; no approximate migration is performed",
                    ));
                }
            }
            if to == Profile::SnapshotAfa3a653 {
                o.entry("created").or_insert(Value::Null);
                o.remove("lastCommittedPoint");
                match kind.as_str() {
                    "line" => {
                        o.entry("polygon").or_insert(json!(false));
                    }
                    "text" => {
                        o.entry("baseFontSize").or_insert(Value::Null);
                    }
                    "freedraw" => {
                        o.entry("strokeOptions")
                            .or_insert(json!({"variability":"variable","streamline":0.5}));
                    }
                    "draw" => {
                        o.insert("type".into(), json!("line"));
                        o.entry("polygon").or_insert(json!(false));
                    }
                    _ => {}
                }
            } else {
                if kind == "stickynote" {
                    return Err(Error::at(
                        format!("{p}/type"),
                        "sticky note has no lossless released equivalent",
                    ));
                }
                for k in ["polygon", "baseFontSize", "labelPosition", "strokeOptions"] {
                    if let Some(v) = o.get(k) {
                        let removable = match k {
                            "polygon" => v == &json!(false),
                            "baseFontSize" | "labelPosition" => v.is_null(),
                            "strokeOptions" => {
                                v == &json!({"variability":"variable","streamline":0.5})
                            }
                            _ => false,
                        };
                        if !removable {
                            return Err(Error::at(
                                format!("{p}/{k}"),
                                "field has no lossless released equivalent",
                            ));
                        }
                    }
                    o.remove(k);
                }
                if matches!(o.get("fontFamily"),Some(Value::Number(n)) if crate::Number(n.clone()).as_safe_integer()==Ok(10))
                {
                    return Err(Error::at(
                        format!("{p}/fontFamily"),
                        "font not in released registry",
                    ));
                }
                // Creation metadata is preserved as a release extension, not discarded.
                if matches!(kind.as_str(), "line" | "arrow" | "freedraw") {
                    o.entry("lastCommittedPoint").or_insert(Value::Null);
                }
            }
            for key in ["startArrowhead", "endArrowhead"] {
                if let Some(Value::String(head)) = o.get_mut(key) {
                    let translated = if to == Profile::SnapshotAfa3a653 {
                        match head.as_str() {
                            "dot" => "circle",
                            "crowfoot_one" => "cardinality_one",
                            "crowfoot_many" => "cardinality_many",
                            "crowfoot_one_or_many" => "cardinality_one_or_many",
                            _ => head.as_str(),
                        }
                    } else {
                        match head.as_str() {
                            "cardinality_one" => "crowfoot_one",
                            "cardinality_many" => "crowfoot_many",
                            "cardinality_one_or_many" => "crowfoot_one_or_many",
                            v if v.starts_with("cardinality_") => {
                                return Err(Error::at(
                                    format!("{p}/{key}"),
                                    "head has no released equivalent",
                                ));
                            }
                            _ => head.as_str(),
                        }
                    };
                    *head = translated.to_owned();
                }
            }
        }
        if to == Profile::V0_18_1
            && let Some(Value::Object(state)) = document.root.get_mut("appState")
            && let Some(locks) = state.get("lockedMultiSelections")
        {
            if locks.as_object().is_none_or(|o| !o.is_empty()) {
                return Err(Error::at(
                    "/appState/lockedMultiSelections",
                    "group locks have no released equivalent",
                ));
            }
            state.remove("lockedMultiSelections");
        }
        let mut changes = vec![];
        crate::projection::differences(
            Some(&Value::Object(self.root.clone())),
            Some(&Value::Object(document.root.clone())),
            "",
            &mut changes,
        );
        Ok(MigrationResult { document, changes })
    }
}
