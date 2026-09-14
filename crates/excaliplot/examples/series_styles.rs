//! Same-color comparisons distinguished by native widths and circle markers.
use excaliplot::{LineChart, NamedSeries};
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("series_styles", || {
        LineChart::from_series(
            &[
                NamedSeries::new(
                    "Before",
                    &[(1., 2.), (3., 5.), (6., 3.), (9., 7.)],
                    (25, 113, 194),
                )
                .line_width(2)
                .marker(5, true),
                NamedSeries::new(
                    "After",
                    &[(1., 4.), (3., 7.), (6., 5.), (9., 9.)],
                    (25, 113, 194),
                )
                .line_width(6)
                .opacity(0.65)
                .marker(7, false),
            ],
            0.0..10.0,
            0.0..10.0,
        )
        .size((800, 440))
        .labels("Same-color comparison", "Time (s)", "Value")
        .render()
    })
}
