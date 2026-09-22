//! Complete typed wire-field vocabulary for both pinned profiles.
use crate::{Key, Number, Object, Profile, Record};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

macro_rules! ids {
    ($($name:ident),*) => {$ (
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);
        impl From<&str> for $name { fn from(v: &str) -> Self { Self(v.into()) } }
        impl crate::WireValue for $name {
            fn read_wire(v: &Value) -> Result<Self, crate::Error> { <String as crate::WireValue>::read_wire(v).map(Self) }
            fn write_wire(self) -> Value { Value::String(self.0) }
        }
    )*};
}
ids!(ElementId, GroupId, FileId, FractionalIndex);

macro_rules! strings {
    ($name:ident {$($variant:ident => $wire:literal),* $(,)?}) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum $name { $($variant,)* Unknown(String) }
        impl $name {
            pub fn as_str(&self) -> &str { match self { $(Self::$variant => $wire,)* Self::Unknown(v) => v } }
            pub fn from_wire(v: &str) -> Self { match v { $($wire => Self::$variant,)* _ => Self::Unknown(v.into()) } }
            pub const KNOWN: &'static [&'static str] = &[$($wire),*];
        }
        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> { s.serialize_str(self.as_str()) }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> { String::deserialize(d).map(|v| Self::from_wire(&v)) }
        }
        impl crate::WireValue for $name {
            fn read_wire(v: &Value) -> Result<Self, crate::Error> { <String as crate::WireValue>::read_wire(v).map(|v|Self::from_wire(&v)) }
            fn write_wire(self) -> Value { Value::String(self.as_str().to_owned()) }
        }
    };
}
strings!(ElementKind { Rectangle=>"rectangle", Diamond=>"diamond", Ellipse=>"ellipse", Text=>"text", Line=>"line", Arrow=>"arrow", Freedraw=>"freedraw", Image=>"image", Frame=>"frame", Magicframe=>"magicframe", Iframe=>"iframe", Embeddable=>"embeddable", Stickynote=>"stickynote", Selection=>"selection", Draw=>"draw" });
strings!(FillStyle { Solid=>"solid", Hachure=>"hachure", CrossHatch=>"cross-hatch", Zigzag=>"zigzag" });
strings!(StrokeStyle { Solid=>"solid", Dashed=>"dashed", Dotted=>"dotted" });
strings!(TextAlign { Left=>"left", Center=>"center", Right=>"right" });
strings!(VerticalAlign { Top=>"top", Middle=>"middle", Bottom=>"bottom" });
strings!(ImageStatus { Pending=>"pending", Saved=>"saved", Error=>"error" });
strings!(BindingKind { Arrow=>"arrow", Text=>"text" });
strings!(BindMode { Inside=>"inside", Orbit=>"orbit", Skip=>"skip" });
strings!(Variability { Variable=>"variable", Constant=>"constant" });
strings!(GenerationStatus { Pending=>"pending", Done=>"done", Error=>"error" });
strings!(LibraryStatus { Published=>"published", Unpublished=>"unpublished" });
strings!(MimeType { Svg=>"image/svg+xml", Png=>"image/png", Jpeg=>"image/jpeg", Gif=>"image/gif", Webp=>"image/webp", Bmp=>"image/bmp", Icon=>"image/x-icon", Avif=>"image/avif", Jfif=>"image/jfif", Binary=>"application/octet-stream" });
strings!(Arrowhead { Arrow=>"arrow", Bar=>"bar", Circle=>"circle", CircleOutline=>"circle_outline", Triangle=>"triangle", TriangleOutline=>"triangle_outline", Diamond=>"diamond", DiamondOutline=>"diamond_outline", Dot=>"dot", CrowfootOne=>"crowfoot_one", CrowfootMany=>"crowfoot_many", CrowfootOneOrMany=>"crowfoot_one_or_many", CardinalityOne=>"cardinality_one", CardinalityMany=>"cardinality_many", CardinalityOneOrMany=>"cardinality_one_or_many", CardinalityExactlyOne=>"cardinality_exactly_one", CardinalityZeroOrOne=>"cardinality_zero_or_one", CardinalityZeroOrMany=>"cardinality_zero_or_many" });

pub type Point = [Number; 2];
/// Numeric IDs stay open for custom/future fonts and roundness algorithms.
pub type FontFamily = Number;
pub type RoundnessType = Number;

macro_rules! fields {
    ($module:ident, $owner:ident, $alias:ident { $($constant:ident : $ty:ty => $wire:literal),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq)] pub struct $owner;
        pub type $alias = Record<$owner>;
        pub mod $module {
            use super::*;
            $(pub const $constant: Key<$owner, $ty> = Key::new($wire);)*
            pub const FIELDS: &[&str] = &[$($wire),*];
            pub(crate) fn check(record: &$alias) -> Vec<crate::Error> {
                let mut errors = Vec::new();
                $(if let Err(e) = record.get($constant) { errors.push(e); })*
                errors
            }
        }
    }
}
fields!(element, ElementRecord, Element {
    TYPE: ElementKind=>"type", ID: ElementId=>"id", X: Number=>"x", Y: Number=>"y",
    WIDTH: Number=>"width", HEIGHT: Number=>"height", ANGLE: Number=>"angle",
    STROKE_COLOR: String=>"strokeColor", BACKGROUND_COLOR: String=>"backgroundColor",
    FILL_STYLE: FillStyle=>"fillStyle", STROKE_WIDTH: Number=>"strokeWidth", STROKE_STYLE: StrokeStyle=>"strokeStyle",
    ROUNDNESS: Roundness=>"roundness", ROUGHNESS: Number=>"roughness", OPACITY: Number=>"opacity",
    SEED: Number=>"seed", VERSION: Number=>"version", VERSION_NONCE: Number=>"versionNonce",
    INDEX: FractionalIndex=>"index", IS_DELETED: bool=>"isDeleted", GROUP_IDS: Vec<GroupId> =>"groupIds",
    FRAME_ID: ElementId=>"frameId", BOUND_ELEMENTS: Vec<BoundElement> =>"boundElements",
    UPDATED: Number=>"updated", CREATED: Number=>"created", LINK: String=>"link", LOCKED: bool=>"locked", CUSTOM_DATA: Object=>"customData",
    FONT_SIZE: Number=>"fontSize", FONT_FAMILY: FontFamily=>"fontFamily", BASE_FONT_SIZE: Number=>"baseFontSize",
    TEXT: String=>"text", ORIGINAL_TEXT: String=>"originalText", TEXT_ALIGN: TextAlign=>"textAlign",
    VERTICAL_ALIGN: VerticalAlign=>"verticalAlign", CONTAINER_ID: ElementId=>"containerId", AUTO_RESIZE: bool=>"autoResize",
    LINE_HEIGHT: Number=>"lineHeight", LABEL_POSITION: Number=>"labelPosition",
    POINTS: Vec<Point> =>"points", START_BINDING: Binding=>"startBinding", END_BINDING: Binding=>"endBinding",
    START_ARROWHEAD: Arrowhead=>"startArrowhead", END_ARROWHEAD: Arrowhead=>"endArrowhead",
    LAST_COMMITTED_POINT: Point=>"lastCommittedPoint", POLYGON: bool=>"polygon", ELBOWED: bool=>"elbowed",
    FIXED_SEGMENTS: Vec<FixedSegment> =>"fixedSegments", START_IS_SPECIAL: bool=>"startIsSpecial", END_IS_SPECIAL: bool=>"endIsSpecial",
    PRESSURES: Vec<Number> =>"pressures", SIMULATE_PRESSURE: bool=>"simulatePressure", STROKE_OPTIONS: StrokeOptions=>"strokeOptions",
    FILE_ID: FileId=>"fileId", STATUS: ImageStatus=>"status", SCALE: Point=>"scale", CROP: ImageCrop=>"crop",
    NAME: String=>"name", BASE_HEIGHT: Number=>"baseHeight"
});

/// What identities a field's value can carry.
///
/// This is the companion to the element vocabulary directly above, and it exists
/// for one reason: [`Document::remap_ids`](crate::Document::remap_ids) must never
/// pass a reference through untouched merely because it did not recognise the
/// field holding it. Recognition and relabeling are separate facts, so they get
/// separate tables — using the vocabulary itself as the "remap understands this"
/// test silently weakened [`OpaquePolicy::Reject`](crate::OpaquePolicy) every
/// time a field was added.
///
/// Every name in [`element::FIELDS`] appears here exactly once; `tests/remap.rs`
/// asserts both directions. A field added above without a row here is classified
/// as unknown and reported as unassessed, which is the safe direction.
///
/// `link` is [`ReferenceKind::None`]: it carries no identity this crate relabels.
/// Whether a *non-null* link is opaque is a value-dependent policy that stays in
/// `remap_ids`, not a property of the field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReferenceKind {
    /// Geometry, style, text, flags, timestamps: no identity.
    None,
    /// One element id.
    Element,
    /// A list of group ids.
    Group,
    /// One file id.
    File,
    /// A nested record or list whose own vocabulary carries element ids.
    Nested,
    /// Host-defined content this crate will not interpret.
    Opaque,
}

pub(crate) const ELEMENT_REFERENCES: &[(&str, ReferenceKind)] = &[
    ("type", ReferenceKind::None),
    ("id", ReferenceKind::Element),
    ("x", ReferenceKind::None),
    ("y", ReferenceKind::None),
    ("width", ReferenceKind::None),
    ("height", ReferenceKind::None),
    ("angle", ReferenceKind::None),
    ("strokeColor", ReferenceKind::None),
    ("backgroundColor", ReferenceKind::None),
    ("fillStyle", ReferenceKind::None),
    ("strokeWidth", ReferenceKind::None),
    ("strokeStyle", ReferenceKind::None),
    ("roundness", ReferenceKind::None),
    ("roughness", ReferenceKind::None),
    ("opacity", ReferenceKind::None),
    ("seed", ReferenceKind::None),
    ("version", ReferenceKind::None),
    ("versionNonce", ReferenceKind::None),
    ("index", ReferenceKind::None),
    ("isDeleted", ReferenceKind::None),
    ("groupIds", ReferenceKind::Group),
    ("frameId", ReferenceKind::Element),
    ("boundElements", ReferenceKind::Nested),
    ("updated", ReferenceKind::None),
    ("created", ReferenceKind::None),
    ("link", ReferenceKind::None),
    ("locked", ReferenceKind::None),
    ("customData", ReferenceKind::Opaque),
    ("fontSize", ReferenceKind::None),
    ("fontFamily", ReferenceKind::None),
    ("baseFontSize", ReferenceKind::None),
    ("text", ReferenceKind::None),
    ("originalText", ReferenceKind::None),
    ("textAlign", ReferenceKind::None),
    ("verticalAlign", ReferenceKind::None),
    ("containerId", ReferenceKind::Element),
    ("autoResize", ReferenceKind::None),
    ("lineHeight", ReferenceKind::None),
    ("labelPosition", ReferenceKind::None),
    ("points", ReferenceKind::None),
    ("startBinding", ReferenceKind::Nested),
    ("endBinding", ReferenceKind::Nested),
    ("startArrowhead", ReferenceKind::None),
    ("endArrowhead", ReferenceKind::None),
    ("lastCommittedPoint", ReferenceKind::None),
    ("polygon", ReferenceKind::None),
    ("elbowed", ReferenceKind::None),
    ("fixedSegments", ReferenceKind::None),
    ("startIsSpecial", ReferenceKind::None),
    ("endIsSpecial", ReferenceKind::None),
    ("pressures", ReferenceKind::None),
    ("simulatePressure", ReferenceKind::None),
    ("strokeOptions", ReferenceKind::None),
    ("fileId", ReferenceKind::File),
    ("status", ReferenceKind::None),
    ("scale", ReferenceKind::None),
    ("crop", ReferenceKind::None),
    ("name", ReferenceKind::None),
    ("baseHeight", ReferenceKind::None),
];

/// The classification for `field`, or `None` when the field is not classified —
/// which callers must treat as "not understood", never as "carries nothing".
pub(crate) fn element_reference_kind(field: &str) -> Option<ReferenceKind> {
    ELEMENT_REFERENCES
        .iter()
        .find(|(name, _)| *name == field)
        .map(|(_, kind)| *kind)
}
fields!(binding, BindingRecord, Binding { ELEMENT_ID: ElementId=>"elementId", FOCUS: Number=>"focus", GAP: Number=>"gap", FIXED_POINT: Point=>"fixedPoint", MODE: BindMode=>"mode" });
fields!(bound_element, BoundElementRecord, BoundElement { ID: ElementId=>"id", TYPE: BindingKind=>"type" });
fields!(roundness, RoundnessRecord, Roundness { TYPE: RoundnessType=>"type", VALUE: Number=>"value" });
fields!(fixed_segment, FixedSegmentRecord, FixedSegment { START: Point=>"start", END: Point=>"end", INDEX: Number=>"index" });
fields!(stroke_options, StrokeOptionsRecord, StrokeOptions { VARIABILITY: Variability=>"variability", STREAMLINE: Number=>"streamline" });
fields!(image_crop, ImageCropRecord, ImageCrop { X: Number=>"x", Y: Number=>"y", WIDTH: Number=>"width", HEIGHT: Number=>"height", NATURAL_WIDTH: Number=>"naturalWidth", NATURAL_HEIGHT: Number=>"naturalHeight" });
fields!(generation_data, GenerationDataRecord, GenerationData { STATUS: GenerationStatus=>"status", HTML: String=>"html", CODE: String=>"code", MESSAGE: String=>"message" });
fields!(binary_file, BinaryFileRecord, BinaryFile { ID: FileId=>"id", MIME_TYPE: MimeType=>"mimeType", DATA_URL: String=>"dataURL", CREATED: Number=>"created", LAST_RETRIEVED: Number=>"lastRetrieved", VERSION: Number=>"version" });
fields!(app_state, AppStateRecord, AppState { GRID_SIZE: Number=>"gridSize", GRID_STEP: Number=>"gridStep", GRID_MODE_ENABLED: bool=>"gridModeEnabled", VIEW_BACKGROUND_COLOR: String=>"viewBackgroundColor", LOCKED_MULTI_SELECTIONS: BTreeMap<GroupId, bool> =>"lockedMultiSelections" });
fields!(library_item, LibraryItemRecord, LibraryItem { ID: String=>"id", STATUS: LibraryStatus=>"status", CREATED: Number=>"created", ELEMENTS: Vec<Element> =>"elements", NAME: String=>"name", ERROR: String=>"error" });

impl Element {
    pub fn kind(&self) -> Result<crate::Field<ElementKind>, crate::Error> {
        self.get(element::TYPE)
    }
    /// Construct a complete default record with explicit identity/time. Callers
    /// supply geometry, text dimensions, bindings and assets through typed fields.
    /// Degenerate defaults remain subject to explicit authored validation.
    pub fn new(
        kind: ElementKind,
        profile: Profile,
        id: ElementId,
        updated: Number,
    ) -> Result<Self, crate::Error> {
        if matches!(
            kind,
            ElementKind::Selection | ElementKind::Draw | ElementKind::Unknown(_)
        ) || (profile == Profile::V0_18_1 && kind == ElementKind::Stickynote)
        {
            return Err(crate::Error::at(
                "/type",
                "kind is not authored in selected profile",
            ));
        }
        let mut object = json!({"type":kind.as_str(),"id":id,"x":0,"y":0,"width":0,"height":0,"angle":0,
            "strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":2,"strokeStyle":"solid",
            "roundness":null,"roughness":1,"opacity":100,"seed":1,"version":1,"versionNonce":0,"index":null,
            "isDeleted":false,"groupIds":[],"frameId":null,"boundElements":null,"updated":updated,"link":null,"locked":false}).as_object().unwrap().clone();
        if profile == Profile::SnapshotAfa3a653 {
            object.insert("created".into(), Value::Null);
        }
        let extra = match kind {
            ElementKind::Text => {
                json!({"fontSize":20,"fontFamily":5,"text":"","originalText":"","textAlign":"left","verticalAlign":"top","containerId":null,"autoResize":true,"lineHeight":1.25})
            }
            ElementKind::Line | ElementKind::Arrow => {
                json!({"points":[[0,0],[0,0]],"startBinding":null,"endBinding":null,"startArrowhead":null,"endArrowhead":null})
            }
            ElementKind::Freedraw => json!({"points":[],"pressures":[],"simulatePressure":true}),
            ElementKind::Image => {
                json!({"fileId":null,"status":"pending","scale":[1,1],"crop":null})
            }
            ElementKind::Frame | ElementKind::Magicframe => json!({"name":null}),
            ElementKind::Stickynote => json!({"baseHeight":0,"backgroundColor":"#ffdf6b"}),
            _ => json!({}),
        };
        object.extend(extra.as_object().unwrap().clone());
        if kind == ElementKind::Arrow {
            object.insert("elbowed".into(), json!(false));
            object.insert("endArrowhead".into(), json!("arrow"));
        }
        if profile == Profile::SnapshotAfa3a653 {
            match kind {
                ElementKind::Line => {
                    object.insert("polygon".into(), json!(false));
                }
                ElementKind::Text => {
                    object.insert("baseFontSize".into(), Value::Null);
                }
                ElementKind::Freedraw => {
                    object.insert(
                        "strokeOptions".into(),
                        json!({"variability":"variable","streamline":0.5}),
                    );
                }
                _ => {}
            }
        } else if matches!(
            kind,
            ElementKind::Line | ElementKind::Arrow | ElementKind::Freedraw
        ) {
            object.insert("lastCommittedPoint".into(), Value::Null);
        }
        Ok(Self::from_object(object))
    }

    /// Atomically replace a line, arrow or freehand path and its bounds.
    /// Nonempty paths must start at local `[0, 0]`; the element's position and
    /// supplied points are never translated. Width and height are the spans of
    /// the point extrema, using checked finite f64 geometry. Empty and degenerate
    /// paths are allowed as drafts and have zero bounds on their empty axes.
    /// Routing, bindings and freehand pressure samples remain caller-owned.
    pub fn set_path(&mut self, points: Vec<Point>) -> Result<(), crate::Error> {
        if !matches!(
            self.kind()?,
            crate::Field::Value(ElementKind::Line | ElementKind::Arrow | ElementKind::Freedraw)
        ) {
            return Err(crate::Error::at(
                "/type",
                "path requires a line, arrow or freedraw element",
            ));
        }
        let mut min = [0.0_f64; 2];
        let mut max = [0.0_f64; 2];
        for (i, point) in points.iter().enumerate() {
            for (axis, coordinate) in point.iter().enumerate() {
                let value = coordinate
                    .as_f64()
                    .map_err(|e| crate::Error::at(format!("/points/{i}/{axis}"), e.message))?;
                // Check exact zero so a tiny nonzero JSON number cannot underflow
                // to zero and silently bypass the local-origin requirement.
                if i == 0 && coordinate.as_safe_integer() != Ok(0) {
                    return Err(crate::Error::at(
                        format!("/points/0/{axis}"),
                        "path must start at local [0, 0]",
                    ));
                }
                min[axis] = min[axis].min(value);
                max[axis] = max[axis].max(value);
            }
        }
        let width =
            Number::from_f64(max[0] - min[0]).map_err(|e| crate::Error::at("/width", e.message))?;
        let height = Number::from_f64(max[1] - min[1])
            .map_err(|e| crate::Error::at("/height", e.message))?;
        let mut next = self.clone();
        next.set(element::POINTS, points)?;
        next.set(element::WIDTH, width)?;
        next.set(element::HEIGHT, height)?;
        *self = next;
        Ok(())
    }
}

/// Explicit revision/lifetime metadata for authored elements. No clock or random
/// source is consulted by the document model.
#[derive(Clone, Debug)]
pub struct Authoring {
    pub id: ElementId,
    pub updated: Number,
    /// Missing for v0.18.1; null or a safe-integer timestamp for the snapshot.
    pub created: crate::Field<Number>,
    pub seed: Number,
    pub version: Number,
    pub version_nonce: Number,
}
impl Element {
    /// Construct with checked profile-specific presence and safe-integer metadata.
    /// Revisions must be positive. Geometry may still be a zero-sized draft.
    pub fn authored(
        kind: ElementKind,
        profile: Profile,
        metadata: Authoring,
    ) -> Result<Self, crate::Error> {
        match (profile, &metadata.created) {
            (Profile::V0_18_1, crate::Field::Missing)
            | (Profile::SnapshotAfa3a653, crate::Field::Null | crate::Field::Value(_)) => {}
            _ => {
                return Err(crate::Error::at(
                    "/created",
                    "created must be missing for v0.18.1 and present (possibly null) for snapshot",
                ));
            }
        }
        for (path, value) in [
            ("/updated", &metadata.updated),
            ("/seed", &metadata.seed),
            ("/version", &metadata.version),
            ("/versionNonce", &metadata.version_nonce),
        ] {
            let integer = value
                .as_safe_integer()
                .map_err(|e| crate::Error::at(path, e.message))?;
            if path == "/version" && integer <= 0 {
                return Err(crate::Error::at(path, "element revision must be positive"));
            }
        }
        if let crate::Field::Value(created) = &metadata.created {
            created
                .as_safe_integer()
                .map_err(|e| crate::Error::at("/created", e.message))?;
        }
        let mut element = Self::new(kind, profile, metadata.id, metadata.updated)?;
        element.set(element::SEED, metadata.seed)?;
        element.set(element::VERSION, metadata.version)?;
        element.set(element::VERSION_NONCE, metadata.version_nonce)?;
        match metadata.created {
            crate::Field::Missing => element.remove(element::CREATED),
            crate::Field::Null => element.set_null(element::CREATED),
            crate::Field::Value(v) => element.set(element::CREATED, v)?,
        }
        Ok(element)
    }
    pub fn elbow_arrow(
        profile: Profile,
        metadata: Authoring,
        points: Vec<Point>,
    ) -> Result<Self, crate::Error> {
        let mut element = Self::authored(ElementKind::Arrow, profile, metadata)?;
        element.set_path(points)?;
        element.set(element::ELBOWED, true)?;
        element.set_null(element::FIXED_SEGMENTS);
        element.set_null(element::START_IS_SPECIAL);
        element.set_null(element::END_IS_SPECIAL);
        Ok(element)
    }

    /// Construct a snapshot sticky note with matching height and baseline.
    /// Dimensions must be finite and nonnegative; zero-sized drafts are allowed.
    pub fn sticky_note(
        profile: Profile,
        metadata: Authoring,
        width: Number,
        height: Number,
    ) -> Result<Self, crate::Error> {
        let mut element = Self::authored(ElementKind::Stickynote, profile, metadata)?;
        for (path, value) in [("/width", &width), ("/height", &height)] {
            value.check_dimension(path)?;
        }
        element.set(element::WIDTH, width)?;
        element.set(element::BASE_HEIGHT, height.clone())?;
        element.set(element::HEIGHT, height)?;
        Ok(element)
    }
}
