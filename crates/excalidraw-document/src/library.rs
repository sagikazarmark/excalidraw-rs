use crate::{
    Document, Error, LibraryItem, LibraryStatus, Object, Profile, Purpose, Severity,
    ValidationReport, library_item,
    validation::{check_present, compatibility_severity, required},
    wire,
};
use serde_json::{Value, json};

impl LibraryItem {
    /// Construct an unpublished item, validating its metadata and internally
    /// scoped element graph. Image resources are supplied separately on insertion.
    pub fn new(
        profile: Profile,
        id: impl Into<String>,
        created: crate::Number,
        elements: Vec<crate::Element>,
    ) -> Result<Self, Error> {
        let mut item = Self::default();
        item.set(library_item::ID, id.into())?;
        item.set(library_item::STATUS, LibraryStatus::Unpublished)?;
        item.set(library_item::CREATED, created)?;
        item.set(library_item::ELEMENTS, elements)?;
        item.check_authored(profile)?;
        Ok(item)
    }

    pub(crate) fn check_authored(&self, profile: Profile) -> Result<(), Error> {
        let report = LibraryDocument::new("library-authoring", vec![self.clone()])
            .validate(profile, Purpose::Author);
        if let Some(d) = report
            .diagnostics
            .into_iter()
            .find(|d| d.severity == Severity::Error)
        {
            return Err(Error::at(d.path, format!("{}: {}", d.code, d.message)));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LibraryDocument {
    pub(crate) root: Object,
}

impl LibraryDocument {
    pub fn new(source: impl Into<String>, items: Vec<LibraryItem>) -> Self {
        Self { root: json!({"type":"excalidrawlib","version":2,"source":source.into(),"libraryItems":items}).as_object().unwrap().clone() }
    }
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        Self::from_value(wire::parse(bytes)?)
    }
    pub fn from_value(value: Value) -> Result<Self, Error> {
        let Value::Object(root) = value else {
            return Err(Error::at("", "expected library object"));
        };
        if root.get("type").and_then(Value::as_str) != Some("excalidrawlib") {
            return Err(Error::at("/type", "expected excalidrawlib"));
        }
        let key = match library_version(&root) {
            Some(1) => "library",
            Some(2) => "libraryItems",
            _ => return Err(Error::at("/version", "expected library version 1 or 2")),
        };
        if !root.get(key).is_some_and(Value::is_array) {
            return Err(Error::at(wire::pointer(key), "expected library array"));
        }
        Ok(Self { root })
    }
    pub fn as_object(&self) -> &Object {
        &self.root
    }
    pub fn to_vec(&self) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec(&self.root)?)
    }
    pub fn to_vec_pretty(&self) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec_pretty(&self.root)?)
    }
    pub fn items(&self) -> Result<Vec<LibraryItem>, Error> {
        if library_version(&self.root) != Some(2) {
            return Err(Error::at(
                "/version",
                "legacy library requires explicit migration",
            ));
        }
        let items = self
            .root
            .get("libraryItems")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                Error::at(
                    "/libraryItems",
                    "legacy library requires explicit migration",
                )
            })?;
        items
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v.as_object()
                    .cloned()
                    .map(LibraryItem::from_object)
                    .ok_or_else(|| Error::at(format!("/libraryItems/{i}"), "expected item object"))
            })
            .collect()
    }
    pub fn set_items(&mut self, items: Vec<LibraryItem>) -> Result<(), Error> {
        if library_version(&self.root) != Some(2) {
            return Err(Error::at(
                "/version",
                "cannot replace legacy items without migration",
            ));
        }
        self.root
            .insert("libraryItems".into(), serde_json::to_value(items)?);
        Ok(())
    }
    pub fn validate(&self, profile: Profile, purpose: Purpose) -> ValidationReport {
        let mut report = ValidationReport::default();
        let legacy = library_version(&self.root) == Some(1);
        let key = if legacy { "library" } else { "libraryItems" };
        if purpose != Purpose::Inspect {
            required(
                &mut report,
                &self.root,
                "",
                &["type", "version", "source", key],
                &[],
            );
        }
        if let Some(source) = self.root.get("source")
            && !source.is_string()
        {
            report.issue("/source", "field-type", Severity::Error, "expected string");
        }
        let Some(items) = self.root.get(key).and_then(Value::as_array) else {
            return report;
        };
        let mut ids = std::collections::BTreeSet::new();
        for (i, item) in items.iter().enumerate() {
            let path = format!("/{key}/{i}");
            let elements = if legacy {
                item
            } else {
                let Some(object) = item.as_object() else {
                    report.issue(path, "item-shape", Severity::Error, "expected object");
                    continue;
                };
                for error in library_item::check(&LibraryItem::from_object(object.clone())) {
                    report.issue(
                        format!("{path}{}", error.path),
                        "field-type",
                        Severity::Error,
                        error.message,
                    );
                }
                check_present(&mut report, object, &path, library_item::FIELDS, &[]);
                if let Some(Value::Number(created)) = object.get("created")
                    && crate::Number(created.clone()).as_safe_integer().is_err()
                {
                    report.issue(
                        format!("{path}/created"),
                        "integer",
                        Severity::Error,
                        "expected safe integer timestamp",
                    );
                }
                if object.get("id").and_then(Value::as_str) == Some("") {
                    report.issue(
                        format!("{path}/id"),
                        "empty-id",
                        Severity::Error,
                        "empty library item identity",
                    );
                }
                if purpose != Purpose::Inspect {
                    required(
                        &mut report,
                        object,
                        &path,
                        &["id", "status", "created", "elements"],
                        &[],
                    );
                }
                if let Some(id) = object.get("id").and_then(Value::as_str)
                    && !ids.insert(id)
                {
                    report.issue(
                        format!("{path}/id"),
                        "duplicate-id",
                        Severity::Error,
                        "duplicate library item ID",
                    );
                }
                if let Some(status) = object.get("status").and_then(Value::as_str)
                    && !LibraryStatus::KNOWN.contains(&status)
                {
                    report.issue(
                        format!("{path}/status"),
                        "enum",
                        Severity::Error,
                        "unknown item status",
                    );
                }
                object.get("elements").unwrap_or(&Value::Null)
            };
            if !elements.is_array() {
                report.issue(
                    &path,
                    "elements-shape",
                    Severity::Error,
                    "expected elements array",
                );
                continue;
            }
            let elements_path = if legacy {
                path.clone()
            } else {
                format!("{path}/elements")
            };
            let values = elements.as_array().unwrap();
            if values.is_empty() {
                report.issue(
                    &elements_path,
                    "empty-library-item",
                    compatibility_severity(purpose),
                    "library item must contain elements",
                );
            }
            for (j, element) in values.iter().enumerate() {
                if element.get("isDeleted") == Some(&Value::Bool(true)) {
                    report.issue(
                        format!("{elements_path}/{j}/isDeleted"),
                        "deleted-library-element",
                        compatibility_severity(purpose),
                        "library item elements must be nondeleted",
                    );
                }
            }
            let document = Document::from_value(json!({"type":"excalidraw","version":2,"source":"library-validation","elements":elements,"appState":{},"files":{}})).unwrap();
            let mut result = document.validate(profile, purpose);
            for d in &mut result.diagnostics {
                d.path = if legacy {
                    format!(
                        "{path}{}",
                        d.path.strip_prefix("/elements").unwrap_or(&d.path)
                    )
                } else {
                    format!("{path}{}", d.path)
                };
            }
            report.diagnostics.extend(result.diagnostics);
        }
        report
    }
}

fn library_version(root: &Object) -> Option<i64> {
    match root.get("version") {
        Some(Value::Number(n)) => crate::Number(n.clone()).as_safe_integer().ok(),
        _ => None,
    }
}
