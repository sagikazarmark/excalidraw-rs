#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub path: String,
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Purpose {
    Inspect,
    Author,
    SelfContained,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValidationReport {
    pub diagnostics: Vec<Diagnostic>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
    pub(crate) fn issue(
        &mut self,
        path: impl Into<String>,
        code: &'static str,
        severity: Severity,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(Diagnostic {
            path: path.into(),
            code,
            severity,
            message: message.into(),
        });
    }
    /// Collapse the report into the single failure [`Error`] can express, for
    /// gating a mutation on it.
    ///
    /// Deliberately lossy: [`Error`] is one path and message, so the first
    /// error wins and warnings are dropped. Callers that want the findings
    /// themselves read [`ValidationReport::diagnostics`] instead.
    pub(crate) fn into_result(self) -> Result<(), Error> {
        match self
            .diagnostics
            .into_iter()
            .find(|d| d.severity == Severity::Error)
        {
            Some(d) => Err(Error::at(d.path, format!("{}: {}", d.code, d.message))),
            None => Ok(()),
        }
    }
    fn errors(&mut self, prefix: &str, errors: Vec<Error>) {
        for e in errors {
            self.issue(
                format!("{prefix}{}", e.path),
                "field-type",
                Severity::Error,
                e.message,
            );
        }
    }
}

/// Whether the container being validated is able to carry binary resources.
///
/// A scene carries a `files` map, so an element referencing an absent resource
/// is a real finding. A library document has no standard `files` field at all,
/// so the same reference is expected and reporting it produces a diagnostic no
/// caller can act on — and, under [`Purpose::SelfContained`], fails every
/// image-bearing library item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Resources {
    /// The container carries resources; a dangling `fileId` is a finding.
    Carried,
    /// The container cannot carry resources; `fileId` references are not checked.
    OutOfScope,
}

impl Document {
    /// Nonmutating scalar, profile, reference and resource inspection. Inspect
    /// permits missing historical fields; Author requires complete declared data.
    pub fn validate(&self, profile: Profile, purpose: Purpose) -> ValidationReport {
        self.validate_scoped(profile, purpose, Resources::Carried)
    }

    pub(crate) fn validate_scoped(
        &self,
        profile: Profile,
        purpose: Purpose,
        resources: Resources,
    ) -> ValidationReport {
        let mut report = ValidationReport::default();
        if purpose != Purpose::Inspect {
            required(
                &mut report,
                &self.root,
                "",
                &["version", "source", "elements", "appState", "files"],
                &[],
            );
        }
        if let Some(version) = self.root.get("version")
            && !matches!(version,Value::Number(n) if crate::Number(n.clone()).as_safe_integer()==Ok(2))
        {
            report.issue(
                "/version",
                "profile",
                Severity::Error,
                "expected scene version 2",
            );
        }
        if let Some(source) = self.root.get("source")
            && !source.is_string()
        {
            report.issue("/source", "field-type", Severity::Error, "expected string");
        }
        let elements = self
            .root
            .get("elements")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut ids: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        let mut groups = BTreeSet::new();
        for (i, element) in elements.iter().enumerate() {
            if let Some(id) = element.get("id").and_then(Value::as_str) {
                ids.entry(id).or_default().push(i);
            }
            if let Some(g) = element.get("groupIds").and_then(Value::as_array) {
                groups.extend(g.iter().filter_map(Value::as_str));
            }
        }
        let files = self.root.get("files").and_then(Value::as_object);
        let cyclic_frames = frame_cycles(elements, &ids);
        let mut previous_index: Option<&str> = None;
        let mut live_labels: BTreeMap<&str, usize> = BTreeMap::new();
        for (i, value) in elements.iter().enumerate() {
            let path = format!("/elements/{i}");
            let Some(object) = value.as_object() else {
                report.issue(&path, "element-shape", Severity::Error, "expected object");
                continue;
            };
            report.errors(&path, element::check(&Element::from_object(object.clone())));
            check_present(
                &mut report,
                object,
                &path,
                element::FIELDS,
                &[
                    "roundness",
                    "index",
                    "frameId",
                    "boundElements",
                    "created",
                    "link",
                    "baseFontSize",
                    "containerId",
                    "labelPosition",
                    "startBinding",
                    "endBinding",
                    "startArrowhead",
                    "endArrowhead",
                    "lastCommittedPoint",
                    "fixedSegments",
                    "startIsSpecial",
                    "endIsSpecial",
                    "fileId",
                    "crop",
                    "name",
                ],
            );
            let kind = object.get("type").and_then(Value::as_str).unwrap_or("");
            semantics::check(&mut report, object, &path, kind, profile, purpose);
            if !ElementKind::KNOWN.contains(&kind) {
                report.issue(
                    format!("{path}/type"),
                    "unknown-kind",
                    if purpose == Purpose::Inspect {
                        Severity::Warning
                    } else {
                        Severity::Error
                    },
                    "unknown kind retained",
                );
            }
            if matches!(kind, "draw" | "selection")
                || (kind == "stickynote" && profile == Profile::V0_18_1)
            {
                report.issue(
                    format!("{path}/type"),
                    "profile",
                    Severity::Error,
                    "kind is not authored in selected profile",
                );
            }
            if let Some(id) = object.get("id").and_then(Value::as_str) {
                if id.is_empty() {
                    report.issue(
                        format!("{path}/id"),
                        "empty-id",
                        Severity::Error,
                        "empty identity",
                    );
                }
                if ids[id].len() > 1 {
                    report.issue(
                        format!("{path}/id"),
                        "duplicate-id",
                        Severity::Error,
                        "ambiguous identity",
                    );
                }
            }
            if purpose != Purpose::Inspect {
                required(
                    &mut report,
                    object,
                    &path,
                    &[
                        "type",
                        "id",
                        "x",
                        "y",
                        "width",
                        "height",
                        "angle",
                        "strokeColor",
                        "backgroundColor",
                        "fillStyle",
                        "strokeWidth",
                        "strokeStyle",
                        "roundness",
                        "roughness",
                        "opacity",
                        "seed",
                        "version",
                        "versionNonce",
                        "index",
                        "isDeleted",
                        "groupIds",
                        "frameId",
                        "boundElements",
                        "updated",
                        "link",
                        "locked",
                    ],
                    &["roundness", "index", "frameId", "boundElements", "link"],
                );
                if profile == Profile::SnapshotAfa3a653 {
                    required(&mut report, object, &path, &["created"], &["created"]);
                }
                let fields: &[&str] = match kind {
                    "text" => &[
                        "fontSize",
                        "fontFamily",
                        "text",
                        "originalText",
                        "textAlign",
                        "verticalAlign",
                        "containerId",
                        "autoResize",
                        "lineHeight",
                    ],
                    "line" | "arrow" => &[
                        "points",
                        "startBinding",
                        "endBinding",
                        "startArrowhead",
                        "endArrowhead",
                    ],
                    "freedraw" => &["points", "pressures", "simulatePressure"],
                    "image" => &["fileId", "status", "scale", "crop"],
                    "frame" | "magicframe" => &["name"],
                    "stickynote" => &["baseHeight"],
                    _ => &[],
                };
                required(
                    &mut report,
                    object,
                    &path,
                    fields,
                    &[
                        "containerId",
                        "startBinding",
                        "endBinding",
                        "startArrowhead",
                        "endArrowhead",
                        "fileId",
                        "crop",
                        "name",
                    ],
                );
                if kind == "arrow" {
                    required(&mut report, object, &path, &["elbowed"], &[]);
                }
                if object.get("elbowed") == Some(&Value::Bool(true)) {
                    required(
                        &mut report,
                        object,
                        &path,
                        &["fixedSegments", "startIsSpecial", "endIsSpecial"],
                        &["fixedSegments", "startIsSpecial", "endIsSpecial"],
                    );
                }
                if profile == Profile::SnapshotAfa3a653 {
                    let fields: &[&str] = match kind {
                        "text" => &["baseFontSize"],
                        "line" => &["polygon"],
                        "freedraw" => &["strokeOptions"],
                        _ => &[],
                    };
                    required(&mut report, object, &path, fields, &["baseFontSize"]);
                } else if matches!(kind, "line" | "arrow" | "freedraw") {
                    required(
                        &mut report,
                        object,
                        &path,
                        &["lastCommittedPoint"],
                        &["lastCommittedPoint"],
                    );
                }
            }
            for (field, allowed) in [
                ("fillStyle", FillStyle::KNOWN),
                ("strokeStyle", StrokeStyle::KNOWN),
                ("textAlign", TextAlign::KNOWN),
                ("verticalAlign", VerticalAlign::KNOWN),
                ("status", ImageStatus::KNOWN),
                ("startArrowhead", Arrowhead::KNOWN),
                ("endArrowhead", Arrowhead::KNOWN),
            ] {
                enumeration(&mut report, object, &path, field, allowed);
            }
            for field in ["startArrowhead", "endArrowhead"] {
                if let Some(head) = object.get(field).and_then(Value::as_str)
                    && profile == Profile::V0_18_1
                    && head.starts_with("cardinality_")
                {
                    report.issue(
                        format!("{path}/{field}"),
                        "profile",
                        Severity::Error,
                        "cardinality head requires snapshot",
                    );
                }
            }
            if profile == Profile::V0_18_1 {
                for field in [
                    "created",
                    "polygon",
                    "baseFontSize",
                    "labelPosition",
                    "strokeOptions",
                    "baseHeight",
                ] {
                    if object.contains_key(field) {
                        report.issue(
                            format!("{path}/{field}"),
                            "profile",
                            if purpose == Purpose::Inspect {
                                Severity::Warning
                            } else {
                                Severity::Error
                            },
                            "field has no released semantics",
                        );
                    }
                }
            }
            for field in ["version", "versionNonce", "seed", "updated", "created"] {
                if let Some(Value::Number(n)) = object.get(field)
                    && crate::Number(n.clone()).as_safe_integer().is_err()
                {
                    report.issue(
                        format!("{path}/{field}"),
                        "integer",
                        Severity::Error,
                        "expected JavaScript safe integer",
                    );
                }
            }
            for (field, min, max) in [
                ("opacity", 0., 100.),
                ("width", 0., f64::MAX),
                ("height", 0., f64::MAX),
                ("strokeWidth", 0., f64::MAX),
                ("roughness", 0., f64::MAX),
                ("labelPosition", 0., 1.),
            ] {
                let Some(stored) = object.get(field) else {
                    continue;
                };
                let Some(n) = stored.as_f64() else {
                    continue;
                };
                // A floor of zero cannot be enforced by comparison alone: a
                // tiny negative magnitude underflows to -0.0, which is not
                // less than 0. Ask the stored decimal for the sign, exactly
                // as the authoring path does.
                let underflowed = min == 0.
                    && matches!(stored, Value::Number(raw)
                        if crate::Number(raw.clone()).is_negative_nonzero());
                if n < min || n > max || underflowed {
                    report.issue(
                        format!("{path}/{field}"),
                        "range",
                        Severity::Error,
                        "outside field range",
                    );
                }
            }
            if let Some(index) = object.get("index").and_then(Value::as_str) {
                if !valid_index(index) {
                    report.issue(
                        format!("{path}/index"),
                        "index-syntax",
                        Severity::Error,
                        "invalid fractional index",
                    );
                }
                if previous_index.is_some_and(|p| p >= index) {
                    report.issue(
                        format!("{path}/index"),
                        "index-order",
                        Severity::Error,
                        "index disagrees with array order",
                    );
                }
                previous_index = Some(index);
            }
            if let Some(g) = object.get("groupIds").and_then(Value::as_array) {
                let mut seen = BTreeSet::new();
                for (j, value) in g.iter().enumerate() {
                    if let Some(id) = value.as_str()
                        && !seen.insert(id)
                    {
                        report.issue(
                            format!("{path}/groupIds/{j}"),
                            "duplicate-group",
                            Severity::Error,
                            "duplicate group in path",
                        );
                    }
                }
            }
            nested(&mut report, object, &path, profile, purpose);
            let deleted = object.get("isDeleted") == Some(&Value::Bool(true));
            for field in ["frameId", "containerId"] {
                // Tombstones retain historical edges without live graph obligations.
                if deleted || !semantics::applies(kind, field, object) {
                    continue;
                }
                if let Some(target) = object.get(field).and_then(Value::as_str) {
                    let allowed: &[&str] = if field == "frameId" {
                        &["frame", "magicframe"]
                    } else if profile == Profile::SnapshotAfa3a653 {
                        &["rectangle", "diamond", "ellipse", "arrow", "stickynote"]
                    } else {
                        &["rectangle", "diamond", "ellipse", "arrow"]
                    };
                    if let Some(target_obj) = resolve(
                        &mut report,
                        &format!("{path}/{field}"),
                        target,
                        &ids,
                        elements,
                        allowed,
                        purpose,
                    ) && field == "containerId"
                    {
                        if let Some(first) = live_labels.get(target) {
                            report.issue(
                                format!("{path}/{field}"),
                                "multiple-labels",
                                compatibility_severity(purpose),
                                // Identify the conflicting sibling by its position
                                // in this element array, not by an absolute
                                // pointer: the same elements are validated inside
                                // a library item, where `/elements/N` names
                                // nothing that exists.
                                format!(
                                    "container {target} already has a live label at element {first}"
                                ),
                            );
                        } else {
                            live_labels.insert(target, i);
                        }
                        if ids[target][0] >= i {
                            report.issue(
                                format!("{path}/{field}"),
                                "bound-text-order",
                                compatibility_severity(purpose),
                                format!(
                                    "live label must follow container {target} at element {}",
                                    ids[target][0]
                                ),
                            );
                        }
                        if !has_reverse(
                            target_obj,
                            object.get("id").and_then(Value::as_str),
                            "text",
                        ) {
                            report.issue(
                                format!("{path}/{field}"),
                                "reciprocal-binding",
                                Severity::Error,
                                "container lacks text back-reference",
                            );
                        }
                    }
                }
            }
            if !deleted && cyclic_frames[i] {
                report.issue(
                    format!("{path}/frameId"),
                    "frame-cycle",
                    Severity::Error,
                    "cyclic frame membership",
                );
            }
            for field in ["startBinding", "endBinding"] {
                if deleted || !semantics::applies(kind, field, object) {
                    continue;
                }
                if let Some(target) = object
                    .get(field)
                    .and_then(|b| b.get("elementId"))
                    .and_then(Value::as_str)
                {
                    let allowed = semantics::bindable_kinds(profile);
                    if let Some(target_obj) = resolve(
                        &mut report,
                        &format!("{path}/{field}/elementId"),
                        target,
                        &ids,
                        elements,
                        allowed,
                        purpose,
                    ) {
                        if semantics::is_bound_text(target_obj) {
                            report.issue(
                                format!("{path}/{field}/elementId"),
                                "non-bindable-target",
                                compatibility_severity(purpose),
                                format!("bound text {target} cannot be an arrow binding target"),
                            );
                        }
                        if !has_reverse(
                            target_obj,
                            object.get("id").and_then(Value::as_str),
                            "arrow",
                        ) {
                            report.issue(
                                format!("{path}/{field}"),
                                "reciprocal-binding",
                                Severity::Error,
                                "target lacks arrow back-reference",
                            );
                        }
                    }
                }
            }
            if !deleted && let Some(bound) = object.get("boundElements").and_then(Value::as_array) {
                let mut seen = BTreeSet::new();
                for (j, b) in bound.iter().enumerate() {
                    if let (Some(id), Some(kind)) = (
                        b.get("id").and_then(Value::as_str),
                        b.get("type").and_then(Value::as_str),
                    ) {
                        if !BindingKind::KNOWN.contains(&kind) {
                            continue;
                        }
                        let p = format!("{path}/boundElements/{j}/id");
                        if !seen.insert(id) {
                            report.issue(
                                &p,
                                "duplicate-binding",
                                Severity::Error,
                                "duplicate reverse binding",
                            );
                        }
                        if let Some(target) =
                            resolve(&mut report, &p, id, &ids, elements, &[kind], purpose)
                        {
                            let own_id = object.get("id").and_then(Value::as_str);
                            let reciprocal = if kind == "text" {
                                target.get("containerId").and_then(Value::as_str) == own_id
                            } else {
                                ["startBinding", "endBinding"].iter().any(|k| {
                                    target
                                        .get(*k)
                                        .and_then(|b| b.get("elementId"))
                                        .and_then(Value::as_str)
                                        == own_id
                                })
                            };
                            if !reciprocal {
                                report.issue(
                                    p,
                                    "reciprocal-binding",
                                    Severity::Error,
                                    "bound element lacks forward reference",
                                );
                            }
                        }
                    }
                }
            }
            if resources == Resources::Carried
                && semantics::applies(kind, "fileId", object)
                && let Some(id) = object.get("fileId").and_then(Value::as_str)
                && files.is_none_or(|f| !f.contains_key(id))
            {
                report.issue(
                    format!("{path}/fileId"),
                    "missing-file",
                    if purpose == Purpose::SelfContained {
                        Severity::Error
                    } else {
                        Severity::Warning
                    },
                    "missing binary resource",
                );
            }
        }
        if let Some(state) = self.root.get("appState").and_then(Value::as_object) {
            check_present(
                &mut report,
                state,
                "/appState",
                app_state::FIELDS,
                if purpose == Purpose::Inspect {
                    &["gridSize"]
                } else {
                    &[]
                },
            );
            report.errors(
                "/appState",
                app_state::check(&AppState::from_object(state.clone())),
            );
            if let Some(locks) = state
                .get("lockedMultiSelections")
                .and_then(Value::as_object)
            {
                if profile == Profile::V0_18_1 {
                    report.issue(
                        "/appState/lockedMultiSelections",
                        "profile",
                        if purpose == Purpose::Inspect {
                            Severity::Warning
                        } else {
                            Severity::Error
                        },
                        "group locks require snapshot",
                    );
                }
                for (id, value) in locks {
                    let path = format!("/appState/lockedMultiSelections{}", pointer(id));
                    if value != &Value::Bool(true) {
                        report.issue(&path, "literal", Severity::Error, "lock value must be true");
                    }
                    if !groups.contains(id.as_str()) {
                        report.issue(
                            path,
                            "missing-group",
                            Severity::Warning,
                            "lock group is not used",
                        );
                    }
                }
            }
        }
        if let Some(files) = files {
            for (id, value) in files {
                let path = format!("/files{}", pointer(id));
                binary_file_record(&mut report, &path, Some(id), value, purpose);
            }
        }
        report
    }
}

/// Validate one binary file record, rooted at `path`.
///
/// Split out so a caller holding a single [`BinaryFile`] can check it directly
/// instead of wrapping it in a throwaway [`Document`] and then stripping the
/// fabricated prefix back off every diagnostic. Both routes run this same code,
/// so a scene's `files` map and a standalone record cannot drift apart.
///
/// `key` is the owning map key when the record is stored in a scene; `None` for
/// a standalone record, where there is no key to disagree with.
pub(crate) fn binary_file_record(
    report: &mut ValidationReport,
    path: &str,
    key: Option<&str>,
    value: &Value,
    purpose: Purpose,
) {
    let Some(o) = value.as_object() else {
        report.issue(
            path.to_owned(),
            "file-shape",
            Severity::Error,
            "expected file object",
        );
        return;
    };
    report.errors(
        path,
        binary_file::check(&BinaryFile::from_object(o.clone())),
    );
    check_present(report, o, path, binary_file::FIELDS, &[]);
    for name in ["created", "lastRetrieved", "version"] {
        if let Some(Value::Number(n)) = o.get(name)
            && crate::Number(n.clone()).as_safe_integer().is_err()
        {
            report.issue(
                format!("{path}/{name}"),
                "integer",
                Severity::Error,
                "expected safe integer metadata",
            );
        }
    }
    required(
        report,
        o,
        path,
        &["id", "mimeType", "dataURL", "created"],
        &[],
    );
    // `required` only asks whether the field is present and non-null, so an
    // empty string passes it. The element path spells this out separately for
    // the same reason; a resource whose identity is empty cannot be addressed
    // by the image element that references it. `BinaryFile::new` already
    // refuses it, and this is what makes `validate` answer the same way.
    if o.get("id").and_then(Value::as_str) == Some("") {
        report.issue(
            format!("{path}/id"),
            "empty-id",
            Severity::Error,
            "empty identity",
        );
    }
    if let Some(key) = key
        && o.get("id").and_then(Value::as_str) != Some(key)
    {
        report.issue(
            format!("{path}/id"),
            "file-id",
            Severity::Error,
            "file key and record ID differ",
        );
    }
    let mime = o.get("mimeType").and_then(Value::as_str);
    if let Some(declared) = mime
        && !MimeType::KNOWN.contains(&declared)
    {
        report.issue(
            format!("{path}/mimeType"),
            "mime-type",
            // MimeType is an open enum so a preserving document can carry a
            // format this crate does not recognise yet; condemning that would
            // break the preserving promise. Only a self-contained document,
            // which promises its embedded resources are usable, rules it out.
            if purpose == Purpose::SelfContained {
                Severity::Error
            } else {
                Severity::Warning
            },
            "unsupported MIME type",
        );
    }
    // A data URL contradicting the record's own declared type is
    // self-contradictory under every purpose, like unusable metadata above.
    if let Some(url) = o.get("dataURL").and_then(Value::as_str)
        && !mime.is_some_and(|m| data_url_carries(url, m))
    {
        report.issue(
            format!("{path}/dataURL"),
            "data-url",
            Severity::Error,
            "expected nonempty data URL matching MIME type",
        );
    }
}

/// Does `url` parse as a data URL that actually carries a payload of `mime`?
///
/// This is a fact about a binary file record, not about any one envelope, so
/// every route that judges a record — [`binary_file_record`] here, the Plus
/// transport in [`crate::plus`] — asks this one function rather than spelling
/// the parse out again and drifting from it.
///
/// The media-type token is compared ASCII case-insensitively (RFC 2045 §5.1:
/// type and subtype names are case-insensitive), so `IMAGE/PNG` and
/// `image/png` agree. Membership of [`MimeType::KNOWN`] is a separate question
/// and deliberately stays case-sensitive where it is asked: an
/// uppercase-but-otherwise-recognised token is reported as an unrecognised
/// MIME type even though it agrees with its own data URL — two different
/// findings about two different things, not a contradiction.
pub(crate) fn data_url_carries(url: &str, mime: &str) -> bool {
    url.strip_prefix("data:")
        .and_then(|s| s.split_once(','))
        .is_some_and(|(header, payload)| {
            !payload.is_empty()
                && header
                    .split(';')
                    .next()
                    .is_some_and(|token| token.eq_ignore_ascii_case(mime))
        })
}

pub(crate) fn required(
    report: &mut ValidationReport,
    o: &Object,
    path: &str,
    fields: &[&str],
    nullable: &[&str],
) {
    for field in fields {
        if !o.contains_key(*field) || (o[*field].is_null() && !nullable.contains(field)) {
            report.issue(
                format!("{path}{}", pointer(field)),
                "required",
                Severity::Error,
                "missing or null required field",
            );
        }
    }
}

// Check only known field subtrees: arbitrary metadata may legitimately archive
// numbers which the editor cannot interpret as geometry.
pub(crate) fn check_present(
    report: &mut ValidationReport,
    object: &Object,
    path: &str,
    fields: &[&str],
    nullable: &[&str],
) {
    fn finite(report: &mut ValidationReport, value: &Value, path: &str) {
        match value {
            Value::Number(n) if n.as_f64().is_none_or(|v| !v.is_finite()) => report.issue(
                path,
                "numeric-range",
                Severity::Error,
                "outside finite editor range",
            ),
            Value::Array(a) => {
                for (i, v) in a.iter().enumerate() {
                    finite(report, v, &format!("{path}/{i}"));
                }
            }
            _ => {}
        }
    }
    for key in fields {
        if let Some(value) = object.get(*key) {
            let p = format!("{path}{}", pointer(key));
            if value.is_null() && !nullable.contains(key) {
                report.issue(&p, "null-field", Severity::Error, "field is not nullable");
            }
            finite(report, value, &p);
        }
    }
}
fn enumeration(report: &mut ValidationReport, o: &Object, path: &str, key: &str, values: &[&str]) {
    if let Some(value) = o.get(key).and_then(Value::as_str)
        && !values.contains(&value)
    {
        report.issue(
            format!("{path}/{key}"),
            "enum",
            Severity::Error,
            "unknown enum value retained",
        );
    }
}
fn nested(
    report: &mut ValidationReport,
    o: &Object,
    path: &str,
    profile: Profile,
    purpose: Purpose,
) {
    macro_rules! check {
        ($value:expr,$path:expr,$ty:ty,$module:ident,$fields:expr) => {
            if let Some(object) = $value.and_then(Value::as_object) {
                report.errors(&$path, $module::check(&<$ty>::from_object(object.clone())));
                check_present(report, object, &$path, $module::FIELDS, &[]);
                if purpose != Purpose::Inspect {
                    required(report, object, &$path, $fields, &[]);
                }
            }
        };
    }
    for key in ["startBinding", "endBinding"] {
        let p = format!("{path}/{key}");
        check!(o.get(key), p, Binding, binding, &[]);
        if let Some(b) = o.get(key).and_then(Value::as_object) {
            enumeration(report, b, &p, "mode", BindMode::KNOWN);
            if profile == Profile::V0_18_1 && b.contains_key("mode") {
                report.issue(
                    format!("{p}/mode"),
                    "profile",
                    Severity::Error,
                    "release uses focus/gap bindings",
                );
            }
            if profile == Profile::SnapshotAfa3a653
                && (b.contains_key("focus") || b.contains_key("gap"))
            {
                report.issue(
                    &p,
                    "profile",
                    Severity::Error,
                    "legacy binding needs explicit migration",
                );
            }
            if purpose != Purpose::Inspect {
                let fields = if profile == Profile::V0_18_1 {
                    &["elementId", "focus", "gap"][..]
                } else {
                    &["elementId", "fixedPoint", "mode"][..]
                };
                required(report, b, &p, fields, &[]);
                if o.get("elbowed") == Some(&Value::Bool(true)) {
                    required(report, b, &p, &["fixedPoint"], &[]);
                }
            }
        }
    }
    check!(
        o.get("crop"),
        format!("{path}/crop"),
        ImageCrop,
        image_crop,
        image_crop::FIELDS
    );
    check!(
        o.get("roundness"),
        format!("{path}/roundness"),
        Roundness,
        roundness,
        &["type"]
    );
    check!(
        o.get("strokeOptions"),
        format!("{path}/strokeOptions"),
        StrokeOptions,
        stroke_options,
        stroke_options::FIELDS
    );
    if let Some(options) = o.get("strokeOptions").and_then(Value::as_object) {
        enumeration(
            report,
            options,
            &format!("{path}/strokeOptions"),
            "variability",
            Variability::KNOWN,
        );
    }
    if let Some(bound) = o.get("boundElements").and_then(Value::as_array) {
        for (i, b) in bound.iter().enumerate() {
            check!(
                Some(b),
                format!("{path}/boundElements/{i}"),
                BoundElement,
                bound_element,
                bound_element::FIELDS
            );
            if let Some(b) = b.as_object() {
                enumeration(
                    report,
                    b,
                    &format!("{path}/boundElements/{i}"),
                    "type",
                    BindingKind::KNOWN,
                );
            }
        }
    }
    if let Some(segments) = o.get("fixedSegments").and_then(Value::as_array) {
        for (i, s) in segments.iter().enumerate() {
            check!(
                Some(s),
                format!("{path}/fixedSegments/{i}"),
                FixedSegment,
                fixed_segment,
                fixed_segment::FIELDS
            );
        }
    }
    if o.get("type").and_then(Value::as_str) == Some("iframe")
        && let Some(data) = o.get("customData").and_then(|d| d.get("generationData"))
    {
        let p = format!("{path}/customData/generationData");
        check!(Some(data), p, GenerationData, generation_data, &["status"]);
        if !data.is_object() {
            report.issue(
                &p,
                "field-type",
                Severity::Error,
                "expected generation object",
            );
        }
        if let Some(data) = data.as_object() {
            enumeration(report, data, &p, "status", GenerationStatus::KNOWN);
            let fields: &[&str] = match data.get("status").and_then(Value::as_str) {
                Some("done") => &["html"],
                Some("error") => &["code"],
                _ => &[],
            };
            required(report, data, &p, fields, &[]);
        }
    }
}
fn resolve<'a>(
    report: &mut ValidationReport,
    path: &str,
    id: &str,
    ids: &BTreeMap<&str, Vec<usize>>,
    elements: &'a [Value],
    allowed: &[&str],
    purpose: Purpose,
) -> Option<&'a Object> {
    let Some(indices) = ids.get(id) else {
        report.issue(
            path,
            "missing-reference",
            Severity::Error,
            format!("missing target {id}"),
        );
        return None;
    };
    if indices.len() != 1 {
        report.issue(
            path,
            "ambiguous-reference",
            Severity::Error,
            format!("duplicate target {id}"),
        );
        return None;
    }
    let target = elements[indices[0]].as_object()?;
    if !target
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|k| allowed.contains(&k))
    {
        report.issue(
            path,
            "reference-kind",
            Severity::Error,
            format!("wrong target kind for {id}"),
        );
    }
    if target.get("isDeleted") == Some(&Value::Bool(true)) {
        report.issue(
            path,
            "deleted-reference",
            compatibility_severity(purpose),
            format!("target {id} is deleted"),
        );
        // A deleted target has no live reciprocal or ordering obligations.
        return None;
    }
    Some(target)
}
fn has_reverse(o: &Object, id: Option<&str>, kind: &str) -> bool {
    id.is_some()
        && o.get("boundElements")
            .and_then(Value::as_array)
            .is_some_and(|a| {
                a.iter().any(|b| {
                    b.get("id").and_then(Value::as_str) == id
                        && b.get("type").and_then(Value::as_str) == Some(kind)
                })
            })
}
fn valid_index(index: &str) -> bool {
    let Some(first) = index.bytes().next() else {
        return false;
    };
    let integer_len = match first {
        b'a'..=b'z' => usize::from(first - b'a') + 2,
        b'A'..=b'Z' => usize::from(b'Z' - first) + 2,
        _ => return false,
    };
    index.len() >= integer_len
        && index.bytes().skip(1).all(|b| b.is_ascii_alphanumeric())
        && !(index.len() > integer_len && index.ends_with('0'))
        && index != "A00000000000000000000000000"
}

// Each node has at most one frame parent. Resolve edges once and memoize whether
// each ancestry path reaches a cycle. Ambiguous/missing IDs terminate traversal,
// preserving the separate reference diagnostics. No recursion on deep scenes.
fn frame_cycles(elements: &[Value], ids: &BTreeMap<&str, Vec<usize>>) -> Vec<bool> {
    let parents: Vec<_> = elements
        .iter()
        .map(|element| {
            if element.get("isDeleted") == Some(&Value::Bool(true)) {
                return None;
            }
            element
                .get("frameId")
                .and_then(Value::as_str)
                .and_then(|id| ids.get(id))
                .filter(|indices| indices.len() == 1)
                .map(|indices| indices[0])
        })
        .collect();
    let mut complete = vec![false; elements.len()];
    let mut cyclic = vec![false; elements.len()];
    let mut visiting = vec![false; elements.len()];
    let mut path = Vec::new();
    for start in 0..elements.len() {
        if complete[start] {
            continue;
        }
        let mut current = Some(start);
        let reaches_cycle = loop {
            let Some(node) = current else {
                break false;
            };
            if complete[node] {
                break cyclic[node];
            }
            if visiting[node] {
                break true;
            }
            visiting[node] = true;
            path.push(node);
            current = parents[node];
        };
        for node in path.drain(..) {
            visiting[node] = false;
            complete[node] = true;
            cyclic[node] = reaches_cycle;
        }
    }
    cyclic
}
mod semantics;
pub(crate) use semantics::applies;
pub(crate) use semantics::compatibility_severity;

use crate::{Document, Error, Object, Profile, model::*, wire::pointer};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
