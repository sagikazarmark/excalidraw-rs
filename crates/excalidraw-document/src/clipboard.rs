use crate::{Document, Error, Object, wire};
use serde_json::{Value, json};

/// Portable clipboard JSON, distinct from scenes and library items. Conversion
/// wraps existing content; it does not simulate copy/paste identity regeneration.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipboardDocument {
    root: Object,
}
impl ClipboardDocument {
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        Self::from_value(wire::parse(bytes)?)
    }
    pub fn from_value(value: Value) -> Result<Self, Error> {
        let root = value
            .as_object()
            .ok_or_else(|| Error::at("", "expected clipboard object"))?;
        if root.get("type").and_then(Value::as_str) != Some("excalidraw/clipboard") {
            return Err(Error::at("/type", "expected excalidraw/clipboard"));
        }
        if !root.get("elements").is_some_and(Value::is_array) {
            return Err(Error::at("/elements", "expected array"));
        }
        if root.get("files").is_some_and(|v| !v.is_object()) {
            return Err(Error::at("/files", "expected object"));
        }
        Ok(Self { root: root.clone() })
    }
    pub fn as_object(&self) -> &Object {
        &self.root
    }
    pub fn to_vec(&self) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec(&self.root)?)
    }
    /// Explicit projection: only elements/files enter the clipboard envelope.
    pub fn from_document(document: &Document) -> Result<Self, Error> {
        let mut root = Object::new();
        root.insert("type".into(), json!("excalidraw/clipboard"));
        root.insert(
            "elements".into(),
            document
                .root
                .get("elements")
                .cloned()
                .ok_or_else(|| Error::at("/elements", "missing array"))?,
        );
        if let Some(files) = document.root.get("files") {
            root.insert("files".into(), files.clone());
        }
        Self::from_value(Value::Object(root))
    }
    /// Explicit projection, preserving elements/files but not clipboard extensions.
    pub fn to_document(&self, source: &str) -> Document {
        let mut document = Document::new(source);
        document
            .root
            .insert("elements".into(), self.root["elements"].clone());
        if let Some(files) = self.root.get("files") {
            document.root.insert("files".into(), files.clone());
        }
        document
    }
}
