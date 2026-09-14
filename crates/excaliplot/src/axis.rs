use plotters::coord::ranged1d::{DefaultFormatting, KeyPointHint, Ranged};
use plotters::coord::types::RangedCoordf64;
use plotters::prelude::{IntoLogRange, LogCoord};
use std::ops::Range;

/// Numeric line/scatter axis mapping. Linear is the default; log axes use
/// explicit positive bounds and Plotters' fixed base-10 coordinates/key points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AxisScale {
    #[default]
    Linear,
    /// Bounds must be increasing within 1e-15..=1e9, with distinct natural
    /// logarithms. Unlike linear axes, no absolute minimum span applies.
    /// Plotters 0.3.7 suppresses ticks below f64::EPSILON; the lower bound
    /// deliberately stays above that cutoff. Tick fit and mapped geometry are
    /// still checked. Use scientific formatting for tiny values.
    /// Automatic ranges, areas and bars do not support this scale.
    Log10,
}

// A single coordinate type keeps all scale combinations on the shared measured
// Cartesian route. Mapping and tick selection belong entirely to Plotters.
pub(crate) enum NumericCoord {
    Linear(RangedCoordf64),
    Log10(LogCoord<f64>),
}

impl AxisScale {
    pub(crate) fn validate(self, range: &Range<f64>) -> Result<(), crate::Error> {
        use crate::Error;
        if !range.start.is_finite()
            || !range.end.is_finite()
            || range.start >= range.end
            || !(range.end - range.start).is_finite()
        {
            return Err(Error::Invalid(
                "axis bounds must be finite, increasing, with finite nonzero span",
            ));
        }
        if self == Self::Log10 {
            if range.start < 1e-15 || range.end > 1e9 {
                return Err(Error::Invalid(
                    "log10 chart axes require positive bounds within 1e-15..=1e9",
                ));
            }
            if range.start.ln() >= range.end.ln() {
                return Err(Error::Invalid(
                    "log axis bounds collapse at logarithmic floating-point resolution",
                ));
            }
        } else if range.start.abs() > 1e9 || range.end.abs() > 1e9 || range.end - range.start < 1e-6
        {
            return Err(Error::Invalid(
                "linear chart axes require bounds within ±1e9 and span >= 1e-6",
            ));
        }
        Ok(())
    }

    pub(crate) fn coordinate(self, range: Range<f64>) -> NumericCoord {
        match self {
            Self::Linear => NumericCoord::Linear(range.into()),
            Self::Log10 => NumericCoord::Log10(range.log_scale().into()),
        }
    }
}

impl Ranged for NumericCoord {
    type FormatOption = DefaultFormatting;
    type ValueType = f64;

    fn map(&self, value: &f64, limit: (i32, i32)) -> i32 {
        match self {
            Self::Linear(coord) => coord.map(value, limit),
            Self::Log10(coord) => coord.map(value, limit),
        }
    }

    fn key_points<Hint: KeyPointHint>(&self, hint: Hint) -> Vec<f64> {
        match self {
            Self::Linear(coord) => coord.key_points(hint),
            Self::Log10(coord) => {
                let mut ticks = coord.key_points(hint);
                // Plotters 0.3.7 can emit a decade boundary twice when minor
                // ticks fill a decade. Remove identical adjacent coordinates,
                // not distinct values that happen to format to the same label.
                ticks.dedup();
                ticks
            }
        }
    }

    fn range(&self) -> Range<f64> {
        match self {
            Self::Linear(coord) => coord.range(),
            Self::Log10(coord) => coord.range(),
        }
    }
}
