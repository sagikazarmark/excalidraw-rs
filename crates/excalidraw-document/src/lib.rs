//! Preserving typed Excalidraw scene and library documents, validation, and
//! versioned native export.
//!
//! [`Document`] preserves scene JSON while exposing typed fields for Excalidraw
//! 0.18.1 and source snapshot `afa3a653fc5d2b742adcbd5a6063187b056d2419`.
//! Select the schema explicitly with [`Profile`]; the scene envelope's version
//! alone does not identify an editor profile.
//!
//! # Inspect and edit JSON
//!
//! ```
//! use excalidraw_document::{Document, Number, element};
//!
//! let input = br#"{"type":"excalidraw","elements":[{"type":"rectangle","id":"box","x":1,"future":{"retained":true}}]}"#;
//! let mut document = Document::from_slice(input)?;
//! document.edit_element(0, |shape| {
//!     shape.set(element::X, Number::from_f64(42.5)?)
//! })?;
//! let output = document.to_vec_pretty()?;
//! # Ok::<(), excalidraw_document::Error>(())
//! ```
//!
//! Parsing rejects duplicate keys, malformed JSON, and unrecognizable envelopes.
//! Preserving encoding retains unknown fields and kinds, missing/null states,
//! malformed element entries, array order, tombstones, exact numbers, and unused
//! resources. This is semantic JSON preservation: formatting, object-key order,
//! and number spelling may change. Nesting is bounded to 128 levels.
//!
//! Records such as [`Element`], [`Binding`], [`BinaryFile`], and [`AppState`] have
//! one authoritative JSON object. Their modules expose typed [`Key`] constants;
//! getters return [`Field::Missing`], [`Field::Null`], or [`Field::Value`], or a
//! type error without discarding the stored value. Setters are explicit edits;
//! [`Document::edit_element`] commits only when its callback succeeds.
//!
//! # Author a diagram
//!
//! [`Element::new`] supplies profile-specific defaults. Geometry, IDs, timestamps,
//! and text dimensions remain caller supplied. [`Document::author`] opens an
//! exclusive [`SceneAuthor`] session whose operations validate a candidate before
//! committing. Errors leave the original document unchanged.
//!
//! ```
//! use excalidraw_document::{Document, Element, ElementKind, Profile, TextContent, element};
//!
//! let profile = Profile::V0_18_1;
//! let mut shape = Element::new(ElementKind::Rectangle, profile, "box".into(), 1_u64.into())?;
//! shape.set(element::WIDTH, 120_u64.into())?;
//! shape.set(element::HEIGHT, 60_u64.into())?;
//! let mut text = Element::new(ElementKind::Text, profile, "label".into(), 1_u64.into())?;
//! text.set_text_content(TextContent::plain("Hello", 60_u64.into(), 25_u64.into()))?;
//!
//! let mut document = Document::new("my-generator");
//! let mut author = document.author(profile)?;
//! author.insert(vec![shape, text])?;
//! author.batch(|batch| {
//!     batch.replace_text(&"label".into(), TextContent::plain("Updated", 84_u64.into(), 25_u64.into()))?;
//!     batch.bind_label(&"label".into(), &"box".into())?;
//!     Ok(())
//! })?;
//! # Ok::<(), excalidraw_document::Error>(())
//! ```
//!
//! [`SceneAuthor::batch`] queues supported edits in order with one candidate clone
//! and one final validation. Standalone operations also support insertion,
//! frame assignment, duplication, and resources. Authoring maintains known
//! relationships and ordering; it does not measure text, route arrows, fit
//! containers, or advance revision/time metadata automatically.
//!
//! # Validation and export
//!
//! [`Document::validate`] reports field, profile, graph, and resource problems
//! without mutation. [`Purpose::Inspect`] permits missing historical fields;
//! [`Purpose::Author`] requires complete declared records;
//! [`Purpose::SelfContained`] additionally requires referenced files. Validation
//! is not an editor renderer or proof that embedded resources decode.
//!
//! [`Document::validated`] returns an immutable checked borrow.
//! [`Document::project_native`] explicitly applies a profile's serializer rules,
//! returning path-addressed changes. Projection may prune app state, files, or
//! tombstones according to the selected profile and export mode; it does not
//! migrate schemas. Numbers that JavaScript would change are rejected by native
//! projection while remaining supported by preserving encoding.
//!
//! # Other document operations
//!
//! - [`LibraryDocument`] preserves v1/v2 libraries with independently scoped item
//!   identities; it does not automatically migrate v1 or invent library assets.
//! - [`Document::remap_ids`] rewrites known identities and references atomically.
//!   [`OpaquePolicy`] controls how uninterpreted extension data is handled.
//! - [`Document::migrate`] performs supported non-geometric profile conversions
//!   and reports blockers for unsupported losses.
//! - [`ClipboardDocument`] preserves native clipboard envelopes without
//!   simulating editor selection or paste-time identity changes.
//! - The optional `embedded` feature enables the `embedded` module's PNG/SVG scene
//!   metadata codecs. These embed and extract scene JSON; they do not render images.
//! - [`plus`] contains data-only Excalidraw Plus API envelopes; it does not make
//!   HTTP requests or implement authentication or synchronization.
//!
//! Default features are empty. The crate has no renderer, font assets, async
//! runtime, HTTP client, or filesystem overwrite policy. Document input is not
//! restricted to the Plotters backend's coordinates, fonts, or glyph subset.

// The README's examples are contracts too, and nothing compiled them: they are
// near-copies of the doctests above that drifted independently. `cfg(doctest)`
// compiles their fences without prepending the README to the rendered docs.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme {}

mod authoring;
mod clipboard;
mod document;
mod library;
mod migration;
pub use clipboard::ClipboardDocument;
#[cfg(feature = "embedded")]
pub mod embedded;
pub mod model;
pub mod plus;
mod projection;
mod remap;
mod validation;
mod views;
mod wire;
pub use migration::MigrationResult;
pub use remap::{IdMap, OpaquePolicy, RemapResult};
pub use views::*;

pub use authoring::{
    ArrowEndpoint, AuthoringBatch, BindingGeometry, FrameOrder, SceneAuthor, TextContent,
};
pub use document::{Document, ValidatedDocument};
pub use library::LibraryDocument;
pub use model::*;
pub use projection::{Change, ExportMode, ExportResult};
pub use validation::{Diagnostic, Purpose, Severity, ValidationReport};
pub use wire::{Error, Field, Key, Number, Object, Record, WireValue};

/// Immutable editor schema/serializer identity (independent of scene version 2).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    V0_18_1,
    SnapshotAfa3a653,
}

impl Profile {
    pub fn source_commit(self) -> &'static str {
        match self {
            Self::V0_18_1 => "a2ec2889babf7d2295469c6d90ebe77fae57df84",
            Self::SnapshotAfa3a653 => "afa3a653fc5d2b742adcbd5a6063187b056d2419",
        }
    }
}

/// The element field vocabulary, for tests that assert it stays in step with
/// the reference classification below. Not part of the supported interface.
#[doc(hidden)]
pub fn element_fields() -> &'static [&'static str] {
    model::element::FIELDS
}

/// The names carrying a reference classification. Paired with
/// [`element_fields`]; see `tests/remap.rs`. Not part of the supported interface.
#[doc(hidden)]
pub fn classified_element_fields() -> Vec<&'static str> {
    model::ELEMENT_REFERENCES
        .iter()
        .map(|(name, _)| *name)
        .collect()
}
