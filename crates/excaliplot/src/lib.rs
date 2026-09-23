//! High-level editable Excalidraw charts built on Plotters and `plotters-excalidraw`.
//!
//! Native scene, backend, styling and export types are re-exported from
//! [`plotters_excalidraw`] so chart helpers and custom drawings share one scene.
//!
//! Start with a chart helper for validated caller data:
//!
//! ```
//! use excaliplot::LineChart;
//!
//! let points = [(1.0, 2.0), (2.0, 5.0), (4.0, 3.0)];
//! let scene = LineChart::new(&points, 0.0..5.0, 0.0..6.0)
//!     .labels("Measurements", "Time (s)", "Value")
//!     .render()?;
//! let bytes = scene.to_bytes()?;
//! assert!(!bytes.is_empty());
//! # Ok::<(), excaliplot::Error>(())
//! ```
//!
//! [`LineChart`], [`BarChart`], [`ScatterChart`], [`AreaChart`] and [`PieChart`]
//! emit native text, lines, rectangles and ellipses. [`PieChart::donut`] enables
//! donut sectors. [`ExcalidrawBackend`] adapts the supported Plotters primitives
//! for custom drawings. Visible pixels and bitmaps return [`Error::Unsupported`].
//! [`AreaChart::from_series`] provides overlapping areas; [`BarChart::from_series`]
//! provides grouped bars using [`NamedBarSeries`]. Both support `.stacked()` and
//! `.percent_stacked()` for nonnegative data with explicit cumulative/percent bounds.
//! [`BarChart::horizontal`] supports single/grouped bars
//! with measured, unrotated categories and signed left/right lengths; horizontal
//! stacking is explicitly unsupported.
//! [`ErrorBarChart`] renders caller-supplied [`VerticalInterval`] limits with an
//! explicit interpretation label, containing linear bounds, and one native
//! cap/stem/center group per observation. No statistical intervals are calculated.
//! [`BandChart`] renders supplied varying lower/upper envelopes at shared ordered
//! X, with optional independent boundary/center paths and a measured fill legend.
//! It validates aligned finite limits, containing bounds and resolved geometry.
//! [`HistogramChart`] renders supplied strictly increasing bin edges and finite
//! nonnegative counts as numeric-width rectangles from zero, without aggregation.
//! [`StepChart`] renders bounded right-continuous post-steps with strict X order.
//! [`StepChart::ecdf`] explicitly sorts finite samples, aggregates ties and draws
//! cumulative fractions on `[0,1]`, with caller bounds exposing both endpoint tails.
//! [`DateLineChart`] / [`DateScatterChart`] accept Chrono calendar dates;
//! [`UtcLineChart`] / [`UtcScatterChart`] accept UTC instants with explicit bounds,
//! elapsed-time spacing and measured calendar ticks. Their numeric Y is linear.
//! [`ReferenceRule`] and [`ShadedInterval`] add explicit data-coordinate annotations
//! to numeric line/scatter/area and date/UTC helpers, using their final mapping.
//! Windows paint before axes/data, rules after data and before legends; each has
//! its own inner group. Invalid or collapsed bounds return no partial scene.
//! [`Callout`] adds a measured single-line note at an explicit scene-unit offset
//! with a genuine unbound native arrow to the mapped anchor, after rules and
//! before legends. [`Scene::add_arrow`] and [`Scene::add_note`] also work directly
//! in scene coordinates, with atomic validation and complete head/text bounds.
//!
//! # Supported boundary
//!
//! Text uses normal Excalifont (font ID 5), printable ASCII plus `é`, `−`, `°`, and `±`,
//! and single-line chart/backend labels. [`Scene::add_note`] additionally accepts
//! explicit newlines in one editable text element. Metrics use the bundled SIL OFL font; native scenes
//! rely on the receiving editor's font. Arbitrary fonts, automatic Plotters
//! legends, exact out-of-range clipping, and raster output are unsupported.
//! Clean geometry is the default; [`SketchStyle`] is opt-in.
//!
//! Current browser checks use Excalidraw 0.18.1; earlier chart acceptance used
//! 0.18.0. Closed filled paths have independent first/last endpoints in 0.18.0: moving the closure
//! handle can remove the fill until it is re-closed. Editing is one-way and
//! does not update data/scales or survive regeneration automatically.
//!
//! # Output and ownership
//!
//! [`Scene::write`] requires an explicit [`Overwrite`] policy; prefer
//! [`Overwrite::Refuse`] to protect manually edited files. For custom Plotters
//! drawings, drop all drawing areas before serializing the borrowed scene.
//! Propagate drawing errors: backend failures prevent subsequent serialization.
//! Generation is synchronous and needs no Node, browser or system font runtime.
//! [`Scene::to_library_bytes`] and [`Scene::write_library`] export the complete
//! scene as one unpublished vector-only `.excalidrawlib` item, retaining groups,
//! finite backgrounds, borders and frames. Empty scenes are rejected. Import it
//! from the editor's library panel and click to insert independent instances.
//!
//! # Composition and decoration
//!
//! [`Scene::bounds`] covers complete geometric content. [`Scene::place_at`] aligns
//! those bounds; [`Scene::translate`] instead applies an offset. [`Scene::append`]
//! consumes generated scenes, remapping identities and preserving painter order,
//! groups and frame membership. Composed scenes remain drawable and composable.
//! [`Scene::add_border`] adds a transparent grouped rectangle; [`Scene::add_frame`]
//! establishes actual native membership. Frame siblings are supported, nesting is
//! rejected. Padding is generation-time geometry, not an editor layout constraint.
//! Invalid operations leave the destination unchanged; drawing failures still
//! prevent export. See [`Bounds`] for the geometric (not exact ink) convention.
//!
//! # Source reports
//!
//! [`DrawingOptions::with_source`] targets explicit label/mark emissions with a
//! validated HTTPS [`ReportUrl`] in a [`SourceLink`]. Optional [`Provenance`] is
//! bounded structured JSON under `customData.excaliplot`; both fields are absent
//! by default. Composition and native library export preserve them. Source keys
//! are descriptive and may repeat on copy, unlike remapped element identities.
//! There is no live refresh or metadata-driven regeneration.

// The README's example is a contract too, and nothing compiled it.
// `cfg(doctest)` compiles its fences without prepending the README to the
// rendered crate documentation.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme {}

mod annotations;
mod axis;
mod bands;
mod calendar;
mod cartesian;
mod chart;
mod error_bars;
mod histogram;
mod pie;
mod steps;
mod ticks;
use plotters_excalidraw::text as typography;

pub use annotations::{Callout, ReferenceRule, ShadedInterval};
pub use axis::AxisScale;
pub use bands::BandChart;
pub use calendar::{DateLineChart, DateScatterChart, UtcLineChart, UtcScatterChart};
pub use chart::{
    AreaChart, BarChart, LegendPosition, LineChart, NamedBarSeries, NamedSeries, ScatterChart,
};
pub use error_bars::{ErrorBarChart, VerticalInterval};
pub use histogram::HistogramChart;
pub use pie::{NamedSlice, PieChart};
pub use plotters_excalidraw::{
    ArrowHead, ArrowStyle, BorderStyle, Bounds, Diagnostics, DrawingGroup, DrawingOptions, Error,
    ExcalidrawBackend, FillStyle, Overwrite, Provenance, ReportUrl, Scene, SketchStyle, SourceLink,
    StrokeStyle,
};
pub use steps::StepChart;
pub use ticks::TickFormat;
