//! A vector Plotters backend that produces editable native Excalidraw documents.
//!
//! Draw with ordinary Plotters, then export the owned [`Scene`].
//!
//! ```
//! use plotters::prelude::*;
//! use plotters_excalidraw::{ExcalidrawBackend, Scene};
//!
//! let mut scene = Scene::new();
//! {
//!     let root = ExcalidrawBackend::new(&mut scene, (640, 400))?.into_drawing_area();
//!     root.fill(&WHITE)?;
//!     let mut chart = ChartBuilder::on(&root)
//!         .margin(30)
//!         .x_label_area_size(40)
//!         .y_label_area_size(40)
//!         .build_cartesian_2d(0.0..10.0, 0.0..10.0)?;
//!     chart.configure_mesh().label_style(("Excalifont", 16)).draw()?;
//!     chart.draw_series(LineSeries::new([(1., 2.), (5., 8.), (9., 4.)], &BLUE))?;
//!     root.present()?;
//! }
//! let bytes = scene.to_bytes()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Supports paths, filled polygons, rectangles, circles and measured normal
//! Excalifont text. Pixel drawing, bitmaps and other fonts are unsupported.
//! Unsupported drawing poisons the scene so a partial document cannot be exported.
//! `present` is a checkpoint; only explicit export methods perform file I/O.
//!
//! Native notes, arrows, fractional polygons, grouping, styling, frames, composition,
//! links and library export are available without depending on Plotters itself.
//! For ready-made chart helpers, use the `excaliplot` crate.

// The README's example is a contract too, and nothing compiled it.
// `cfg(doctest)` compiles its fences without prepending the README to the
// rendered crate documentation.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme {}

mod backend;
mod options;
mod provenance;
mod scene;
mod typography;

/// Fractional Excalifont measurement shared by native rendering and chart layout.
pub mod text {
    pub use crate::typography::measure;
}

pub use backend::ExcalidrawBackend;
pub use options::{DrawingGroup, DrawingOptions, FillStyle, SketchStyle};
pub use provenance::{Provenance, ReportUrl, SourceLink};
pub use scene::{
    ArrowHead, ArrowStyle, BorderStyle, Bounds, Diagnostics, Overwrite, Scene, StrokeStyle,
};

/// Errors are returned explicitly; dropping a backend never writes a file.
#[derive(Debug)]
pub enum Error {
    Unsupported(&'static str),
    Invalid(&'static str),
    UnsupportedGlyph(char),
    Drawing(String),
    FailedScene,
    Json(serde_json::Error),
    Document(excalidraw_document::Error),
    Io(std::io::Error),
}

impl From<excalidraw_document::Error> for Error {
    fn from(error: excalidraw_document::Error) -> Self {
        Self::Document(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(op) => write!(f, "unsupported operation: {op}"),
            Self::Invalid(reason) => write!(f, "invalid input: {reason}"),
            Self::UnsupportedGlyph(ch) => {
                write!(f, "unsupported glyph U+{:04X}: {ch:?}", *ch as u32)
            }
            Self::Drawing(message) => write!(f, "drawing failed: {message}"),
            Self::FailedScene => write!(f, "cannot export a scene after a drawing error"),
            Self::Json(error) => error.fmt(f),
            Self::Document(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Document(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}
