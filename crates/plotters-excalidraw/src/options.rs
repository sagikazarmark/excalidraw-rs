use crate::{Error, SourceLink, StrokeStyle};
use serde::Serialize;
use std::{cell::RefCell, rc::Rc};

/// Native Excalidraw fill pattern. Source RGB/alpha remain authoritative.
#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FillStyle {
    #[default]
    Solid,
    Hachure,
    CrossHatch,
}

/// Added sketch styling only; never overrides source colors or stroke widths.
#[derive(Clone, Copy, Debug, Default)]
pub struct SketchStyle {
    pub(crate) roughness: u8,
    pub(crate) fill: FillStyle,
}

impl SketchStyle {
    /// Native fill pattern selected for this style.
    pub fn fill_style(self) -> FillStyle {
        self.fill
    }

    pub fn new(roughness: u8, fill: FillStyle) -> Result<Self, Error> {
        if roughness > 2 {
            return Err(Error::Invalid("supported roughness is 0, 1, or 2"));
        }
        Ok(Self { roughness, fill })
    }
}

#[derive(Clone, Default)]
pub(crate) struct Snapshot {
    pub(crate) style: SketchStyle,
    pub(crate) stroke: StrokeStyle,
    pub(crate) groups: Vec<String>,
    pub(crate) source: Option<SourceLink>,
}

struct State {
    namespace: String,
    next_group: usize,
    active: Snapshot,
}

/// A scene-local synchronous scope handle, usable after a backend is consumed by
/// `into_drawing_area`. All areas of that scene observe the current scope.
#[derive(Clone)]
pub struct DrawingOptions(Rc<RefCell<State>>);

impl DrawingOptions {
    pub(crate) fn new(namespace: String) -> Self {
        Self(Rc::new(RefCell::new(State {
            namespace,
            next_group: 0,
            active: Snapshot::default(),
        })))
    }
    pub(crate) fn snapshot(&self) -> Snapshot {
        self.0.borrow().active.clone()
    }

    pub fn new_group(&self) -> DrawingGroup {
        DrawingGroup {
            options: self.clone(),
            id: self.new_group_id(),
        }
    }

    pub(crate) fn new_group_id(&self) -> String {
        let mut state = self.0.borrow_mut();
        let id = format!("{}-group-{}", state.namespace, state.next_group);
        state.next_group += 1;
        id
    }

    /// Restore previous styling on return, Err, or panic unwinding. The closure
    /// executes without holding a borrow of the scope state.
    pub fn with_style<T>(&self, style: SketchStyle, draw: impl FnOnce() -> T) -> T {
        let previous = self.snapshot();
        self.0.borrow_mut().active.style = style;
        let _restore = Restore {
            options: self,
            previous,
        };
        draw()
    }

    /// Native pattern for unfilled paths emitted during this synchronous scope.
    /// Preserves vertices, source width and alpha; circles, fills and text are
    /// unaffected. Scope only intended paths, then re-enter for delayed legends.
    /// Dash spacing is editor-defined, not Plotters dash length/gap/phase.
    /// Nested scopes restore on return, error or panic; other scenes are isolated.
    pub fn with_stroke_style<T>(&self, stroke: StrokeStyle, draw: impl FnOnce() -> T) -> T {
        let previous = self.snapshot();
        self.0.borrow_mut().active.stroke = stroke;
        let _restore = Restore {
            options: self,
            previous,
        };
        draw()
    }

    /// Attach a validated source only to elements emitted during this closure.
    /// All interleaved areas of this scene see it; other scenes do not. Nested
    /// scopes replace the source (`None` temporarily clears it), then restore on
    /// return, error or panic. Already emitted and appended content is unaffected.
    /// Scope only intended labels/marks, rather than an entire chart with axes.
    pub fn with_source<T>(&self, source: Option<&SourceLink>, draw: impl FnOnce() -> T) -> T {
        let previous = self.snapshot();
        self.0.borrow_mut().active.source = source.cloned();
        let _restore = Restore {
            options: self,
            previous,
        };
        draw()
    }
}

/// Explicit reusable group identity. Re-enter it for delayed legend emission;
/// grouping is never inferred from paint colors or draw order.
#[derive(Clone)]
pub struct DrawingGroup {
    options: DrawingOptions,
    id: String,
}

impl DrawingGroup {
    pub fn scope<T>(&self, draw: impl FnOnce() -> T) -> T {
        let previous = self.options.snapshot();
        if !previous.groups.contains(&self.id) {
            self.options
                .0
                .borrow_mut()
                .active
                .groups
                .insert(0, self.id.clone());
        }
        let _restore = Restore {
            options: &self.options,
            previous,
        };
        draw()
    }
}

struct Restore<'a> {
    options: &'a DrawingOptions,
    previous: Snapshot,
}
impl Drop for Restore<'_> {
    fn drop(&mut self) {
        self.options.0.borrow_mut().active = std::mem::take(&mut self.previous);
    }
}
