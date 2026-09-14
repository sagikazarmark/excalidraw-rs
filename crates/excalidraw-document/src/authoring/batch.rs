use super::{ArrowEndpoint, BindingGeometry, SceneAuthor, TextContent, check};
use crate::{Element, ElementId, Error, GroupId, Point};
use serde_json::Value;

/// An ordered queue of relationship-aware edits, created by [`SceneAuthor::batch`].
///
/// Methods only enqueue owned arguments and currently return `Ok(())`: all
/// operation errors (including unknown/deleted IDs, wrong kinds, invalid geometry
/// and group conflicts) are deferred to `batch`. Use `?` as with single edits.
/// Ignoring a method's result cannot suppress an execution error or commit a
/// partial batch. There is no candidate-document access during the callback.
///
/// Edits execute in order and observe earlier edits. The first execution error
/// aborts the batch, even if a later edit could repair it. Graph constraints checked
/// only by final Author validation may be temporarily violated and repaired: for
/// example, bind a replacement label before unbinding the previous label. If an
/// intermediate graph prevents a composition operation, that operation aborts
/// the batch; later repair does not retry it.
///
/// Supported operations are text replacement, label/arrow binding and unbinding,
/// connected translation, grouping and reordering. Their geometry, metadata and
/// ordering policies match the corresponding [`SceneAuthor`] methods. Insertion,
/// duplication, frame reassignment, ungrouping, resource edits and raw record
/// mutation are deliberately outside this API.
pub struct AuthoringBatch {
    edits: Vec<BatchEdit>,
}

// This closed set is the safety boundary for execution without per-edit Author
// validation. Starting from an Author-valid document, these operations cannot
// introduce duplicate/missing identities, malformed records, frame cycles or
// dangling references on success. Composition's index assumptions remain valid
// even with temporary label/target-kind conflicts. Stop on *any* operation error:
// existing implementations may have partially modified the private candidate.
enum BatchEdit {
    ReplaceText(ElementId, TextContent),
    BindLabel(ElementId, ElementId),
    UnbindLabel(ElementId),
    BindArrow(ElementId, ArrowEndpoint, ElementId, BindingGeometry),
    UnbindArrow(ElementId, ArrowEndpoint),
    TranslateConnected(Vec<ElementId>, Point),
    Group(Vec<ElementId>, GroupId),
    Reorder(Vec<ElementId>, Option<ElementId>),
}

impl AuthoringBatch {
    /// Queue [`SceneAuthor::replace_text`]; errors are deferred to `batch`.
    pub fn replace_text(&mut self, id: &ElementId, content: TextContent) -> Result<(), Error> {
        self.edits.push(BatchEdit::ReplaceText(id.clone(), content));
        Ok(())
    }

    /// Queue [`SceneAuthor::bind_label`]; errors are deferred to `batch`.
    pub fn bind_label(&mut self, label: &ElementId, container: &ElementId) -> Result<(), Error> {
        self.edits
            .push(BatchEdit::BindLabel(label.clone(), container.clone()));
        Ok(())
    }

    /// Queue [`SceneAuthor::unbind_label`]; errors are deferred to `batch`.
    pub fn unbind_label(&mut self, label: &ElementId) -> Result<(), Error> {
        self.edits.push(BatchEdit::UnbindLabel(label.clone()));
        Ok(())
    }

    /// Queue [`SceneAuthor::bind_arrow`]; errors are deferred to `batch`.
    pub fn bind_arrow(
        &mut self,
        arrow: &ElementId,
        endpoint: ArrowEndpoint,
        target: &ElementId,
        geometry: BindingGeometry,
    ) -> Result<(), Error> {
        self.edits.push(BatchEdit::BindArrow(
            arrow.clone(),
            endpoint,
            target.clone(),
            geometry,
        ));
        Ok(())
    }

    /// Queue [`SceneAuthor::unbind_arrow`]; errors are deferred to `batch`.
    pub fn unbind_arrow(
        &mut self,
        arrow: &ElementId,
        endpoint: ArrowEndpoint,
    ) -> Result<(), Error> {
        self.edits
            .push(BatchEdit::UnbindArrow(arrow.clone(), endpoint));
        Ok(())
    }

    /// Queue [`SceneAuthor::translate_connected`]; errors are deferred to `batch`.
    pub fn translate_connected(&mut self, ids: &[ElementId], delta: Point) -> Result<(), Error> {
        self.edits
            .push(BatchEdit::TranslateConnected(ids.to_vec(), delta));
        Ok(())
    }

    /// Queue [`SceneAuthor::group`]; errors are deferred to `batch`.
    pub fn group(&mut self, ids: &[ElementId], group: GroupId) -> Result<(), Error> {
        self.edits.push(BatchEdit::Group(ids.to_vec(), group));
        Ok(())
    }

    /// Queue [`SceneAuthor::reorder`]; errors are deferred to `batch`.
    pub fn reorder(&mut self, ids: &[ElementId], before: Option<&ElementId>) -> Result<(), Error> {
        self.edits
            .push(BatchEdit::Reorder(ids.to_vec(), before.cloned()));
        Ok(())
    }
}

impl SceneAuthor<'_> {
    /// Queue edits, execute them on one owned candidate, then validate once and
    /// commit atomically. The original is untouched on callback error, execution
    /// error, final validation failure, or panic. Panics propagate normally; the
    /// author remains usable if the caller catches an unwind.
    ///
    /// The callback runs before cloning or execution. All queued operation errors
    /// are deferred to this method; see [`AuthoringBatch`] for intermediate-graph
    /// rules and supported operations. An empty successful batch is a no-op.
    ///
    /// ```
    /// # use excalidraw_document::*;
    /// # fn update(author: &mut SceneAuthor<'_>) -> Result<(), Error> {
    /// author.batch(|batch| {
    ///     batch.replace_text(&"label".into(), TextContent::plain("Updated", 80_u64.into(), 25_u64.into()))?;
    ///     batch.bind_label(&"label".into(), &"box".into())?;
    ///     Ok(())
    /// })
    /// # }
    /// ```
    pub fn batch(
        &mut self,
        edit: impl FnOnce(&mut AuthoringBatch) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut batch = AuthoringBatch { edits: Vec::new() };
        edit(&mut batch)?;
        if batch.edits.is_empty() {
            return Ok(());
        }

        let mut next = self.document.clone();
        // Move records out of the sole clone: Document::elements() would clone
        // every element again. The root is restored before final validation.
        let Some(Value::Array(values)) = next.root.remove("elements") else {
            return Err(Error::at("/elements", "expected array"));
        };
        let elements = values
            .into_iter()
            .enumerate()
            .map(|(i, value)| match value {
                Value::Object(object) => Ok(Element::from_object(object)),
                _ => Err(Error::at(
                    format!("/elements/{i}"),
                    "expected element object",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut staged = SceneAuthor {
            document: &mut next,
            profile: self.profile,
            staged_elements: Some(elements),
        };
        for edit in batch.edits {
            match edit {
                BatchEdit::ReplaceText(id, content) => staged.replace_text(&id, content),
                BatchEdit::BindLabel(label, container) => staged.bind_label(&label, &container),
                BatchEdit::UnbindLabel(label) => staged.unbind_label(&label),
                BatchEdit::BindArrow(arrow, endpoint, target, geometry) => {
                    staged.bind_arrow(&arrow, endpoint, &target, geometry)
                }
                BatchEdit::UnbindArrow(arrow, endpoint) => staged.unbind_arrow(&arrow, endpoint),
                BatchEdit::TranslateConnected(ids, delta) => {
                    staged.translate_connected(&ids, delta)
                }
                BatchEdit::Group(ids, group) => staged.group(&ids, group),
                BatchEdit::Reorder(ids, before) => staged.reorder(&ids, before.as_ref()),
            }?;
        }
        let elements = staged
            .staged_elements
            .take()
            .expect("batch owns staged elements");
        next.set_elements(elements);
        check(&next, self.profile)?;
        *self.document = next;
        Ok(())
    }
}
