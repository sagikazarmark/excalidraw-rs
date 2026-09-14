//! Optional structural views over preserving records, not validation certificates.
use crate::{
    BindMode, Binding, BindingRecord, Element, ElementId, ElementKind, ElementRecord, Error, Field,
    GenerationData, Key, Number, Object, Point, Profile, WireValue, binding, element,
};
use serde_json::Value;

impl Element {
    /// Inspect `customData.generationData`. A missing/null parent propagates its
    /// presence; use `CUSTOM_DATA` to distinguish parent null from child null.
    /// Generation-data children remain independently inspectable typed fields.
    pub fn generation_data(&self) -> Result<Field<GenerationData>, Error> {
        match self.get(element::CUSTOM_DATA)? {
            Field::Missing => Ok(Field::Missing),
            Field::Null => Ok(Field::Null),
            Field::Value(custom) => match custom.get("generationData") {
                None => Ok(Field::Missing),
                Some(Value::Null) => Ok(Field::Null),
                Some(value) => GenerationData::read_wire(value)
                    .map(Field::Value)
                    .map_err(|e| {
                        Error::at(format!("/customData/generationData{}", e.path), e.message)
                    }),
            },
        }
    }

    /// Edit only the nested field, retaining siblings and empty parent objects.
    /// Removal leaves missing/null parents untouched. Setting null or a record
    /// materializes an object parent. A malformed parent always fails atomically.
    pub fn set_generation_data(&mut self, data: Field<GenerationData>) -> Result<(), Error> {
        let mut custom = match self.get(element::CUSTOM_DATA)? {
            Field::Value(custom) => custom,
            Field::Missing | Field::Null if matches!(data, Field::Missing) => return Ok(()),
            Field::Missing | Field::Null => Object::new(),
        };
        match data {
            Field::Missing => {
                custom.remove("generationData");
            }
            Field::Null => {
                custom.insert("generationData".into(), Value::Null);
            }
            Field::Value(data) => {
                custom.insert("generationData".into(), data.write_wire());
            }
        }
        self.set(element::CUSTOM_DATA, custom)
    }

    pub fn view(&self) -> Result<ElementView<'_>, Error> {
        ElementView::new(self)
    }
}

/// Named entries in upstream `FONT_FAMILY`, not a closed wire enum.
/// Sources: `packages/excalidraw/constants.ts` at a2ec2889babf7d2295469c6d90ebe77fae57df84
/// and `packages/common/src/constants.ts` at afa3a653fc5d2b742adcbd5a6063187b056d2419.
/// Reserved ID 4 and the separate fallback registries are intentionally excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnownFont {
    Virgil,
    Helvetica,
    Cascadia,
    Excalifont,
    Nunito,
    LilitaOne,
    ComicShanns,
    LiberationSans,
    Assistant,
}
impl KnownFont {
    pub fn name(self) -> &'static str {
        match self {
            Self::Virgil => "Virgil",
            Self::Helvetica => "Helvetica",
            Self::Cascadia => "Cascadia",
            Self::Excalifont => "Excalifont",
            Self::Nunito => "Nunito",
            Self::LilitaOne => "Lilita One",
            Self::ComicShanns => "Comic Shanns",
            Self::LiberationSans => "Liberation Sans",
            Self::Assistant => "Assistant",
        }
    }
    pub fn to_number(self) -> Number {
        Number::from(match self {
            Self::Virgil => 1_i64,
            Self::Helvetica => 2,
            Self::Cascadia => 3,
            Self::Excalifont => 5,
            Self::Nunito => 6,
            Self::LilitaOne => 7,
            Self::ComicShanns => 8,
            Self::LiberationSans => 9,
            Self::Assistant => 10,
        })
    }
    pub fn from_number(profile: Profile, number: &Number) -> Option<Self> {
        Some(match number.as_safe_integer().ok()? {
            1 => Self::Virgil,
            2 => Self::Helvetica,
            3 => Self::Cascadia,
            5 => Self::Excalifont,
            6 => Self::Nunito,
            7 => Self::LilitaOne,
            8 => Self::ComicShanns,
            9 => Self::LiberationSans,
            10 if profile == Profile::SnapshotAfa3a653 => Self::Assistant,
            _ => return None,
        })
    }
}

/// Named `ROUNDNESS` entries, identical in both pinned constants files cited above.
/// Unknown numeric algorithms continue to use the open `RoundnessType` alias.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnownRoundness {
    Legacy,
    ProportionalRadius,
    AdaptiveRadius,
}
impl KnownRoundness {
    pub fn name(self) -> &'static str {
        match self {
            Self::Legacy => "LEGACY",
            Self::ProportionalRadius => "PROPORTIONAL_RADIUS",
            Self::AdaptiveRadius => "ADAPTIVE_RADIUS",
        }
    }
    pub fn to_number(self) -> Number {
        Number::from(match self {
            Self::Legacy => 1_i64,
            Self::ProportionalRadius => 2,
            Self::AdaptiveRadius => 3,
        })
    }
    pub fn from_number(_profile: Profile, number: &Number) -> Option<Self> {
        Some(match number.as_safe_integer().ok()? {
            1 => Self::Legacy,
            2 => Self::ProportionalRadius,
            3 => Self::AdaptiveRadius,
            _ => return None,
        })
    }
}

/// Discrimination by a decoded kind only, independent of profile or authored
/// validity. Missing, null and malformed kinds are errors. Other malformed fields
/// are reported by `get`; a `Text` view does not certify valid text or geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ElementView<'a> {
    Rectangle(&'a Element),
    Diamond(&'a Element),
    Ellipse(&'a Element),
    Text(&'a Element),
    Line(&'a Element),
    Arrow(&'a Element),
    Freedraw(&'a Element),
    Image(&'a Element),
    Frame(&'a Element),
    Magicframe(&'a Element),
    Iframe(&'a Element),
    Embeddable(&'a Element),
    Stickynote(&'a Element),
    /// Historical draw or nonpersisted selection records; inspect `kind()` for which.
    Legacy(&'a Element),
    /// An unknown string kind; the original name and extensions remain in the record.
    Unknown(&'a Element),
}
impl<'a> ElementView<'a> {
    pub fn new(element: &'a Element) -> Result<Self, Error> {
        Ok(match element.kind()? {
            Field::Value(kind) => match kind {
                ElementKind::Rectangle => Self::Rectangle(element),
                ElementKind::Diamond => Self::Diamond(element),
                ElementKind::Ellipse => Self::Ellipse(element),
                ElementKind::Text => Self::Text(element),
                ElementKind::Line => Self::Line(element),
                ElementKind::Arrow => Self::Arrow(element),
                ElementKind::Freedraw => Self::Freedraw(element),
                ElementKind::Image => Self::Image(element),
                ElementKind::Frame => Self::Frame(element),
                ElementKind::Magicframe => Self::Magicframe(element),
                ElementKind::Iframe => Self::Iframe(element),
                ElementKind::Embeddable => Self::Embeddable(element),
                ElementKind::Stickynote => Self::Stickynote(element),
                ElementKind::Selection | ElementKind::Draw => Self::Legacy(element),
                ElementKind::Unknown(_) => Self::Unknown(element),
            },
            Field::Missing | Field::Null => {
                return Err(Error::at("/type", "expected nonnull element kind"));
            }
        })
    }
    pub fn element(&self) -> &'a Element {
        match self {
            Self::Rectangle(e)
            | Self::Diamond(e)
            | Self::Ellipse(e)
            | Self::Text(e)
            | Self::Line(e)
            | Self::Arrow(e)
            | Self::Freedraw(e)
            | Self::Image(e)
            | Self::Frame(e)
            | Self::Magicframe(e)
            | Self::Iframe(e)
            | Self::Embeddable(e)
            | Self::Stickynote(e)
            | Self::Legacy(e)
            | Self::Unknown(e) => e,
        }
    }
    pub fn get<T: WireValue>(&self, key: Key<ElementRecord, T>) -> Result<Field<T>, Error> {
        self.element().get(key)
    }
}

/// Explicit structural binding discrimination. All known fields are decoded
/// before classification, so malformed fields cannot be swallowed by a fallback.
/// Null is presence, not absence: e.g. a null `mode` prevents release classification.
/// Numeric ranges, supported mode names and target validity require validation.
/// Parsed fields are owned; `binding()` retains the original record and extensions.
#[derive(Clone, Debug, PartialEq)]
pub enum BindingView<'a> {
    ReleaseOrdinary {
        binding: &'a Binding,
        element_id: ElementId,
        focus: Number,
        gap: Number,
    },
    ReleaseFixed {
        binding: &'a Binding,
        element_id: ElementId,
        focus: Number,
        gap: Number,
        fixed_point: Point,
    },
    Snapshot {
        binding: &'a Binding,
        element_id: ElementId,
        fixed_point: Point,
        mode: BindMode,
    },
    PartialMixed {
        binding: &'a Binding,
        element_id: Field<ElementId>,
        focus: Field<Number>,
        gap: Field<Number>,
        fixed_point: Field<Point>,
        mode: Field<BindMode>,
    },
}
impl Binding {
    pub fn view(&self) -> Result<BindingView<'_>, Error> {
        BindingView::new(self)
    }
}
impl<'a> BindingView<'a> {
    pub fn new(binding: &'a Binding) -> Result<Self, Error> {
        let element_id = binding.get(binding::ELEMENT_ID)?;
        let focus = binding.get(binding::FOCUS)?;
        let gap = binding.get(binding::GAP)?;
        let fixed_point = binding.get(binding::FIXED_POINT)?;
        let mode = binding.get(binding::MODE)?;
        Ok(match (element_id, focus, gap, fixed_point, mode) {
            (
                Field::Value(element_id),
                Field::Value(focus),
                Field::Value(gap),
                Field::Missing,
                Field::Missing,
            ) => Self::ReleaseOrdinary {
                binding,
                element_id,
                focus,
                gap,
            },
            (
                Field::Value(element_id),
                Field::Value(focus),
                Field::Value(gap),
                Field::Value(fixed_point),
                Field::Missing,
            ) => Self::ReleaseFixed {
                binding,
                element_id,
                focus,
                gap,
                fixed_point,
            },
            (
                Field::Value(element_id),
                Field::Missing,
                Field::Missing,
                Field::Value(fixed_point),
                Field::Value(mode),
            ) => Self::Snapshot {
                binding,
                element_id,
                fixed_point,
                mode,
            },
            (element_id, focus, gap, fixed_point, mode) => Self::PartialMixed {
                binding,
                element_id,
                focus,
                gap,
                fixed_point,
                mode,
            },
        })
    }
    pub fn binding(&self) -> &'a Binding {
        match self {
            Self::ReleaseOrdinary { binding, .. }
            | Self::ReleaseFixed { binding, .. }
            | Self::Snapshot { binding, .. }
            | Self::PartialMixed { binding, .. } => binding,
        }
    }
    pub fn get<T: WireValue>(&self, key: Key<BindingRecord, T>) -> Result<Field<T>, Error> {
        self.binding().get(key)
    }
}
