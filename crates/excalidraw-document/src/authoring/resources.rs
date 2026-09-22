use super::{SceneAuthor, reindex, require_kind};
use crate::{
    BinaryFile, Document, Element, ElementKind, Error, Field, FileId, IdMap, ImageStatus,
    LibraryItem, MimeType, Number, OpaquePolicy, Point, Purpose, ValidationReport, binary_file,
    element, library_item, wire::pointer,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

impl BinaryFile {
    /// Construct a resource with explicit identity and timestamp. Checks the
    /// declared MIME type and a nonempty matching data URL, without decoding it.
    pub fn new(
        id: FileId,
        mime_type: MimeType,
        data_url: String,
        created: Number,
    ) -> Result<Self, Error> {
        let mut file = Self::default();
        file.set(binary_file::ID, id)?;
        file.set(binary_file::MIME_TYPE, mime_type)?;
        file.set(binary_file::DATA_URL, data_url)?;
        file.set(binary_file::CREATED, created)?;
        resource_id(&file)?;
        Ok(file)
    }
}

impl SceneAuthor<'_> {
    /// Register under the record's ID. An identical complete record deduplicates;
    /// a different record at the same identity is rejected, including metadata.
    pub fn register_file(&mut self, file: BinaryFile) -> Result<(), Error> {
        self.transaction_document(|document| merge_resource(document, file))
    }

    /// Append an image and its resource atomically, assigning `fileId` from the
    /// resource record and setting status to Saved. Revisions remain caller-owned.
    pub fn insert_image(&mut self, mut image: Element, file: BinaryFile) -> Result<(), Error> {
        require_kind(&image, ElementKind::Image)?;
        let id = resource_id(&file)?;
        image.set(element::FILE_ID, id)?;
        image.set(element::STATUS, ImageStatus::Saved)?;
        self.transaction_document(|document| {
            merge_resource(document, file)?;
            let mut elements = document.elements()?;
            elements.push(image);
            reindex(&mut elements)?;
            document.set_elements(elements);
            Ok(())
        })
    }

    /// Insert an internally closed library graph with complete, fresh element
    /// and group mappings. Destinations must be disjoint from both source and
    /// scene identities (including tombstones and locked groups).
    ///
    /// `files` contains exactly the image resources keyed by their source IDs;
    /// each key must equal its record ID. Unmapped file IDs are shared, and
    /// `mapping.files` explicitly chooses replacement IDs. Existing destination
    /// assets must equal the remapped records in full; assets are never overwritten.
    /// Every image must have an explicit resource, even if the scene has it already.
    ///
    /// Translates every element's x/y, including frames, and marks images Saved.
    /// Element/resource extensions are preserved under the opaque policy. Item
    /// envelope metadata stays on the borrowed item; its opaque paths are reported
    /// at `/libraryItems/0/...`. Other unassessed paths are source-scene-relative,
    /// as returned by `remap_ids`. The entire scene update is atomic.
    pub fn insert_library_item(
        &mut self,
        item: &LibraryItem,
        mapping: &IdMap,
        files: BTreeMap<FileId, BinaryFile>,
        offset: Point,
        policy: OpaquePolicy,
    ) -> Result<Vec<String>, Error> {
        item.check_authored(self.profile)?;
        let Field::Value(elements) = item.get(library_item::ELEMENTS)? else {
            return Err(Error::at("/libraryItems/0/elements", "expected elements"));
        };
        let mut unassessed_paths = Vec::new();
        for key in item.as_object().keys() {
            if !library_item::FIELDS.contains(&key.as_str()) {
                let path = format!("/libraryItems/0{}", pointer(key));
                if policy == OpaquePolicy::Reject {
                    return Err(Error::at(path, "opaque item metadata requires Preserve"));
                }
                unassessed_paths.push(path);
            }
        }
        let mut source_ids = BTreeSet::new();
        let mut source_groups = BTreeSet::new();
        let mut needed_files = BTreeSet::new();
        for (i, e) in elements.iter().enumerate() {
            if let Field::Value(id) = e.get(element::ID)? {
                source_ids.insert(id);
            }
            if let Field::Value(groups) = e.get(element::GROUP_IDS)? {
                source_groups.extend(groups);
            }
            if e.kind()? == Field::Value(ElementKind::Image) {
                let Field::Value(id) = e.get(element::FILE_ID)? else {
                    return Err(Error::at(
                        format!("/libraryItems/0/elements/{i}/fileId"),
                        "library image requires an explicit resource identity",
                    ));
                };
                needed_files.insert(id);
            }
        }
        if files.keys().cloned().collect::<BTreeSet<_>>() != needed_files {
            return Err(Error::at(
                "/files",
                "supply exactly the library image resources",
            ));
        }
        for (id, file) in &files {
            if resource_id(file)? != *id {
                return Err(Error::at(
                    format!("/files{}/id", pointer(&id.0)),
                    "file key and record ID differ",
                ));
            }
        }
        let mut occupied_ids = BTreeSet::new();
        let mut occupied_groups = BTreeSet::new();
        for e in self.document.elements()? {
            if let Field::Value(id) = e.get(element::ID)? {
                occupied_ids.insert(id);
            }
            if let Field::Value(groups) = e.get(element::GROUP_IDS)? {
                occupied_groups.extend(groups);
            }
        }
        if let Field::Value(locks) = self
            .document
            .app_state()?
            .get(crate::app_state::LOCKED_MULTI_SELECTIONS)?
        {
            occupied_groups.extend(locks.into_keys());
        }
        fresh_resource_copy_ids(&source_ids, &occupied_ids, &mapping.elements, "/elements")?;
        fresh_resource_copy_ids(
            &source_groups,
            &occupied_groups,
            &mapping.groups,
            "/groupIds",
        )?;

        let mut source = Document::new("library-insertion");
        source.set_elements(elements);
        source.set_files(files);
        // remap_ids also rejects dangling references independently of the scene,
        // preventing a source reference from accidentally binding to another copy.
        let remapped = source.remap_ids(mapping, policy)?;
        unassessed_paths.extend(remapped.unassessed_paths);
        let mut additions = remapped.document.elements()?;
        let dx = offset[0]
            .as_f64()
            .map_err(|e| Error::at("/offset/0", e.message))?;
        let dy = offset[1]
            .as_f64()
            .map_err(|e| Error::at("/offset/1", e.message))?;
        for (i, e) in additions.iter_mut().enumerate() {
            for (key, delta) in [(element::X, dx), (element::Y, dy)] {
                let path = format!("/elements/{i}/{}", key.name);
                let Field::Value(value) = e.get(key)? else {
                    return Err(Error::at(path, "expected coordinate"));
                };
                // Keep exact wire numbers when the caller requests no translation.
                if delta != 0.0 {
                    let value = value.as_f64().map_err(|e| Error::at(&path, e.message))?;
                    e.set(
                        key,
                        Number::from_f64(value + delta).map_err(|e| Error::at(&path, e.message))?,
                    )?;
                }
            }
            if e.kind()? == Field::Value(ElementKind::Image) {
                e.set(element::STATUS, ImageStatus::Saved)?;
            }
        }
        self.transaction_document(|document| {
            for file in remapped.document.files()?.into_values() {
                merge_resource(document, file)?;
            }
            let mut elements = document.elements()?;
            elements.extend(additions);
            reindex(&mut elements)?;
            document.set_elements(elements);
            Ok(())
        })?;
        Ok(unassessed_paths)
    }
}

fn fresh_resource_copy_ids<T: Ord + Clone>(
    source: &BTreeSet<T>,
    occupied: &BTreeSet<T>,
    mapping: &BTreeMap<T, T>,
    path: &str,
) -> Result<(), Error> {
    if mapping.keys().cloned().collect::<BTreeSet<_>>() != *source {
        return Err(Error::at(
            path,
            "supply a complete identity mapping with no extra sources",
        ));
    }
    if mapping
        .values()
        .any(|id| source.contains(id) || occupied.contains(id))
    {
        return Err(Error::at(
            path,
            "copy identity must be fresh in both source and scene",
        ));
    }
    Ok(())
}

fn resource_id(file: &BinaryFile) -> Result<FileId, Error> {
    let Field::Value(id) = file.get(binary_file::ID)? else {
        return Err(Error::at("/id", "expected resource identity"));
    };
    if id.0.is_empty() {
        return Err(Error::at("/id", "empty resource identity"));
    }
    // The same record checks the scene `files` map runs, rooted at the record
    // itself. Validating through a fabricated document would have meant
    // stripping the invented prefix back off every diagnostic, and pinned the
    // profile to one value regardless of the caller's.
    let mut report = ValidationReport::default();
    let record = Value::Object(file.as_object().clone());
    // SelfContained, not Author: a resource being newly authored must be usable
    // on its own, which is exactly what that purpose promises. Author would
    // downgrade an unrecognised MIME type to a warning, which is right for a
    // preserving document that already holds one and wrong for a record this
    // call is creating.
    crate::validation::binary_file_record(&mut report, "", None, &record, Purpose::SelfContained);
    report.into_result()?;
    Ok(id)
}

fn merge_resource(document: &mut Document, file: BinaryFile) -> Result<(), Error> {
    let id = resource_id(&file)?;
    let mut files = document.files()?;
    if let Some(existing) = files.get(&id) {
        if existing != &file {
            return Err(Error::at(
                format!("/files{}", pointer(&id.0)),
                "resource identity already contains a different record",
            ));
        }
    } else {
        files.insert(id, file);
        document.set_files(files);
    }
    Ok(())
}
