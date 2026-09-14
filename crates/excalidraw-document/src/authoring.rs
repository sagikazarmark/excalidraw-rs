//! Explicit, transactional authoring above the preserving record interface.
mod batch;
mod composition;
mod resources;
use crate::{
    BindMode, Binding, BindingKind, BoundElement, Document, Element, ElementId, ElementKind, Error,
    Field, Number, Point, Profile, Purpose, Severity, binding, bound_element, element,
};
pub use batch::AuthoringBatch;
pub use composition::FrameOrder;

/// Source and displayed text, with caller-measured dimensions in scene units.
/// For unwrapped text, use [`TextContent::plain`]. No font measurement is performed.
#[derive(Clone, Debug)]
pub struct TextContent {
    pub original: String,
    pub display: String,
    pub width: Number,
    pub height: Number,
}
impl TextContent {
    pub fn plain(text: impl Into<String>, width: Number, height: Number) -> Self {
        let text = text.into();
        Self {
            original: text.clone(),
            display: text,
            width,
            height,
        }
    }
}

/// The endpoint being changed. Arrow geometry remains caller-owned.
#[derive(Clone, Copy, Debug)]
pub enum ArrowEndpoint {
    Start,
    End,
}
impl ArrowEndpoint {
    fn key(self) -> crate::Key<crate::ElementRecord, Binding> {
        match self {
            Self::Start => element::START_BINDING,
            Self::End => element::END_BINDING,
        }
    }
}

/// Explicit binding geometry for the selected editor profile.
/// Release elbow arrows require `fixed_point: Some(..)`.
#[derive(Clone, Debug)]
pub enum BindingGeometry {
    Release {
        focus: Number,
        gap: Number,
        fixed_point: Option<Point>,
    },
    Snapshot {
        fixed_point: Point,
        mode: BindMode,
    },
}
impl BindingGeometry {
    fn binding(self, profile: Profile, target: ElementId) -> Result<Binding, Error> {
        let mut result = Binding::default();
        result.set(binding::ELEMENT_ID, target)?;
        match (profile, self) {
            (
                Profile::V0_18_1,
                Self::Release {
                    focus,
                    gap,
                    fixed_point,
                },
            ) => {
                result.set(binding::FOCUS, focus)?;
                result.set(binding::GAP, gap)?;
                if let Some(point) = fixed_point {
                    result.set(binding::FIXED_POINT, point)?;
                }
            }
            (Profile::SnapshotAfa3a653, Self::Snapshot { fixed_point, mode }) => {
                result.set(binding::FIXED_POINT, fixed_point)?;
                result.set(binding::MODE, mode)?;
            }
            _ => {
                return Err(Error::at(
                    "/binding",
                    "binding geometry does not match authoring profile",
                ));
            }
        }
        Ok(result)
    }
}

/// An exclusive, profile-bound authoring session. Every operation validates a
/// candidate document before committing; an error leaves the original untouched.
/// [`SceneAuthor::batch`] amortizes cloning and validation over multiple edits.
/// Revision/time metadata is never advanced automatically. Insertions and label
/// moves assign fresh ordered indices to the entire scene (including tombstones).
pub struct SceneAuthor<'a> {
    document: &'a mut Document,
    profile: Profile,
    // Only the restricted batch executor constructs this state. Its operations
    // preserve identities and record shapes, but may temporarily violate graph
    // constraints. Never expose this author or document to a caller.
    staged_elements: Option<Vec<Element>>,
}
impl Document {
    /// Begin authoring a complete scene. Historical/partial documents must first
    /// be completed explicitly through the preserving interface.
    pub fn author(&mut self, profile: Profile) -> Result<SceneAuthor<'_>, Error> {
        check(self, profile)?;
        Ok(SceneAuthor {
            document: self,
            profile,
            staged_elements: None,
        })
    }
}

impl SceneAuthor<'_> {
    fn transaction_document(
        &mut self,
        edit: impl FnOnce(&mut Document) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut next = self.document.clone();
        edit(&mut next)?;
        check(&next, self.profile)?;
        *self.document = next;
        Ok(())
    }
    pub fn document(&self) -> &Document {
        self.document
    }

    /// Append a batch atomically. IDs and all known references must be valid in
    /// the resulting scene. Assigns ordered indices; preserves revision metadata.
    pub fn insert(&mut self, additions: Vec<Element>) -> Result<(), Error> {
        self.transaction(|elements| {
            elements.extend(additions);
            reindex(elements)
        })
    }

    /// Replace source/display content and dimensions together. Position, font,
    /// wrapping policy and revision metadata are retained.
    pub fn replace_text(&mut self, id: &ElementId, content: TextContent) -> Result<(), Error> {
        self.transaction(|elements| {
            let i = live_index(elements, id)?;
            elements[i]
                .set_text_content(content)
                .map_err(|e| Error::at(format!("/elements/{i}{}", e.path), e.message))
        })
    }

    /// Attach or rebind a label, maintaining both directions and placing it after
    /// its container. Existing labels are rejected rather than silently replaced.
    /// Label position/dimensions remain supplied data.
    pub fn bind_label(&mut self, label: &ElementId, container: &ElementId) -> Result<(), Error> {
        self.transaction(|elements| {
            let label_index = live_index(elements, label)?;
            require_scene_kind(elements, label_index, ElementKind::Text)?;
            let container_index = live_index(elements, container)?;
            let old = reference(&elements[label_index], element::CONTAINER_ID)?;
            if old.as_ref() == Some(container) {
                return Ok(());
            }
            if let Some(old) = old {
                remove_reverse(elements, &old, label, BindingKind::Text)?;
            }
            elements[label_index].set(element::CONTAINER_ID, container.clone())?;
            add_reverse(&mut elements[container_index], label, BindingKind::Text)?;
            if label_index < container_index {
                let label = elements.remove(label_index);
                elements.insert(container_index, label);
                reindex(elements)?;
            }
            Ok(())
        })
    }

    pub fn unbind_label(&mut self, label: &ElementId) -> Result<(), Error> {
        self.transaction(|elements| {
            let i = live_index(elements, label)?;
            require_scene_kind(elements, i, ElementKind::Text)?;
            if let Some(old) = reference(&elements[i], element::CONTAINER_ID)? {
                remove_reverse(elements, &old, label, BindingKind::Text)?;
            }
            elements[i].set_null(element::CONTAINER_ID);
            Ok(())
        })
    }

    /// Bind or rebind one endpoint. The reverse reference is shared when both
    /// endpoints target the same element. No routing or endpoint movement occurs.
    pub fn bind_arrow(
        &mut self,
        arrow: &ElementId,
        endpoint: ArrowEndpoint,
        target: &ElementId,
        geometry: BindingGeometry,
    ) -> Result<(), Error> {
        let binding = geometry.binding(self.profile, target.clone())?;
        self.change_arrow(arrow, endpoint, Some(binding))
    }

    pub fn unbind_arrow(
        &mut self,
        arrow: &ElementId,
        endpoint: ArrowEndpoint,
    ) -> Result<(), Error> {
        self.change_arrow(arrow, endpoint, None)
    }

    fn change_arrow(
        &mut self,
        arrow: &ElementId,
        endpoint: ArrowEndpoint,
        next: Option<Binding>,
    ) -> Result<(), Error> {
        self.transaction(|elements| {
            let i = live_index(elements, arrow)?;
            require_scene_kind(elements, i, ElementKind::Arrow)?;
            let old = binding_target(&elements[i], endpoint)?;
            if let Some(next) = next {
                let merged = match elements[i].get(endpoint.key())? {
                    Field::Value(existing) => existing,
                    _ => Binding::default(),
                };
                // Replace operation-owned geometry, preserving unknown binding
                // metadata even when rebinding to a different target.
                let mut object = merged.into_object();
                for key in binding::FIELDS {
                    object.remove(*key);
                }
                object.extend(next.into_object());
                elements[i].set(endpoint.key(), Binding::from_object(object))?;
            } else {
                elements[i].set_null(endpoint.key());
            }
            if let Some(old) = old {
                let still_bound = [ArrowEndpoint::Start, ArrowEndpoint::End]
                    .into_iter()
                    .map(|end| binding_target(&elements[i], end))
                    .collect::<Result<Vec<_>, _>>()?
                    .contains(&Some(old.clone()));
                if !still_bound {
                    remove_reverse(elements, &old, arrow, BindingKind::Arrow)?;
                }
            }
            if let Some(target) = binding_target(&elements[i], endpoint)? {
                let target_index = live_index(elements, &target)?;
                add_reverse(&mut elements[target_index], arrow, BindingKind::Arrow)?;
            }
            Ok(())
        })
    }

    fn transaction(
        &mut self,
        edit: impl FnOnce(&mut Vec<Element>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        if let Some(elements) = &mut self.staged_elements {
            return edit(elements);
        }
        let mut elements = self.document.elements()?;
        edit(&mut elements)?;
        let mut next = self.document.clone();
        next.set_elements(elements);
        check(&next, self.profile)?;
        *self.document = next;
        Ok(())
    }
}

impl Element {
    /// Replace content and caller-measured bounds atomically. The preserving
    /// `set(TEXT, ..)` remains available for intentional field-local edits.
    pub fn set_text_content(&mut self, content: TextContent) -> Result<(), Error> {
        require_kind(self, ElementKind::Text)?;
        for (key, value) in [("width", &content.width), ("height", &content.height)] {
            value.check_dimension(&format!("/{key}"))?;
        }
        self.set(element::ORIGINAL_TEXT, content.original)?;
        self.set(element::TEXT, content.display)?;
        self.set(element::WIDTH, content.width)?;
        self.set(element::HEIGHT, content.height)?;
        Ok(())
    }
}

fn check(document: &Document, profile: Profile) -> Result<(), Error> {
    let report = document.validate(profile, Purpose::Author);
    if let Some(d) = report
        .diagnostics
        .into_iter()
        .find(|d| d.severity == Severity::Error)
    {
        return Err(Error::at(d.path, format!("{}: {}", d.code, d.message)));
    }
    Ok(())
}
fn require_kind(element: &Element, kind: ElementKind) -> Result<(), Error> {
    if element.kind()? != Field::Value(kind.clone()) {
        return Err(Error::at("/type", format!("expected {}", kind.as_str())));
    }
    Ok(())
}
fn require_scene_kind(elements: &[Element], i: usize, kind: ElementKind) -> Result<(), Error> {
    require_kind(&elements[i], kind)
        .map_err(|e| Error::at(format!("/elements/{i}{}", e.path), e.message))
}
fn live_index(elements: &[Element], id: &ElementId) -> Result<usize, Error> {
    let i = elements
        .iter()
        .position(|e| e.as_object().get("id").and_then(serde_json::Value::as_str) == Some(&id.0))
        .ok_or_else(|| Error::at("/elements", format!("missing element {}", id.0)))?;
    if elements[i].get(element::IS_DELETED)? == Field::Value(true) {
        return Err(Error::at(
            format!("/elements/{i}/isDeleted"),
            "cannot author a deleted element",
        ));
    }
    Ok(i)
}
fn reference(
    element: &Element,
    key: crate::Key<crate::ElementRecord, ElementId>,
) -> Result<Option<ElementId>, Error> {
    Ok(match element.get(key)? {
        Field::Value(id) => Some(id),
        _ => None,
    })
}
fn binding_target(element: &Element, endpoint: ArrowEndpoint) -> Result<Option<ElementId>, Error> {
    Ok(match element.get(endpoint.key())? {
        Field::Value(b) => match b.get(binding::ELEMENT_ID)? {
            Field::Value(id) => Some(id),
            _ => None,
        },
        _ => None,
    })
}
fn reverses(element: &Element) -> Result<Vec<BoundElement>, Error> {
    Ok(match element.get(element::BOUND_ELEMENTS)? {
        Field::Value(v) => v,
        _ => vec![],
    })
}
fn add_reverse(element: &mut Element, id: &ElementId, kind: BindingKind) -> Result<(), Error> {
    let mut refs = reverses(element)?;
    if !refs.iter().any(|r| {
        r.get(bound_element::ID) == Ok(Field::Value(id.clone()))
            && r.get(bound_element::TYPE) == Ok(Field::Value(kind.clone()))
    }) {
        let mut reverse = BoundElement::default();
        reverse.set(bound_element::ID, id.clone())?;
        reverse.set(bound_element::TYPE, kind)?;
        refs.push(reverse);
        element.set(element::BOUND_ELEMENTS, refs)?;
    }
    Ok(())
}
fn remove_reverse(
    elements: &mut [Element],
    target: &ElementId,
    id: &ElementId,
    kind: BindingKind,
) -> Result<(), Error> {
    let i = live_index(elements, target)?;
    let mut refs = reverses(&elements[i])?;
    refs.retain(|r| {
        !(r.get(bound_element::ID) == Ok(Field::Value(id.clone()))
            && r.get(bound_element::TYPE) == Ok(Field::Value(kind.clone())))
    });
    elements[i].set(element::BOUND_ELEMENTS, refs)
}
fn reindex(elements: &mut [Element]) -> Result<(), Error> {
    const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    for (i, element) in elements.iter_mut().enumerate() {
        let mut n = i;
        let mut digits = vec![DIGITS[n % 62]];
        while n >= 62 {
            n /= 62;
            digits.push(DIGITS[n % 62]);
        }
        digits.push(b'a' + (digits.len() - 1) as u8);
        digits.reverse();
        let index = String::from_utf8(digits).expect("ASCII fractional index");
        element.set(element::INDEX, crate::FractionalIndex(index))?;
    }
    Ok(())
}
