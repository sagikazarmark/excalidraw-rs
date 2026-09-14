//! Relationship-aware composition. All operations retain revision/time/seed
//! metadata, including duplication: new IDs do not imply regenerated revisions.
//! Geometry arithmetic uses checked finite f64 scene coordinates, as in set_path.
//! Selection expansion rejects tombstones reached through relationships or groups;
//! ungroup alone also edits tombstone memberships. Empty selections are allowed
//! except when creating a group; requested IDs must always be unique and live.

use super::{ArrowEndpoint, SceneAuthor, binding_target, live_index, reference, reindex};
use crate::{
    Document, Element, ElementId, ElementKind, Error, Field, GroupId, IdMap, Number, OpaquePolicy,
    Point, app_state, bound_element, element,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Painter-order policy for frame assignment.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FrameOrder {
    /// Gather native child-before-frame and group/label blocks, reassigning indices.
    #[default]
    Normalize,
    /// Retain array order and exact indices, including null indices. Intended for
    /// generators with deliberate painter order and potentially crossing groups;
    /// the caller owns placement of children before their frame.
    Preserve,
}

impl SceneAuthor<'_> {
    /// Reparent selection roots, expanding groups, frame descendants and bound
    /// label/container pairs. Memberships internal to the selection are retained.
    /// A destination inside the closure is rejected (even for an apparent no-op).
    /// Frame-like roots cannot be assigned to another frame: native editors do
    /// not support creating nested frames. Historical nested input is preserved.
    /// Normalize affected old/destination frames to contiguous child-before-frame
    /// blocks and retain nested group/label units. Reassign ordered indices.
    /// Reject crossing historical memberships that cannot form nested blocks.
    pub fn set_frame(&mut self, ids: &[ElementId], frame: Option<&ElementId>) -> Result<(), Error> {
        self.set_frame_with_order(ids, frame, FrameOrder::Normalize)
    }

    /// Assign frame membership with an explicit painter-order policy. Both
    /// policies expand the same groups, descendants and label/container pairs,
    /// reject invalid destinations and nested frame roots, and validate the
    /// candidate atomically. Preserve changes only frame memberships: it does not
    /// gather blocks or reject crossing groups merely because they cannot form
    /// nested blocks. Under Preserve, geometry, revisions and indices remain untouched.
    pub fn set_frame_with_order(
        &mut self,
        ids: &[ElementId],
        frame: Option<&ElementId>,
        order: FrameOrder,
    ) -> Result<(), Error> {
        self.transaction(|elements| {
            let selected = closure(elements, ids, false, false)?;
            if let Some(frame) = frame {
                let i = live_index(elements, frame)?;
                if !matches!(
                    elements[i].kind()?,
                    Field::Value(ElementKind::Frame | ElementKind::Magicframe)
                ) {
                    return Err(Error::at("/frameId", "destination must be a frame"));
                }
                if selected.contains(&i) {
                    return Err(Error::at("/frameId", "destination is inside the selection"));
                }
            }
            if selected.is_empty() {
                return Ok(());
            }
            let selected_ids = identities(elements, &selected)?;
            let mut affected = selected_ids.clone();
            if let Some(frame) = frame {
                affected.insert(frame.clone());
            }
            for &i in &selected {
                if reference(&elements[i], element::FRAME_ID)?
                    .is_some_and(|id| selected_ids.contains(&id))
                {
                    continue;
                }
                if frame.is_some() && is_frame(&elements[i])? {
                    return Err(Error::at(
                        "/frameId",
                        "cannot assign a frame or magicframe to another frame",
                    ));
                }
                if let Some(old) = reference(&elements[i], element::FRAME_ID)? {
                    affected.insert(old);
                }
                match frame {
                    Some(frame) => elements[i].set(element::FRAME_ID, frame.clone())?,
                    None => elements[i].set_null(element::FRAME_ID),
                }
            }
            if order == FrameOrder::Preserve {
                return Ok(());
            }
            if let Some(frame) = frame {
                // Native addElementsToFrame puts additions above existing children,
                // below the frame (or above its highest child for historical order).
                let anchor = elements
                    .iter()
                    .enumerate()
                    .filter_map(|(i, e)| {
                        (!selected.contains(&i)
                            && (e.get(element::ID) == Ok(Field::Value(frame.clone()))
                                || reference(e, element::FRAME_ID) == Ok(Some(frame.clone()))))
                        .then_some(i)
                    })
                    .next_back()
                    .expect("destination outside selection");
                let before_anchor =
                    elements[anchor].get(element::ID)? == Field::Value(frame.clone());
                let mut order: Vec<_> = (0..elements.len())
                    .filter(|i| !selected.contains(i))
                    .collect();
                let position =
                    order.iter().position(|i| *i == anchor).unwrap() + usize::from(!before_anchor);
                order.splice(position..position, selected.iter().copied());
                apply_order(elements, order);
            }
            normalize_order(elements, &affected)
        })
    }

    /// Wrap whole existing groups and label/container pairs in a fresh outer
    /// group, retaining inner-to-outer ordering. Frame descendants are included;
    /// every member must have the same immediate frame membership.
    /// Gather at the highest selected position, retaining stable inner group and
    /// label blocks, then normalize the containing frame and reassign indices.
    /// Crossing historical group/frame/label memberships reject atomically.
    pub fn group(&mut self, ids: &[ElementId], group: GroupId) -> Result<(), Error> {
        let occupied = if let Some(elements) = &self.staged_elements {
            groups_with_locks(self.document, all_groups(elements)?)
        } else {
            document_groups(self.document)?
        };
        if occupied.contains(&group) {
            return Err(Error::at(
                "/groupIds",
                "group identity or lock already exists",
            ));
        }
        self.transaction(|elements| {
            let selected = closure(elements, ids, false, false)?;
            if selected.is_empty() || group.0.is_empty() {
                return Err(Error::at(
                    "/groupIds",
                    "new group and selection must be nonempty",
                ));
            }
            let mut membership = None;
            let affected = identities(elements, &selected)?;
            for i in selected {
                let frame = reference(&elements[i], element::FRAME_ID)?;
                if membership
                    .as_ref()
                    .is_some_and(|previous| previous != &frame)
                {
                    return Err(Error::at(
                        "/frameId",
                        "cannot group across frame memberships",
                    ));
                }
                membership = Some(frame);
                let mut groups = groups(&elements[i])?;
                groups.push(group.clone());
                elements[i].set(element::GROUP_IDS, groups)?;
            }
            normalize_order(elements, &affected)
        })
    }

    /// Remove exactly one group level and its snapshot lock. Other levels keep
    /// their ordering. Tombstone memberships are also removed, avoiding stale locks.
    pub fn ungroup(&mut self, group: &GroupId) -> Result<(), Error> {
        self.transaction_document(|document| {
            let mut elements = document.elements()?;
            if !all_groups(&elements)?.contains(group) {
                return Err(Error::at("/groupIds", "unknown group identity"));
            }
            for element in &mut elements {
                let mut path = groups(element)?;
                if path.contains(group) {
                    path.retain(|id| id != group);
                    element.set(element::GROUP_IDS, path)?;
                }
            }
            document.set_elements(elements);
            if let Some(locks) = document
                .root
                .get_mut("appState")
                .and_then(|v| v.get_mut("lockedMultiSelections"))
                .and_then(serde_json::Value::as_object_mut)
            {
                locks.remove(&group.0);
            }
            Ok(())
        })
    }

    /// Move groups, frame descendants, and labels/containers together. Connected
    /// arrows AND their endpoint targets expand transitively to the entire bound
    /// component (including groups/descendants reached there). Containing frames
    /// are not pulled in just because a child moves. No routing is performed:
    /// local points, fixed segments and binding geometry stay unchanged while all
    /// connected origins receive the same delta. Revisions remain unchanged.
    pub fn translate_connected(&mut self, ids: &[ElementId], delta: Point) -> Result<(), Error> {
        self.transaction(|elements| {
            let selected = closure(elements, ids, true, false)?;
            translate_elements(elements, &selected, &delta)
        })
    }

    /// Move a stable scene-order block before another whole composition unit, or
    /// append with None. Both selections expand groups, labels and whole containing
    /// frames (ancestors and descendants); arrows do not expand. An anchor within
    /// the moving unit is rejected. This avoids splitting frames/groups or moving
    /// a label before its container. All scene indices are reassigned.
    pub fn reorder(&mut self, ids: &[ElementId], before: Option<&ElementId>) -> Result<(), Error> {
        self.transaction(|elements| {
            let selected = closure(elements, ids, false, true)?;
            let anchor = if let Some(before) = before {
                let unit = closure(elements, std::slice::from_ref(before), false, true)?;
                if !selected.is_disjoint(&unit) {
                    return Err(Error::at("/before", "anchor overlaps moving composition"));
                }
                unit.first().copied()
            } else {
                None
            };
            if selected.is_empty() {
                return Ok(());
            }
            let insertion = anchor.map_or(elements.len() - selected.len(), |anchor| {
                (0..anchor).filter(|i| !selected.contains(i)).count()
            });
            let (mut block, mut rest) = (Vec::new(), Vec::new());
            for (i, element) in std::mem::take(elements).into_iter().enumerate() {
                if selected.contains(&i) {
                    block.push(element);
                } else {
                    rest.push(element);
                }
            }
            rest.splice(insertion..insertion, block);
            *elements = rest;
            reindex(elements)
        })
    }

    /// Append a translated, reference-complete copy. Closure includes groups,
    /// frame ancestors/descendants, labels/containers and connected arrows/targets.
    /// Every selected element and group requires an explicit, fresh mapping;
    /// extra mappings are rejected. Freshness includes the entire scene, even
    /// tombstones. File freshness includes resource keys and all image references,
    /// even missing external resources. Referenced files are shared unless explicitly
    /// mapped to fresh file IDs. Selected group locks are copied. Closure reaching
    /// a tombstone is rejected rather than guessing how to repair its references.
    ///
    /// Uses remap_ids on the selected temporary scene. OpaquePolicy covers selected
    /// elements and referenced assets (even shared assets); root and unrelated app
    /// metadata are not copied.
    /// Preserve returns unassessed paths in the resulting document and does not
    /// guess opaque references. Revisions, timestamps, seeds and local geometry
    /// are retained; only IDs, origins and ordered indices change.
    pub fn duplicate(
        &mut self,
        ids: &[ElementId],
        mapping: &IdMap,
        delta: Point,
        policy: OpaquePolicy,
    ) -> Result<Vec<String>, Error> {
        let mut paths = Vec::new();
        self.transaction_document(|document| {
            let mut elements = document.elements()?;
            let selected = closure(&elements, ids, true, true)?;
            let copies: Vec<_> = selected.iter().map(|i| elements[*i].clone()).collect();
            let copy_ids = identities(&elements, &selected)?;
            let copy_groups = all_groups(&copies)?;
            complete_map(&copy_ids, &mapping.elements, "/elements")?;
            complete_map(&copy_groups, &mapping.groups, "/groupIds")?;
            fresh_map(
                &identities(&elements, &(0..elements.len()).collect())?,
                &mapping.elements,
                "/elements",
            )?;
            fresh_map(&document_groups(document)?, &mapping.groups, "/groupIds")?;

            let mut temporary = Document::new("composition");
            temporary.set_elements(copies);
            let files = document.files()?;
            let mut selected_files = BTreeMap::new();
            for i in &selected {
                if elements[*i].kind()? == Field::Value(ElementKind::Image)
                    && let Field::Value(id) = elements[*i].get(element::FILE_ID)?
                {
                    let file = files
                        .get(&id)
                        .ok_or_else(|| Error::at("/files", "missing selected asset"))?;
                    selected_files.insert(id, file.clone());
                }
            }
            let mut reserved_files = files.keys().cloned().collect::<BTreeSet<_>>();
            // External resources and tombstones still reserve their file identity.
            for element in &elements {
                if element.kind()? == Field::Value(ElementKind::Image)
                    && let Field::Value(id) = element.get(element::FILE_ID)?
                {
                    reserved_files.insert(id);
                }
            }
            fresh_map(&reserved_files, &mapping.files, "/files")?;
            temporary.set_files(selected_files);
            if let Some(locks) = document
                .root
                .get("appState")
                .and_then(|v| v.get("lockedMultiSelections"))
                .and_then(serde_json::Value::as_object)
            {
                let mut state = temporary.app_state()?;
                state.set(
                    app_state::LOCKED_MULTI_SELECTIONS,
                    locks
                        .iter()
                        .filter(|(id, _)| copy_groups.contains(&GroupId((*id).clone())))
                        .map(|(id, value)| {
                            (
                                GroupId(id.clone()),
                                value.as_bool().expect("validated lock"),
                            )
                        })
                        .collect(),
                )?;
                temporary.set_app_state(state);
            }
            let result = temporary.remap_ids(mapping, policy)?;
            let mut copies = result.document.elements()?;
            let every_copy = (0..copies.len()).collect();
            translate_elements(&mut copies, &every_copy, &delta)?;
            if copies.is_empty() {
                return Ok(());
            }
            let offset = elements.len();
            elements.extend(copies);
            reindex(&mut elements)?;
            document.set_elements(elements);
            let mut merged_files = files;
            for (id, file) in result.document.files()? {
                merged_files.entry(id).or_insert(file);
            }
            document.set_files(merged_files);
            if let Field::Value(locks) = result
                .document
                .app_state()?
                .get(app_state::LOCKED_MULTI_SELECTIONS)?
            {
                let state = document
                    .root
                    .entry("appState")
                    .or_insert_with(|| serde_json::json!({}));
                let target = state
                    .as_object_mut()
                    .expect("validated state")
                    .entry("lockedMultiSelections")
                    .or_insert_with(|| serde_json::json!({}))
                    .as_object_mut()
                    .expect("validated locks");
                for (id, value) in locks {
                    target.insert(id.0, value.into());
                }
            }
            paths = result
                .unassessed_paths
                .into_iter()
                .map(|path| {
                    if let Some(rest) = path.strip_prefix("/elements/")
                        && let Some((index, suffix)) = rest.split_once('/')
                        && let Ok(index) = index.parse::<usize>()
                    {
                        return format!("/elements/{}/{suffix}", offset + index);
                    }
                    for (old, new) in &mapping.files {
                        let prefix = format!("/files{}/", crate::wire::pointer(&old.0));
                        if let Some(suffix) = path.strip_prefix(&prefix) {
                            return format!("/files{}/{suffix}", crate::wire::pointer(&new.0));
                        }
                    }
                    path
                })
                .collect();
            Ok(())
        })?;
        Ok(paths)
    }
}

fn groups(element: &Element) -> Result<Vec<GroupId>, Error> {
    Ok(match element.get(element::GROUP_IDS)? {
        Field::Value(ids) => ids,
        _ => Vec::new(),
    })
}

fn is_frame(element: &Element) -> Result<bool, Error> {
    Ok(matches!(
        element.kind()?,
        Field::Value(ElementKind::Frame | ElementKind::Magicframe)
    ))
}

fn apply_order(elements: &mut Vec<Element>, order: Vec<usize>) {
    let mut old: Vec<_> = std::mem::take(elements).into_iter().map(Some).collect();
    *elements = order
        .into_iter()
        .map(|i| old[i].take().expect("unique order index"))
        .collect();
}

/// Normalize only units connected to this edit. Unit sets must be nested or
/// disjoint; crossing historical memberships have no coherent block ordering.
/// Process inner units first, gathering each at its highest position (native
/// actionGroup), with a frame moved behind its children (native frame ordering).
fn normalize_order(
    elements: &mut Vec<Element>,
    affected: &BTreeSet<ElementId>,
) -> Result<(), Error> {
    struct Unit {
        members: BTreeSet<usize>,
        frame: Option<usize>,
    }
    let index: BTreeMap<_, _> = elements
        .iter()
        .enumerate()
        .map(|(i, e)| {
            Ok((
                reference(e, element::ID)?.ok_or_else(|| Error::at("/id", "missing identity"))?,
                i,
            ))
        })
        .collect::<Result<_, Error>>()?;
    let mut frame_units: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut group_units: BTreeMap<GroupId, BTreeSet<usize>> = BTreeMap::new();
    let mut labels = Vec::new();
    for (i, e) in elements.iter().enumerate() {
        if e.get(element::IS_DELETED)? == Field::Value(true) {
            continue;
        }
        if is_frame(e)? {
            frame_units.entry(i).or_default().insert(i);
        }
        for group in groups(e)? {
            group_units.entry(group).or_default().insert(i);
        }
        if e.kind()? == Field::Value(ElementKind::Text)
            && let Some(container) = reference(e, element::CONTAINER_ID)?
        {
            labels.push(BTreeSet::from([i, live_index(elements, &container)?]));
        }
        let mut parent = reference(e, element::FRAME_ID)?;
        let mut seen = BTreeSet::from([i]);
        while let Some(id) = parent {
            let j = *index
                .get(&id)
                .ok_or_else(|| Error::at("/frameId", "missing frame"))?;
            if !seen.insert(j) {
                return Err(Error::at("/frameId", "cyclic frame membership"));
            }
            frame_units.entry(j).or_default().extend([i, j]);
            parent = reference(&elements[j], element::FRAME_ID)?;
        }
    }
    // A group containing a frame or one side of a label pair owns the whole unit.
    for members in group_units.values_mut() {
        for (frame, children) in &frame_units {
            if members.contains(frame) {
                members.extend(children);
            }
        }
        for pair in &labels {
            if !members.is_disjoint(pair) {
                members.extend(pair);
            }
        }
    }
    let mut units: Vec<_> = labels
        .into_iter()
        .map(|members| Unit {
            members,
            frame: None,
        })
        .chain(group_units.into_values().map(|members| Unit {
            members,
            frame: None,
        }))
        .chain(frame_units.into_iter().map(|(frame, members)| Unit {
            members,
            frame: Some(frame),
        }))
        .collect();
    let mut touched: BTreeSet<_> = affected
        .iter()
        .filter_map(|id| index.get(id).copied())
        .collect();
    loop {
        let previous = touched.len();
        for unit in &units {
            if !unit.members.is_disjoint(&touched) {
                touched.extend(&unit.members);
            }
        }
        if touched.len() == previous {
            break;
        }
    }
    units.retain(|unit| !unit.members.is_disjoint(&touched));
    for (i, a) in units.iter().enumerate() {
        for b in &units[i + 1..] {
            if !a.members.is_disjoint(&b.members)
                && !a.members.is_subset(&b.members)
                && !b.members.is_subset(&a.members)
            {
                return Err(Error::at(
                    "/elements",
                    "crossing group/frame/label memberships cannot form ordered blocks",
                ));
            }
        }
    }
    units.sort_by_key(|unit| (unit.members.len(), unit.frame.is_some()));
    let mut order: Vec<_> = (0..elements.len()).collect();
    for unit in units {
        let highest = order
            .iter()
            .rposition(|i| unit.members.contains(i))
            .unwrap();
        let insertion = order[..=highest]
            .iter()
            .filter(|i| !unit.members.contains(i))
            .count();
        let mut block: Vec<_> = order
            .iter()
            .copied()
            .filter(|i| unit.members.contains(i) && Some(*i) != unit.frame)
            .collect();
        if let Some(frame) = unit.frame {
            block.push(frame);
        }
        order.retain(|i| !unit.members.contains(i));
        order.splice(insertion..insertion, block);
    }
    apply_order(elements, order);
    reindex(elements)
}

fn all_groups(elements: &[Element]) -> Result<BTreeSet<GroupId>, Error> {
    elements
        .iter()
        .map(groups)
        .collect::<Result<Vec<_>, _>>()
        .map(|paths| paths.into_iter().flatten().collect())
}

fn document_groups(document: &Document) -> Result<BTreeSet<GroupId>, Error> {
    Ok(groups_with_locks(
        document,
        all_groups(&document.elements()?)?,
    ))
}

fn groups_with_locks(document: &Document, mut groups: BTreeSet<GroupId>) -> BTreeSet<GroupId> {
    if let Some(locks) = document
        .root
        .get("appState")
        .and_then(|v| v.get("lockedMultiSelections"))
        .and_then(serde_json::Value::as_object)
    {
        groups.extend(locks.keys().cloned().map(GroupId));
    }
    groups
}

fn identities(
    elements: &[Element],
    indices: &BTreeSet<usize>,
) -> Result<BTreeSet<ElementId>, Error> {
    indices
        .iter()
        .map(|i| match elements[*i].get(element::ID)? {
            Field::Value(id) => Ok(id),
            _ => Err(Error::at(format!("/elements/{i}/id"), "missing identity")),
        })
        .collect()
}

fn complete_map<T: Ord>(
    selected: &BTreeSet<T>,
    mapping: &BTreeMap<T, T>,
    path: &str,
) -> Result<(), Error> {
    if selected.iter().ne(mapping.keys()) {
        return Err(Error::at(
            path,
            "mapping must cover exactly the composition closure",
        ));
    }
    Ok(())
}

fn fresh_map<T: Ord>(
    existing: &BTreeSet<T>,
    mapping: &BTreeMap<T, T>,
    path: &str,
) -> Result<(), Error> {
    if mapping.values().any(|id| existing.contains(id)) {
        return Err(Error::at(
            path,
            "mapping destination already exists in the document",
        ));
    }
    // Empty destinations and duplicate destinations are also checked by remap_ids.
    Ok(())
}

/// Build relationship edges once, then walk to a fixed point. Group membership
/// is expanded once per group rather than building a quadratic clique.
fn closure(
    elements: &[Element],
    ids: &[ElementId],
    arrows: bool,
    ancestors: bool,
) -> Result<BTreeSet<usize>, Error> {
    let index: BTreeMap<_, _> = elements
        .iter()
        .enumerate()
        .map(|(i, e)| match e.get(element::ID)? {
            Field::Value(id) => Ok((id, i)),
            _ => Err(Error::at(format!("/elements/{i}/id"), "missing identity")),
        })
        .collect::<Result<_, _>>()?;
    let mut selected = BTreeSet::new();
    for id in ids {
        let &i = index
            .get(id)
            .ok_or_else(|| Error::at("/elements", format!("missing element {}", id.0)))?;
        if elements[i].get(element::IS_DELETED)? == Field::Value(true) {
            return Err(Error::at(
                format!("/elements/{i}/isDeleted"),
                "cannot author a deleted element",
            ));
        }
        if !selected.insert(i) {
            return Err(Error::at("/elements", "duplicate requested identity"));
        }
    }
    let mut edges = vec![Vec::new(); elements.len()];
    let mut members: BTreeMap<GroupId, Vec<usize>> = BTreeMap::new();
    let mut paths = Vec::new();
    for (i, e) in elements.iter().enumerate() {
        let path = groups(e)?;
        for group in &path {
            members.entry(group.clone()).or_default().push(i);
        }
        paths.push(path);
        if e.get(element::IS_DELETED)? == Field::Value(true) {
            continue;
        }
        let mut link = |target: ElementId| -> Result<(), Error> {
            let j = *index
                .get(&target)
                .ok_or_else(|| Error::at("/elements", "unresolved composition reference"))?;
            edges[i].push(j);
            edges[j].push(i);
            Ok(())
        };
        if e.kind()? == Field::Value(ElementKind::Text)
            && let Some(target) = reference(e, element::CONTAINER_ID)?
        {
            link(target)?;
        }
        if arrows {
            if e.kind()? == Field::Value(ElementKind::Arrow) {
                for endpoint in [ArrowEndpoint::Start, ArrowEndpoint::End] {
                    if let Some(target) = binding_target(e, endpoint)? {
                        link(target)?;
                    }
                }
            }
            if let Field::Value(refs) = e.get(element::BOUND_ELEMENTS)? {
                for bound in refs {
                    if let Field::Value(target) = bound.get(bound_element::ID)? {
                        link(target)?;
                    }
                }
            }
        }
        if let Some(parent) = reference(e, element::FRAME_ID)? {
            let j = *index
                .get(&parent)
                .ok_or_else(|| Error::at("/frameId", "unresolved frame"))?;
            edges[j].push(i);
            if ancestors {
                edges[i].push(j);
            }
        }
    }
    let mut queue: VecDeque<_> = selected.iter().copied().collect();
    let mut seen_groups = BTreeSet::new();
    while let Some(i) = queue.pop_front() {
        if elements[i].get(element::IS_DELETED)? == Field::Value(true) {
            return Err(Error::at(
                format!("/elements/{i}/isDeleted"),
                "composition reaches a deleted element",
            ));
        }
        let mut add = |j| {
            if selected.insert(j) {
                queue.push_back(j);
            }
        };
        for j in &edges[i] {
            add(*j);
        }
        for group in &paths[i] {
            if seen_groups.insert(group) {
                for j in &members[group] {
                    add(*j);
                }
            }
        }
    }
    Ok(selected)
}

fn translate_elements(
    elements: &mut [Element],
    selected: &BTreeSet<usize>,
    delta: &Point,
) -> Result<(), Error> {
    let delta = [
        delta[0]
            .as_f64()
            .map_err(|e| Error::at("/delta/0", e.message))?,
        delta[1]
            .as_f64()
            .map_err(|e| Error::at("/delta/1", e.message))?,
    ];
    for i in selected {
        for (key, delta) in [(element::X, delta[0]), (element::Y, delta[1])] {
            if delta == 0.0 {
                continue;
            }
            let path = format!("/elements/{i}/{}", key.name);
            let Field::Value(value) = elements[*i].get(key)? else {
                return Err(Error::at(path, "missing coordinate"));
            };
            let value = value
                .as_f64()
                .and_then(|v| Number::from_f64(v + delta))
                .map_err(|e| Error::at(path, e.message))?;
            elements[*i].set(key, value)?;
        }
    }
    Ok(())
}
