use crate::{DrawingOptions, Error, FillStyle, provenance::CustomData};
use excalidraw_document::{
    Authoring, Document, Element, ElementId, ElementKind, Field, FrameOrder, GroupId, IdMap,
    KnownFont, LibraryDocument, LibraryItem, Number, OpaquePolicy, Profile, TextContent, element,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};
use std::{fs::OpenOptions, io::Write, path::Path};

/// Existing output is protected unless the caller explicitly opts into replacement.
#[derive(Clone, Copy, Debug)]
pub enum Overwrite {
    Refuse,
    Allow,
}

/// Axis-aligned geometric layout bounds in scene units. Includes straight stroke
/// extents, straight arrowhead vertices, rotated text boxes and path vertices; excludes random sketch excursions
/// and native frame-name chrome. This is not an exact raster/ink bounding box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    fn expanded(self, amount: f64) -> Result<Self, Error> {
        Self::from_edges(
            self.x - amount,
            self.y - amount,
            self.x + self.width + amount,
            self.y + self.height + amount,
        )
    }
    fn from_edges(left: f64, top: f64, right: f64, bottom: f64) -> Result<Self, Error> {
        let bounds = Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        };
        if ![left, top, right, bottom, bounds.width, bounds.height]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(Error::Invalid("scene coordinates overflow"));
        }
        Ok(bounds)
    }

    fn union(self, other: Self) -> Result<Self, Error> {
        Self::from_edges(
            self.x.min(other.x),
            self.y.min(other.y),
            (self.x + self.width).max(other.x + other.width),
            (self.y + self.height).max(other.y + other.height),
        )
    }
}

/// Native line pattern, independent of chart paints and sketch styling.
#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StrokeStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

/// Supported native end heads for straight, unbound arrows.
#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArrowHead {
    #[default]
    Arrow,
    Triangle,
}

/// Arrow paint: width 1..=20 and opacity 0.005..=1 (native integer percent).
/// The current drawing scope supplies sketch and stroke patterns, groups and source.
#[derive(Clone, Copy, Debug)]
pub struct ArrowStyle {
    pub color: (u8, u8, u8),
    pub width: u32,
    pub opacity: f64,
    pub head: ArrowHead,
}

impl Default for ArrowStyle {
    fn default() -> Self {
        Self {
            color: (30, 30, 30),
            width: 2,
            opacity: 1.,
            head: ArrowHead::Arrow,
        }
    }
}

/// Transparent decorative rectangle styling. RGB channels are bounded by type;
/// width is a positive whole number of scene units (zero is rejected).
#[derive(Clone, Copy, Debug)]
pub struct BorderStyle {
    pub color: (u8, u8, u8),
    pub width: u32,
    pub stroke: StrokeStyle,
}

impl Default for BorderStyle {
    fn default() -> Self {
        Self {
            color: (30, 30, 30),
            width: 1,
            stroke: StrokeStyle::Solid,
        }
    }
}

pub(crate) struct Paint {
    pub(crate) color: String,
    pub(crate) opacity: u8,
}

/// Observable dispatch and output counts, including invisible/unsupported calls.
#[derive(Debug)]
pub struct Diagnostics {
    pub calls: BTreeMap<&'static str, usize>,
    pub elements: BTreeMap<&'static str, usize>,
    pub vertices: usize,
}

/// Owned native scene. Borrow it for drawing, then serialize after drawing succeeds.
pub struct Scene {
    namespace: String,
    updated: u64,
    elements: Vec<Element>,
    calls: RefCell<BTreeMap<&'static str, usize>>,
    failed: Cell<bool>,
    options: DrawingOptions,
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene {
    /// Fresh identity namespace for each real export instance.
    pub fn new() -> Self {
        let mut scene = Self::with_namespace(uuid::Uuid::new_v4().to_string());
        scene.updated = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        scene
    }

    /// Controlled identities and timestamps for reproducible fixtures only.
    /// Reusing a namespace is not safe for independently inserted exports.
    pub fn with_namespace(namespace: impl Into<String>) -> Self {
        let namespace = namespace.into();
        Self {
            options: DrawingOptions::new(namespace.clone()),
            namespace,
            updated: 0,
            elements: Vec::new(),
            calls: RefCell::new(BTreeMap::new()),
            failed: Cell::new(false),
        }
    }

    /// Scene-local style/group scopes, shared by all drawing areas of this scene.
    pub fn drawing_options(&self) -> DrawingOptions {
        self.options.clone()
    }

    fn ensure_valid(&self) -> Result<(), Error> {
        if self.failed.get() {
            Err(Error::FailedScene)
        } else {
            Ok(())
        }
    }

    /// Add one editable, unrotated, left-aligned native Excalifont note. `position`
    /// is the line box's top-left in scene units, and `font_size` is in scene units.
    /// Explicit newlines are supported; chart/backend labels remain single-line.
    /// CRLF becomes LF; leading, middle and trailing blank lines each occupy one
    /// line box (empty lines measure as a space). Empty note strings are rejected
    /// because native import deletes them. Width is
    /// the maximum line advance; height is the sum of 1.25-em line boxes. There is
    /// no container binding or automatic wrapping. The editor may delete wholly
    /// blank text when committing an edit.
    ///
    /// Uses opaque dark text and the current drawing style/group/source scopes.
    /// Only printable ASCII, `é`, `−`, `°`, `±` and explicit newlines are supported. Tabs,
    /// bare CR and other unsupported controls/glyphs are errors. Font size must be
    /// finite and positive; width and total height must fit u32 scene units.
    /// Nonfinite placement and bounds overflow are rejected before mutation;
    /// errors leave the scene unchanged and do not clear prior drawing failures.
    pub fn add_note(
        &mut self,
        text: &str,
        position: (f64, f64),
        font_size: f64,
    ) -> Result<(), Error> {
        let previous = self.bounds()?;
        if !position.0.is_finite() || !position.1.is_finite() {
            return Err(Error::Invalid("position must be finite"));
        }
        let text = text.replace("\r\n", "\n");
        let size = crate::typography::measure_note(&text, font_size)?;
        let bounds = Bounds::from_edges(
            position.0,
            position.1,
            position.0 + size.0,
            position.1 + size.1,
        )?;
        if let Some(previous) = previous {
            previous.union(bounds)?;
        }
        self.push(
            position,
            size,
            Paint {
                color: "#1e1e1e".into(),
                opacity: 100,
            },
            1,
            Kind::Text { text, font_size },
        )?;
        Ok(())
    }

    /// Add a borderless filled polygon with fractional scene coordinates.
    /// Consecutive duplicate vertices are removed and the path is closed automatically.
    /// At least three noncollinear vertices in the stored local geometry are required. Coordinates,
    /// relative geometry and combined scene bounds must be finite. Opacity must be
    /// 0.005..=1 and is rounded to native integer percent. The current sketch, fill,
    /// group and source scopes apply. Invalid input leaves the scene unchanged.
    /// Self-intersections are not checked; supply a simple polygon for predictable fill.
    pub fn add_polygon(
        &mut self,
        points: &[(f64, f64)],
        color: (u8, u8, u8),
        opacity: f64,
    ) -> Result<(), Error> {
        let previous = self.bounds()?;
        if !opacity.is_finite() || !(0.005..=1.).contains(&opacity) {
            return Err(Error::Invalid("polygon opacity must be 0.005..=1.0"));
        }
        if points
            .iter()
            .any(|&(x, y)| !x.is_finite() || !y.is_finite())
        {
            return Err(Error::Invalid("polygon coordinates must be finite"));
        }
        let mut points = points.to_vec();
        points.dedup();
        if points.first() == points.last() {
            points.pop();
        }
        if points.len() < 3 {
            return Err(Error::Invalid("polygon needs at least three vertices"));
        }
        let origin = points[0];
        let local: Vec<_> = points
            .iter()
            .map(|&(x, y)| [x - origin.0, y - origin.1])
            .collect();
        if local.iter().flatten().any(|v| !v.is_finite()) {
            return Err(Error::Invalid("polygon relative coordinates overflow"));
        }
        let (left, top, right, bottom) = path_edges(&local);
        let bounds = Bounds::from_edges(
            origin.0 + left,
            origin.1 + top,
            origin.0 + right,
            origin.1 + bottom,
        )?;
        if let Some(previous) = previous {
            previous.union(bounds)?;
        }
        // Compare products exactly in the stored local geometry. Multiplying
        // binary significands fits u128 and avoids both cancellation and scaling
        // away small vertices in paths with extreme coordinate ratios.
        if !local[2..]
            .iter()
            .any(|p| exact_product(local[1][0], p[1]) != exact_product(local[1][1], p[0]))
        {
            return Err(Error::Invalid("polygon vertices must not be collinear"));
        }
        points.push(origin);
        self.path(
            points,
            Paint {
                color: format!("#{:02x}{:02x}{:02x}", color.0, color.1, color.2),
                opacity: (opacity * 100.).round() as u8,
            },
            1,
            true,
        )?;
        self.call("fill_polygon_fractional");
        Ok(())
    }

    /// Complete content bounds; an empty scene has no bounds.
    pub fn bounds(&self) -> Result<Option<Bounds>, Error> {
        self.bounds_offset((0., 0.))
    }

    /// Emit one genuine native arrow from `start` to `end` in scene units, with
    /// an end head and no bindings. Exactly two distinct finite endpoints are
    /// required; length must be at least one scene unit. There is no canvas limit
    /// for scene operations. Geometry/style/overflow errors leave the scene intact.
    /// Bounds include the straight head and stroke, excluding random sketch ink.
    pub fn add_arrow(
        &mut self,
        start: (f64, f64),
        end: (f64, f64),
        style: ArrowStyle,
    ) -> Result<(), Error> {
        let previous = self.bounds()?;
        if !(1..=20).contains(&style.width) {
            return Err(Error::Invalid("arrow width must be 1..=20 scene units"));
        }
        if !style.opacity.is_finite() || !(0.005..=1.).contains(&style.opacity) {
            return Err(Error::Invalid("arrow opacity must be 0.005..=1.0"));
        }
        if ![start.0, start.1, end.0, end.1]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(Error::Invalid("arrow endpoints must be finite"));
        }
        let delta = [end.0 - start.0, end.1 - start.1];
        let (left, top, right, bottom) = arrow_edges(delta, style.head)?;
        let bounds = Bounds::from_edges(
            start.0 + left,
            start.1 + top,
            start.0 + right,
            start.1 + bottom,
        )?
        .expanded(f64::from(style.width) / 2.)?;
        if let Some(previous) = previous {
            previous.union(bounds)?;
        }
        let (r, g, b) = style.color;
        self.push(
            start,
            (delta[0].abs(), delta[1].abs()),
            Paint {
                color: format!("#{r:02x}{g:02x}{b:02x}"),
                opacity: (style.opacity * 100.).round() as u8,
            },
            style.width,
            Kind::Arrow {
                points: [[0., 0.], delta],
                head: style.head,
            },
        )?;
        Ok(())
    }

    fn bounds_offset(&self, offset: (f64, f64)) -> Result<Option<Bounds>, Error> {
        self.ensure_valid()?;
        if !offset.0.is_finite() || !offset.1.is_finite() {
            return Err(Error::Invalid("position must be finite"));
        }
        let mut bounds: Option<Bounds> = None;
        for element in &self.elements {
            let next = element_bounds_offset(element, offset)?;
            bounds = Some(match bounds {
                Some(previous) => previous.union(next)?,
                None => next,
            });
        }
        Ok(bounds)
    }

    /// Move every origin by an offset, preserving relative path points. Validation
    /// precedes mutation; errors leave the scene unchanged and do not poison it.
    pub fn translate(&mut self, offset: (f64, f64)) -> Result<(), Error> {
        self.bounds_offset(offset)?;
        let mut candidate = self.to_document()?;
        candidate.author(Profile::V0_18_1)?.translate_connected(
            &element_ids(&self.elements),
            [Number::from_f64(offset.0)?, Number::from_f64(offset.1)?],
        )?;
        self.elements = candidate.elements()?;
        Ok(())
    }

    /// Place the complete bounds' top-left at `position`, without scaling.
    /// Unlike translation, placement of empty content is an error.
    pub fn place_at(&mut self, position: (f64, f64)) -> Result<(), Error> {
        let bounds = self
            .bounds()?
            .ok_or(Error::Invalid("empty scene has no bounds"))?;
        self.translate((position.0 - bounds.x, position.1 - bounds.y))
    }

    /// Consume generated content, appending it in source painter order. All source
    /// identities are reallocated locally, preserving relationships while isolating
    /// escaped source drawing handles. Destination document settings stay authoritative.
    /// Validation errors leave the destination unchanged (the input is consumed).
    pub fn append(&mut self, source: Scene) -> Result<(), Error> {
        let destination_bounds = self.bounds()?;
        let source_bounds = source.bounds()?;
        if let (Some(a), Some(b)) = (destination_bounds, source_bounds) {
            a.union(b)?;
        }
        let mut mapping = IdMap {
            elements: source
                .elements
                .iter()
                .enumerate()
                .map(|(ordinal, element)| {
                    (
                        ElementId(string(element, "id").to_owned()),
                        ElementId(format!(
                            "{}-{}",
                            self.namespace,
                            self.elements.len() + ordinal
                        )),
                    )
                })
                .collect(),
            ..IdMap::default()
        };
        for element in &source.elements {
            for group in element_groups(element)? {
                mapping
                    .groups
                    .entry(group)
                    .or_insert_with(|| GroupId(self.options.new_group_id()));
            }
        }
        // Links and customData are host-owned provenance, not native references.
        // The bounded generated schema needs no interpretation of unassessed paths.
        let remapped = source
            .to_document()?
            .remap_ids(&mapping, OpaquePolicy::Preserve)?;
        let mut candidate = self.elements.clone();
        candidate.extend(remapped.document.elements()?);
        // Painter order is authoritative. Coarse authoring may assign indices;
        // clear them rather than retaining colliding indices from separate scenes.
        clear_indices(&mut candidate);
        for (call, count) in source.calls.into_inner() {
            *self.calls.get_mut().entry(call).or_default() += count;
        }
        self.elements = candidate;
        Ok(())
    }

    fn wrapping_bounds(&self, padding: f64) -> Result<Bounds, Error> {
        if !padding.is_finite() || padding < 0. {
            return Err(Error::Invalid("padding must be finite and nonnegative"));
        }
        self.bounds()?
            .ok_or(Error::Invalid("empty scene has no bounds"))?
            .expanded(padding)
    }

    /// Add a transparent border above the content and an outer selection group,
    /// retaining all inner groups. Padding is generation-time clearance to the
    /// inner straight stroke edge, not a live editor constraint or rough ink promise.
    pub fn add_border(&mut self, padding: f64, style: BorderStyle) -> Result<(), Error> {
        if style.width == 0 {
            return Err(Error::Invalid("border width must be positive"));
        }
        let bounds = self
            .wrapping_bounds(padding)?
            .expanded(f64::from(style.width) / 2.)?;
        bounds.expanded(f64::from(style.width) / 2.)?;
        let (r, g, b) = style.color;
        let mut border = self.make_element(
            (bounds.x, bounds.y),
            (bounds.width, bounds.height),
            Paint {
                color: format!("#{r:02x}{g:02x}{b:02x}"),
                opacity: 100,
            },
            style.width,
            Kind::Rectangle,
        )?;
        border.set(element::STROKE_STYLE, style.stroke.native())?;
        border.set(element::ROUGHNESS, Number::from(0_i64))?;
        border.set(element::FILL_STYLE, excalidraw_document::FillStyle::Solid)?;
        border.set(element::GROUP_IDS, vec![])?;
        let mut candidate = self.elements.clone();
        candidate.push(border);
        let group = GroupId(self.options.new_group_id());
        // This decorative outer selection intentionally crosses frame memberships
        // and preserves painter order, unlike SceneAuthor::group's editor action.
        for element in &mut candidate {
            let mut groups = element_groups(element)?;
            groups.push(group.clone());
            element.set(element::GROUP_IDS, groups)?;
        }
        self.elements = candidate;
        Ok(())
    }

    /// Wrap all current content in a native frame, preserving inner selection
    /// groups. Children and frame use scene-global coordinates. Existing frames
    /// or memberships are rejected: compose framed siblings instead of nesting.
    /// Padding excludes frame-name chrome and is only applied at generation time.
    /// Names follow the library's bounded single-line text policy.
    pub fn add_frame(&mut self, padding: f64, name: Option<&str>) -> Result<(), Error> {
        let bounds = self.wrapping_bounds(padding)?;
        if self
            .elements
            .iter()
            .any(|e| string(e, "type") == "frame" || !e.as_object()["frameId"].is_null())
        {
            return Err(Error::Invalid("nested native frames are unsupported"));
        }
        if let Some(name) = name {
            crate::typography::measure(name, 20.)?;
        }
        let mut frame = self.make_element(
            (bounds.x, bounds.y),
            (bounds.width, bounds.height),
            Paint {
                color: "#bbb".into(),
                opacity: 100,
            },
            1,
            Kind::Frame {
                name: name.map(str::to_owned),
            },
        )?;
        frame.set(element::GROUP_IDS, vec![])?;
        frame.set(element::ROUGHNESS, Number::from(0_i64))?;
        let id = ElementId(string(&frame, "id").to_owned());
        let children = element_ids(&self.elements);
        let mut elements = self.elements.clone();
        elements.push(frame);
        let mut candidate = Document::new("excaliplot");
        candidate.set_elements(elements);
        candidate.author(Profile::V0_18_1)?.set_frame_with_order(
            &children,
            Some(&id),
            FrameOrder::Preserve,
        )?;
        self.elements = candidate.elements()?;
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        self.to_document()?.to_vec_pretty().map_err(Error::Document)
    }

    /// Clone generated content into the general preserving document model.
    /// Edits to the returned document are independent of this drawing scene.
    pub fn to_document(&self) -> Result<Document, Error> {
        self.ensure_valid()?;
        let mut document = Document::new("excaliplot");
        document.set_elements(self.elements.clone());
        let mut state = excalidraw_document::AppState::default();
        state.set(
            excalidraw_document::app_state::VIEW_BACKGROUND_COLOR,
            "#ffffff".into(),
        )?;
        document.set_app_state(state);
        Ok(document)
    }

    /// Serialize the whole scene as one unpublished vector-only native library
    /// item (Excalidraw library format v2), retaining painter order and relationships.
    /// The item identity and creation time belong to this scene's namespace;
    /// fixture namespaces produce deterministic output. Empty scenes are rejected.
    /// This is separate from the scene document returned by [`Self::to_bytes`].
    pub fn to_library_bytes(&self) -> Result<Vec<u8>, Error> {
        self.bounds()?
            .ok_or(Error::Invalid("empty scene cannot be a library item"))?;
        let item = LibraryItem::new(
            Profile::V0_18_1,
            format!("{}-library", self.namespace),
            Number::from(self.updated),
            self.elements.clone(),
        )?;
        LibraryDocument::new("excaliplot", vec![item])
            .to_vec_pretty()
            .map_err(Error::Document)
    }

    /// Serialize before opening the destination. `Refuse` uses atomic create-new,
    /// so checking and creating cannot race with another writer.
    pub fn write(&self, path: impl AsRef<Path>, overwrite: Overwrite) -> Result<(), Error> {
        let bytes = self.to_bytes()?;
        Self::write_bytes(path, overwrite, &bytes)
    }

    /// Write one native `.excalidrawlib` item with the same explicit overwrite
    /// protection as [`Self::write`]. Failed or empty scenes cannot create or
    /// truncate a file. Serialization and reference validation precede file I/O.
    pub fn write_library(&self, path: impl AsRef<Path>, overwrite: Overwrite) -> Result<(), Error> {
        let bytes = self.to_library_bytes()?;
        Self::write_bytes(path, overwrite, &bytes)
    }

    fn write_bytes(
        path: impl AsRef<Path>,
        overwrite: Overwrite,
        bytes: &[u8],
    ) -> Result<(), Error> {
        let mut options = OpenOptions::new();
        options.write(true);
        match overwrite {
            Overwrite::Refuse => {
                options.create_new(true);
            }
            Overwrite::Allow => {
                options.create(true).truncate(true);
            }
        }
        let mut file = options.open(path).map_err(Error::Io)?;
        file.write_all(bytes).map_err(Error::Io)?;
        file.flush().map_err(Error::Io)
    }

    pub fn diagnostics(&self) -> Diagnostics {
        let mut result = Diagnostics {
            calls: self.calls.borrow().clone(),
            elements: BTreeMap::new(),
            vertices: 0,
        };
        for element in &self.elements {
            let kind = match string(element, "type") {
                "line" => {
                    result.vertices += element.as_object()["points"].as_array().unwrap().len();
                    "line"
                }
                "rectangle" => "rectangle",
                "arrow" => {
                    result.vertices += 2;
                    "arrow"
                }
                "ellipse" => "ellipse",
                "text" => "text",
                "frame" => "frame",
                _ => unreachable!("generated scene kind"),
            };
            *result.elements.entry(kind).or_default() += 1;
        }
        result
    }

    pub(crate) fn call(&self, name: &'static str) {
        *self.calls.borrow_mut().entry(name).or_default() += 1;
    }
    pub(crate) fn fail(&self, error: Error) -> Error {
        self.failed.set(true);
        error
    }

    // Owned scene geometry is fractional even when a caller (such as Plotters)
    // supplies integer coordinates. Callers validate paints and close fills.
    pub(crate) fn path(
        &mut self,
        points: Vec<(f64, f64)>,
        paint: Paint,
        stroke_width: u32,
        fill: bool,
    ) -> Result<(), Error> {
        if points.len() < 2 {
            return Ok(());
        }
        let (x, y) = points[0];
        let local: Vec<_> = points.iter().map(|&(px, py)| [px - x, py - y]).collect();
        if local.iter().flatten().any(|v| !v.is_finite()) {
            return Err(Error::Invalid("path relative coordinates overflow"));
        }
        let (min_x, min_y, max_x, max_y) = path_edges(&local);
        if max_x == min_x && max_y == min_y {
            return Ok(());
        }
        let mut element = self.make_element(
            (x, y),
            (max_x - min_x, max_y - min_y),
            paint,
            stroke_width.max(1),
            Kind::Line { points: local },
        )?;
        if fill {
            fill_only(&mut element)?;
            element.set(
                element::STROKE_STYLE,
                excalidraw_document::StrokeStyle::Solid,
            )?;
        }
        self.elements.push(element);
        Ok(())
    }

    pub(crate) fn push(
        &mut self,
        origin: (f64, f64),
        size: (f64, f64),
        paint: Paint,
        stroke_width: u32,
        kind: Kind,
    ) -> Result<&mut Element, Error> {
        let element = self.make_element(origin, size, paint, stroke_width, kind)?;
        self.elements.push(element);
        Ok(self.elements.last_mut().expect("just inserted element"))
    }

    // Construct and check only this primitive. Whole-document authoring here
    // would repeatedly clone/validate the growing scene for every Plotters mark.
    fn make_element(
        &self,
        origin: (f64, f64),
        size: (f64, f64),
        paint: Paint,
        stroke_width: u32,
        kind: Kind,
    ) -> Result<Element, Error> {
        let (x, y) = origin;
        let (width, height) = size;
        if width < 0. || height < 0. {
            return Err(Error::Invalid("element dimensions must be nonnegative"));
        }
        let ordinal = self.elements.len();
        let options = self.options.snapshot();
        let mut element = Element::authored(
            kind.native(),
            Profile::V0_18_1,
            Authoring {
                id: ElementId(format!("{}-{ordinal}", self.namespace)),
                updated: Number::from(self.updated),
                created: Field::Missing,
                seed: Number::from((ordinal % 2_147_483_646 + 1) as u64),
                version: Number::from(1_i64),
                version_nonce: Number::from(0_i64),
            },
        )?;
        for (key, value) in [
            (element::X, x),
            (element::Y, y),
            (element::WIDTH, width),
            (element::HEIGHT, height),
            (element::ANGLE, 0.),
        ] {
            element.set(key, Number::from_f64(value)?)?;
        }
        element.set(element::STROKE_COLOR, paint.color)?;
        element.set(element::STROKE_WIDTH, Number::from(u64::from(stroke_width)))?;
        element.set(
            element::ROUGHNESS,
            Number::from(u64::from(options.style.roughness)),
        )?;
        element.set(element::OPACITY, Number::from(u64::from(paint.opacity)))?;
        element.set(
            element::FILL_STYLE,
            match options.style.fill {
                FillStyle::Solid => excalidraw_document::FillStyle::Solid,
                FillStyle::Hachure => excalidraw_document::FillStyle::Hachure,
                FillStyle::CrossHatch => excalidraw_document::FillStyle::CrossHatch,
            },
        )?;
        element.set(
            element::GROUP_IDS,
            options.groups.into_iter().map(GroupId).collect(),
        )?;
        element.set(element::BOUND_ELEMENTS, vec![])?;
        match kind {
            Kind::Line { points } => {
                element.set_path(native_points(points)?)?;
                element.set(element::STROKE_STYLE, options.stroke.native())?;
            }
            Kind::Arrow { points, head } => {
                element.set_path(native_points(points)?)?;
                element.set(
                    element::END_ARROWHEAD,
                    match head {
                        ArrowHead::Arrow => excalidraw_document::Arrowhead::Arrow,
                        ArrowHead::Triangle => excalidraw_document::Arrowhead::Triangle,
                    },
                )?;
                element.set(element::STROKE_STYLE, options.stroke.native())?;
            }
            Kind::Text { text, font_size } => {
                element.set_text_content(TextContent::plain(
                    text,
                    Number::from_f64(width)?,
                    Number::from_f64(height)?,
                ))?;
                element.set(element::FONT_SIZE, Number::from_f64(font_size)?)?;
                element.set(element::FONT_FAMILY, KnownFont::Excalifont.to_number())?;
                element.set(
                    element::LINE_HEIGHT,
                    Number::from_f64(crate::typography::LINE_HEIGHT)?,
                )?;
            }
            Kind::Frame { name: Some(name) } => element.set(element::NAME, name)?,
            Kind::Rectangle | Kind::Ellipse | Kind::Frame { name: None } => {}
        }
        if let Some(source) = options.source {
            let url = serde_json::to_value(source.url).map_err(Error::Json)?;
            element.set(
                element::LINK,
                url.as_str().expect("report URL string").to_owned(),
            )?;
            if let Some(excaliplot) = source.provenance {
                let custom =
                    serde_json::to_value(CustomData { excaliplot }).map_err(Error::Json)?;
                element.set(
                    element::CUSTOM_DATA,
                    custom.as_object().expect("provenance object").clone(),
                )?;
            }
        }
        Ok(element)
    }
}

// Canonical signed significand/exponent of a product of two finite f64 values.
// Each significand has at most 53 bits; their exact product fits in 106 bits.
fn exact_product(a: f64, b: f64) -> (bool, u128, i32) {
    fn parts(value: f64) -> (bool, u128, i32) {
        let bits = value.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32;
        let fraction = u128::from(bits & ((1_u64 << 52) - 1));
        (
            bits >> 63 != 0,
            if exponent == 0 {
                fraction
            } else {
                fraction | (1_u128 << 52)
            },
            if exponent == 0 {
                -1074
            } else {
                exponent - 1075
            },
        )
    }
    let (sa, ma, ea) = parts(a);
    let (sb, mb, eb) = parts(b);
    let product = ma * mb;
    if product == 0 {
        return (false, 0, 0);
    }
    let zeros = product.trailing_zeros();
    (sa != sb, product >> zeros, ea + eb + zeros as i32)
}

fn number(element: &Element, key: &str) -> f64 {
    element.as_object()[key]
        .as_f64()
        .expect("generated finite number")
}
fn string<'a>(element: &'a Element, key: &str) -> &'a str {
    element.as_object()[key].as_str().expect("generated string")
}
pub(crate) fn fill_only(element: &mut Element) -> Result<(), Error> {
    let color = string(element, "strokeColor").to_owned();
    element.set(element::BACKGROUND_COLOR, color)?;
    element.set(element::STROKE_COLOR, "transparent".into())?;
    Ok(())
}

fn native_points(
    points: impl IntoIterator<Item = [f64; 2]>,
) -> Result<Vec<excalidraw_document::Point>, Error> {
    points
        .into_iter()
        .map(|[x, y]| Ok([Number::from_f64(x)?, Number::from_f64(y)?]))
        .collect()
}

fn element_ids(elements: &[Element]) -> Vec<ElementId> {
    elements
        .iter()
        .map(|e| ElementId(string(e, "id").to_owned()))
        .collect()
}

fn element_groups(element: &Element) -> Result<Vec<GroupId>, Error> {
    match element.get(element::GROUP_IDS)? {
        Field::Value(groups) => Ok(groups),
        _ => Err(Error::Invalid("generated element lacks groups")),
    }
}

fn clear_indices(elements: &mut [Element]) {
    for element in elements {
        element.set_null(element::INDEX);
    }
}

impl StrokeStyle {
    fn native(self) -> excalidraw_document::StrokeStyle {
        match self {
            Self::Solid => excalidraw_document::StrokeStyle::Solid,
            Self::Dashed => excalidraw_document::StrokeStyle::Dashed,
            Self::Dotted => excalidraw_document::StrokeStyle::Dotted,
        }
    }
}
fn element_bounds_offset(element: &Element, offset: (f64, f64)) -> Result<Bounds, Error> {
    let x = number(element, "x") + offset.0;
    let y = number(element, "y") + offset.1;
    let kind = string(element, "type");
    let points: Vec<[f64; 2]> = element
        .as_object()
        .get("points")
        .and_then(Value::as_array)
        .map(|points| {
            points
                .iter()
                .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
                .collect()
        })
        .unwrap_or_default();
    let (left, top, right, bottom) = match kind {
        "line" => path_edges(&points),
        "arrow" => {
            // Arrows are always unrotated. Use the same direct edge arithmetic
            // as insertion; center-based rotation can overflow near f64::MAX
            // even when both absolute endpoints are finite.
            let head = if string(element, "endArrowhead") == "triangle" {
                ArrowHead::Triangle
            } else {
                ArrowHead::Arrow
            };
            let (left, top, right, bottom) = arrow_edges(points[1], head)?;
            return Bounds::from_edges(x + left, y + top, x + right, y + bottom)?
                .expanded(number(element, "strokeWidth") / 2.);
        }
        _ => (0., 0., number(element, "width"), number(element, "height")),
    };
    let cx = left / 2. + right / 2.;
    let cy = top / 2. + bottom / 2.;
    let (sin, cos) = number(element, "angle").sin_cos();
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (px, py) in [(left, top), (right, top), (right, bottom), (left, bottom)] {
        let rx = x + cx + (px - cx) * cos - (py - cy) * sin;
        let ry = y + cy + (px - cx) * sin + (py - cy) * cos;
        if !rx.is_finite() || !ry.is_finite() {
            return Err(Error::Invalid("scene coordinates overflow"));
        }
        min_x = min_x.min(rx);
        min_y = min_y.min(ry);
        max_x = max_x.max(rx);
        max_y = max_y.max(ry);
    }
    let stroke =
        if string(element, "strokeColor") == "transparent" || matches!(kind, "text" | "frame") {
            0.
        } else {
            number(element, "strokeWidth") / 2.
        };
    Bounds::from_edges(
        min_x - stroke,
        min_y - stroke,
        max_x + stroke,
        max_y + stroke,
    )
}

fn path_edges(points: &[[f64; 2]]) -> (f64, f64, f64, f64) {
    points.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(left, top, right, bottom), &[x, y]| {
            (left.min(x), top.min(y), right.max(x), bottom.max(y))
        },
    )
}

// Excalidraw 0.18.0 getArrowheadSize/Angle: open arrow 25/20°, triangle
// 15/25°, capped to half the final segment. Straight geometry only; random
// rough-path excursions are excluded just like other scene bounds.
fn arrow_edges(delta: [f64; 2], head: ArrowHead) -> Result<(f64, f64, f64, f64), Error> {
    let [dx, dy] = delta;
    let length = dx.hypot(dy);
    if !length.is_finite() || length < 1. {
        return Err(Error::Invalid(
            "arrow length must be finite and at least one scene unit",
        ));
    }
    let (size, degrees): (f64, f64) = match head {
        ArrowHead::Arrow => (25., 20.),
        ArrowHead::Triangle => (15., 25.),
    };
    let size = size.min(length / 2.);
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (ux, uy) = (dx / length * size, dy / length * size);
    Ok(path_edges(&[
        [0., 0.],
        delta,
        [dx - ux * cos - uy * sin, dy - uy * cos + ux * sin],
        [dx - ux * cos + uy * sin, dy - uy * cos - ux * sin],
    ]))
}

// Drawing commands, not a stored or serializable document model.
pub(crate) enum Kind {
    Rectangle,
    Ellipse,
    Arrow {
        points: [[f64; 2]; 2],
        head: ArrowHead,
    },
    Frame {
        name: Option<String>,
    },
    Text {
        text: String,
        font_size: f64,
    },
    Line {
        points: Vec<[f64; 2]>,
    },
}

impl Kind {
    fn native(&self) -> ElementKind {
        match self {
            Self::Rectangle => ElementKind::Rectangle,
            Self::Ellipse => ElementKind::Ellipse,
            Self::Frame { .. } => ElementKind::Frame,
            Self::Text { .. } => ElementKind::Text,
            Self::Line { .. } => ElementKind::Line,
            Self::Arrow { .. } => ElementKind::Arrow,
        }
    }
}
