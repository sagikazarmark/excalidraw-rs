use crate::{Element, Error, Field, Number, Object, WireValue, wire};
use serde_json::{Value, json};

/// A recognizable native scene envelope. Unknown root and nested data is retained.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub(crate) root: Object,
}
impl Document {
    pub fn validated(
        &self,
        profile: crate::Profile,
        purpose: crate::Purpose,
    ) -> Result<ValidatedDocument<'_>, crate::ValidationReport> {
        let report = self.validate(profile, purpose);
        if report.is_valid() {
            Ok(ValidatedDocument {
                document: self,
                profile,
            })
        } else {
            Err(report)
        }
    }
    pub fn new(source: impl Into<String>) -> Self {
        Self::from_value(json!({"type":"excalidraw","version":2,"source":source.into(),"elements":[],"appState":{},"files":{}})).expect("valid empty document")
    }
    /// Strict JSON decode with duplicate-key rejection and exact numeric retention.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        Self::from_value(wire::parse(bytes)?)
    }
    /// Values already decoded by another parser cannot be checked for duplicate keys.
    pub fn from_value(value: Value) -> Result<Self, Error> {
        let Value::Object(root) = value else {
            return Err(Error::at("", "expected scene object"));
        };
        if root.get("type").and_then(Value::as_str) != Some("excalidraw") {
            return Err(Error::at("/type", "expected excalidraw"));
        }
        for (key, array) in [("elements", true), ("appState", false), ("files", false)] {
            if let Some(v) = root.get(key)
                && !v.is_null()
                && !(if array { v.is_array() } else { v.is_object() })
            {
                return Err(Error::at(wire::pointer(key), "invalid collection shape"));
            }
        }
        Ok(Self { root })
    }
    pub fn as_object(&self) -> &Object {
        &self.root
    }
    pub fn into_value(self) -> Value {
        Value::Object(self.root)
    }
    pub fn to_vec(&self) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec(&self.root)?)
    }
    pub fn to_vec_pretty(&self) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec_pretty(&self.root)?)
    }
    pub fn source(&self) -> Result<Field<String>, Error> {
        root_field(&self.root, "source", String::read_wire)
    }
    pub fn version(&self) -> Result<Field<Number>, Error> {
        root_field(&self.root, "version", Number::read_wire)
    }
    /// Presence-aware structural access; element contents are decoded on inspection.
    pub fn elements_field(&self) -> Result<Field<Vec<Element>>, Error> {
        root_field(&self.root, "elements", Vec::<Element>::read_wire)
    }
    pub fn app_state_field(&self) -> Result<Field<crate::AppState>, Error> {
        root_field(&self.root, "appState", crate::AppState::read_wire)
    }
    pub fn files_field(
        &self,
    ) -> Result<Field<std::collections::BTreeMap<crate::FileId, crate::BinaryFile>>, Error> {
        root_field(&self.root, "files", |value| {
            Object::read_wire(value)?
                .into_iter()
                .map(|(id, value)| {
                    crate::BinaryFile::read_wire(&value)
                        .map(|file| (crate::FileId(id.clone()), file))
                        .map_err(|e| {
                            Error::at(format!("{}{}", wire::pointer(&id), e.path), e.message)
                        })
                })
                .collect()
        })
    }
    /// Presence-aware setters check only the envelope, not authored validity.
    pub fn set_source(&mut self, source: Field<String>) -> Result<(), Error> {
        self.set_root_field("source", source)
    }
    pub fn set_version(&mut self, version: Field<Number>) -> Result<(), Error> {
        self.set_root_field("version", version)
    }
    pub fn set_elements_field(&mut self, elements: Field<Vec<Element>>) -> Result<(), Error> {
        self.set_root_field("elements", elements)
    }
    pub fn set_app_state_field(&mut self, state: Field<crate::AppState>) -> Result<(), Error> {
        self.set_root_field("appState", state)
    }
    pub fn set_files_field(
        &mut self,
        files: Field<std::collections::BTreeMap<crate::FileId, crate::BinaryFile>>,
    ) -> Result<(), Error> {
        match files {
            Field::Missing => self.remove_root("files"),
            Field::Null => self.set_root("files", Value::Null),
            Field::Value(files) => self.set_root(
                "files",
                Value::Object(
                    files
                        .into_iter()
                        .map(|(id, file)| (id.0, file.write_wire()))
                        .collect(),
                ),
            ),
        }
    }
    fn set_root_field<T: WireValue>(&mut self, key: &str, field: Field<T>) -> Result<(), Error> {
        match field {
            Field::Missing => self.remove_root(key),
            Field::Null => self.set_root(key, Value::Null),
            Field::Value(value) => self.set_root(key, value.write_wire()),
        }
    }
    /// Returns owned typed views. Use `set_elements` to publish changes atomically.
    /// Malformed nonobject entries are reported; they remain in the original document.
    pub fn elements(&self) -> Result<Vec<Element>, Error> {
        let values = self
            .root
            .get("elements")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::at("/elements", "expected array"))?;
        values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v.as_object()
                    .cloned()
                    .map(Element::from_object)
                    .ok_or_else(|| Error::at(format!("/elements/{i}"), "expected element object"))
            })
            .collect()
    }
    pub fn set_elements(&mut self, elements: Vec<Element>) {
        self.root.insert(
            "elements".into(),
            Value::Array(
                elements
                    .into_iter()
                    .map(|e| Value::Object(e.into_object()))
                    .collect(),
            ),
        );
    }
    /// Edit one element without disturbing malformed or unknown neighboring records.
    pub fn edit_element(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut Element) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let path = format!("/elements/{index}");
        let value = self
            .root
            .get("elements")
            .and_then(Value::as_array)
            .and_then(|a| a.get(index))
            .and_then(Value::as_object)
            .ok_or_else(|| Error::at(&path, "expected element object"))?;
        let mut element = Element::from_object(value.clone());
        edit(&mut element)?;
        self.root
            .get_mut("elements")
            .unwrap()
            .as_array_mut()
            .unwrap()[index] = Value::Object(element.into_object());
        Ok(())
    }
    pub fn app_state(&self) -> Result<crate::AppState, Error> {
        self.root
            .get("appState")
            .and_then(Value::as_object)
            .cloned()
            .map(crate::AppState::from_object)
            .ok_or_else(|| Error::at("/appState", "expected object"))
    }
    pub fn set_app_state(&mut self, state: crate::AppState) {
        self.root
            .insert("appState".into(), Value::Object(state.into_object()));
    }
    pub fn files(
        &self,
    ) -> Result<std::collections::BTreeMap<crate::FileId, crate::BinaryFile>, Error> {
        self.root
            .get("files")
            .and_then(Value::as_object)
            .ok_or_else(|| Error::at("/files", "expected object"))?
            .iter()
            .map(|(id, v)| {
                v.as_object()
                    .cloned()
                    .map(|o| (crate::FileId(id.clone()), crate::BinaryFile::from_object(o)))
                    .ok_or_else(|| {
                        Error::at(
                            format!("/files{}", wire::pointer(id)),
                            "expected file record",
                        )
                    })
            })
            .collect()
    }
    pub fn set_files(
        &mut self,
        files: std::collections::BTreeMap<crate::FileId, crate::BinaryFile>,
    ) {
        self.root.insert(
            "files".into(),
            Value::Object(
                files
                    .into_iter()
                    .map(|(id, f)| (id.0, Value::Object(f.into_object())))
                    .collect(),
            ),
        );
    }
    /// Set root extension/metadata, checking the envelope before committing.
    pub fn set_root(&mut self, key: impl Into<String>, value: Value) -> Result<(), Error> {
        let mut root = self.root.clone();
        root.insert(key.into(), value);
        *self = Self::from_value(Value::Object(root))?;
        Ok(())
    }
    /// Remove metadata atomically. The required `type` discriminator cannot be removed.
    pub fn remove_root(&mut self, key: &str) -> Result<(), Error> {
        let mut root = self.root.clone();
        root.remove(key);
        *self = Self::from_value(Value::Object(root))?;
        Ok(())
    }
}

fn root_field<T>(
    root: &Object,
    key: &str,
    read: impl FnOnce(&Value) -> Result<T, Error>,
) -> Result<Field<T>, Error> {
    match root.get(key) {
        None => Ok(Field::Missing),
        Some(Value::Null) => Ok(Field::Null),
        Some(value) => read(value)
            .map(Field::Value)
            .map_err(|e| Error::at(format!("{}{}", wire::pointer(key), e.path), e.message)),
    }
}

/// Immutable validated borrow; mutation cannot invalidate its certification.
pub struct ValidatedDocument<'a> {
    document: &'a Document,
    profile: crate::Profile,
}
impl ValidatedDocument<'_> {
    pub fn document(&self) -> &Document {
        self.document
    }
    pub fn project_native(
        &self,
        mode: crate::ExportMode,
        source: &str,
    ) -> Result<crate::ExportResult, Error> {
        self.document.project_native(self.profile, mode, source)
    }
}
