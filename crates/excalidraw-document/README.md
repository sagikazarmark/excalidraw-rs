# excalidraw-document

[![crates.io](https://img.shields.io/crates/v/excalidraw-document?style=flat-square)](https://crates.io/crates/excalidraw-document)
[![docs.rs](https://img.shields.io/docsrs/excalidraw-document?style=flat-square)](https://docs.rs/excalidraw-document)

**Preserving typed Excalidraw scene documents, validation and versioned export.**

`excalidraw-document` preserves scene and library JSON with typed access to the complete
wire-field vocabulary of Excalidraw **0.18.1** and source snapshot
**afa3a653fc5d2b742adcbd5a6063187b056d2419**.

## Install

Requires Rust **1.97+**. To use a local checkout, adjust the path:

```toml
[dependencies]
excalidraw-document = { path = "../excaliplot.orig/crates/excalidraw-document", version = "0.1.0" }
```

See the [release guide](https://github.com/sagikazarmark/excaliplot.orig/blob/main/docs/releasing.md)
for distribution status and publication steps.

## Quick Start

```rust
use excalidraw_document::{Document, Number, element};

let input = br#"{"type":"excalidraw","elements":[{"type":"rectangle","id":"box","x":1,"future":{"retained":true}}]}"#;
let mut document = Document::from_slice(input)?;
document.edit_element(0, |shape| {
    shape.set(element::X, Number::from_f64(42.5)?)
})?;
let output = document.to_vec_pretty()?;
# Ok::<(), excalidraw_document::Error>(())
```

## Contracts

- `Document::from_slice` rejects duplicate keys, malformed JSON and unrecognizable
  envelopes. It preserves unknown fields/types, missing/null states, malformed
  element entries, ordered arrays, tombstones, exact numbers and unused resources.
- `Element`, `Binding`, `ImageCrop`, `BinaryFile`, `AppState`, etc. are record
  types backed by one authoritative JSON object. Their modules expose typed
  `Key<Owner, Value>` constants. `get` returns `Field::{Missing, Null, Value}` or
  a type error. A type error never discards the stored field.
- `Element::new` supplies ordinary defaults for any supported kind;
  `Element::authored` accepts explicit revision/lifetime metadata and
  `Element::elbow_arrow` initializes the elbow-specific fields. Geometry and text
  dimensions remain caller supplied. `validated` returns an immutable checked
  borrow that can perform native projection without stale validation.
- `set`, `set_null`, `remove` and `set_raw` are deliberate edits. No automatic
  revisions, timestamps, ID generation, font layout, repair or file pruning.
  `edit_element` commits only if the callback succeeds.
- All 13 snapshot persisted kinds (12 in .1), both binding generations, every
  nested field, style/head enum, image/crop/file metadata, sticky notes and AI
  metadata have typed descriptors. Unknown enum values remain open.
- `validate(profile, purpose)` reports field/profile/graph/resource problems
  without mutation. `Inspect` allows missing historical fields; `Author` requires
  complete declared records; `SelfContained` additionally requires referenced files.
  Validation is not an editor renderer or a proof that resources decode.
- Semantic checks cover known field/kind applicability, profile-specific binding
  targets and legacy fields, primary font IDs, roundness algorithms, positive text
  metrics, recorded pressure counts, polygon closure, elbow segment indices and
  endpoints, and crop extents. Historical/transient consistency issues are warnings
  under `Inspect` and errors under `Author`/`SelfContained`; inactive routing
  metadata left by arrow-type conversion stays warning-only. Malformed types,
  impossible indices and invalid scalar ranges remain errors (streamline's
  renderer-domain check is a consistency warning during inspection). Unknown fields are
  preserved and are not interpreted as geometry. Simulated pressure may be empty,
  fixed-point ratios may lie outside `0..1`, and crop sizes have no UI minimum.
- `project_native(profile, mode, source)` is an explicit serializer projection,
  returning path-addressed changes. It prunes appState/files and applies .1's
  tombstone/linear-state cleanup or snapshot's element retention. It does not
  migrate schemas. Numbers that JavaScript would change are rejected by this
  operation while remaining supported by preserving encoding.
- `LibraryDocument` preserves v1/v2 JSON separately; v2 item identities are scoped
  independently. It does not automatically migrate v1 or invent library assets.

The guarantee is semantic JSON preservation, not original formatting/object-key
order/number spelling. Unicode strings must be valid; JSON nesting is bounded to
128. `from_value` cannot recover duplicate keys discarded by a previous parser.
Use `Number::from_f64` for checked finite geometry; no Plotters coordinate, color,
font or glyph restrictions apply to document input. Source `version: 2` alone does
not select an editor profile.

## Programmatic authoring

`Document::author(profile)` borrows a complete document exclusively and returns a
`SceneAuthor`. Operations build and validate a candidate before committing, so
errors leave the original unchanged. They use `Purpose::Author`; check
`SelfContained` separately when all referenced resources must be included.

Use `author.batch` to apply multiple edits with **one candidate clone and one final
validation**:

```rust
# use excalidraw_document::*;
# fn update(author: &mut SceneAuthor<'_>) -> Result<(), Error> {
author.batch(|batch| {
    batch.replace_text(&"label".into(), TextContent::plain("Updated", 84_u64.into(), 25_u64.into()))?;
    batch.bind_label(&"label".into(), &"box".into())?;
    Ok(())
})
# }
```

The callback queues owned arguments; operations execute in order after it returns.
All operation errors are reported by `batch`, including when an enqueue result was
ignored. Callback errors/panics, execution errors and invalid final documents leave
the original unchanged. The first execution error aborts; final-only graph
constraints may be temporarily violated and repaired by subsequent queued edits.
The queue supports text replacement, label/arrow bind/unbind, connected translation,
grouping and reordering. Insertions, frame reassignment, duplication and resource
operations use their existing standalone transactions. The callback cannot inspect
the intermediate document. Empty batches are no-ops; metadata policies are unchanged.

- `insert` appends a batch, checks identities/references, and assigns ordered
  fractional indices to the entire scene, including tombstones.
- `replace_text` updates source text, displayed text and supplied dimensions
  together. `TextContent::plain` is convenient for unwrapped content; explicit
  `original`/`display` fields support wrapping without a font engine.
- `bind_label` / `unbind_label` maintain reciprocal references and label uniqueness.
  Binding moves a preceding label after its container and reindexes if needed.
- `bind_arrow` / `unbind_arrow` maintain endpoint bindings and shared reverse
  references, including rebinding and two endpoints targeting the same element.
  `BindingGeometry` makes release versus snapshot geometry explicit.
- Existing positions, styles, revision/time metadata and custom data remain caller
  supplied. Operations do not route arrows, measure text or fit containers.
  Standalone operations clone and validate the scene. Use `batch` for multiple
  supported edits and `insert` with a vector for bulk insertion.

```rust
use excalidraw_document::{Document, Element, ElementKind, Profile, TextContent, element};

let profile = Profile::V0_18_1;
let mut shape = Element::new(ElementKind::Rectangle, profile, "box".into(), 1_u64.into())?;
shape.set(element::WIDTH, 120_u64.into())?;
shape.set(element::HEIGHT, 60_u64.into())?;
let mut text = Element::new(ElementKind::Text, profile, "label".into(), 1_u64.into())?;
text.set_text_content(TextContent::plain("Hello", 60_u64.into(), 25_u64.into()))?;
let mut document = Document::new("my-generator");
let mut author = document.author(profile)?;
author.insert(vec![shape, text])?;
author.bind_label(&"label".into(), &"box".into())?;
# Ok::<(), excalidraw_document::Error>(())
```

`Element::set_path` updates local points and their bounds atomically for lines,
arrows and freehand strokes. Nonempty paths must begin at `[0, 0]`; position is
unchanged. `elbow_arrow` uses it. `sticky_note` accepts size and initializes its
height baseline with a nontransparent default color. `Element::authored` rejects
profile-incompatible creation metadata and invalid revision/timestamp integers.

Run the complete Rust-only diagram example:

```sh
cargo run -p excalidraw-document --example authored_diagram -- release > diagram.excalidraw
# Use "snapshot" for the pinned source profile.
```

The example creates two labeled boxes and a bound arrow, then replaces a label's
content. Both pinned browser suites load this output, check relationship/path/order
invariants, move a container and its connected arrow, open the replaced text in the
native editor, edit it, and save/reopen it. Caller-supplied fixed-point geometry may
still be normalized by upstream; the example explicitly supplies the snapshot's
stable `0.5001` midpoint convention.

### Composition and assets

`SceneAuthor` also provides:

- `set_frame(ids, frame)` to attach/detach whole group and label units. It arranges
  affected children before their frame in contiguous blocks. Creating nested frames
  is rejected because the pinned editors do not move grandchildren coherently.
  Generators can instead use `set_frame_with_order(..., FrameOrder::Preserve)` to
  retain deliberate array order and indices, including crossing selection groups.
  This keeps the same reference/nesting validation; the generator is responsible
  for placing children before their frame.
- `group(ids, group)` / `ungroup(group)` to create a fresh outer group or remove
  one group level and its lock. Group members are gathered into stable contiguous
  blocks; inner groups remain intact. Grouping across frame memberships is rejected.
- `translate_connected(ids, delta)` to move an entire connected composition: groups, frame
  descendants, labels, connected arrows **and their endpoint targets**. This moves
  all connected geometry equally instead of routing an arrow to a stationary target.
- `reorder(ids, before)` to move whole composition blocks and update indices.
  `None` appends. Frames and labels remain together.
- `duplicate(ids, mapping, delta, policy)` to copy a reference-complete composition
  using explicit fresh element/group IDs, optional new file IDs, and translation.
  File-ID freshness includes external references and tombstones. Revisions, seeds
  and timestamps stay supplied data. `Preserve` reports unassessed extension paths;
  `Reject` refuses opaque metadata instead of guessing its references.
- `register_file(file)` / `insert_image(image, file)` to register a resource under
  its own ID and atomically attach it to an image with status `Saved`. Identical
  complete resources deduplicate; a different record at an existing ID is rejected.

`BinaryFile::new` accepts an ID, MIME type, matching data URL and creation timestamp.
It checks metadata and URL shape without decoding the image. `LibraryItem::new`
creates a validated unpublished item from elements. `insert_library_item` takes
an item, complete fresh element/group mapping, its resources, an offset and opaque
policy. Repeated insertions retain internal bindings without cross-copy identities.
Resources are passed separately; native library JSON has no standard files field.
See method rustdoc for selection expansion, identity and diagnostic-path policies.

The `authored_gallery` example emits a JSON bundle of complete scenes and a library
for the browser suite:

```sh
cargo run -p excalidraw-document --example authored_gallery -- snapshot > gallery.json
```

It exercises every persisted kind plus image crop/flip, freehand resize, multiline
editing, frame/group movement, isolated copies, library loading, elbow fixed
segments, sticky growth/shrink, and historical tombstone load/save policies.

### Typed inspection conveniences

- `source`, `version`, `elements_field`, `app_state_field` and `files_field` return
  `Field<T>`, preserving missing/null/value distinctions at the scene root. Matching
  `set_*` methods accept those states; `remove_root` supports metadata removal while
  guarding the required document discriminator.
- `generation_data` / `set_generation_data` access known iframe metadata within
  `customData`, preserving siblings and parent presence on removal.
- `KnownFont` and `KnownRoundness` provide named numeric values and profile-aware
  lookup. Open wire numbers still retain unknown IDs.
- `Element::view` and `Binding::view` discriminate kinds and binding generations.
  `BindingView::PartialMixed` retains incomplete/mixed shapes; malformed known
  fields return errors. These are structural views, not validation certificates.

## Explicit transformations and transports

- `Document::remap_ids(&IdMap, OpaquePolicy)` applies simultaneous element/group/
  file substitutions and rewrites known bindings, frame membership, group locks
  and asset references. Collisions, duplicate identities and unresolved known
  references fail atomically. `Reject` refuses opaque metadata; `Preserve` leaves
  it untouched and returns `unassessed_paths`. Links and custom metadata are never
  guessed or recursively string-replaced. Relabeling does not reset revision/time
  metadata or implement a complete editor duplication operation.
- `Document::migrate(from, to)` implements exact non-geometric conversions with
  path-addressed changes. Arrowhead renames, legacy draw, default polygon/freehand/
  text fields and interaction metadata are handled explicitly. Nonnull arrow
  bindings, sticky-note downgrades and unsupported semantic losses return blockers.
  The supplied source profile is a caller assertion; the result is not automatically
  certified as a complete authored document. Preserved creation metadata can remain
  a release extension. Validate the result for the intended purpose.
- `ClipboardDocument` preserves the native clipboard envelope. Explicit scene
  conversion selects elements/files; it does not simulate clipboard selection,
  frame detachment or paste-time identity/seed changes.
- Feature **`embedded`** enables `embedded::{embed_png, extract_png, embed_svg,
  extract_svg}`. It stores preserving scene JSON in Excalidraw's compressed byte-
  string payload, with PNG CRC/chunk checks and SVG XML parsing. Extraction takes
  an explicit uncompressed byte limit. Legacy raw JSON and SVG payload versions
  1/2 are supported. Existing scene metadata is replaced; ambiguous/overlapping
  containers fail. No image rendering or raster decoding is performed.
- `plus::{SceneContent, ReplaceSceneContent, PatchSceneContent}` provides data-only
  adapters for the public API contract observed 2026-09-12. Scene versions and
  failed-embedding diagnostics stay separate from native documents. Unknown root,
  appState and file fields are rejected rather than silently stripped; omission in
  PATCH differs from empty arrays/maps. Element schemas are incomplete publicly:
  `unconfirmed_element_paths` reports known documentation gaps, not a complete
  server validator. No HTTP, authentication, CAS or reconciliation is implemented.

The crate has no renderer, HTTP client, async runtime, font assets or filesystem
overwrite policy. Default features remain empty; only embedded transport adds
compression/base64/XML dependencies.

Run `cargo test -p excalidraw-document`. The workspace's
`compatibility/document/` runner compares native projections with both pinned
editors in Chromium. Source provenance and detailed plans live under `docs/` in
the repository.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
