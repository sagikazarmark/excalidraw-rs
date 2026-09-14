//! Thresholds and event windows alongside measured/model series styling.
use excaliplot::{LineChart, NamedSeries, ReferenceRule, ShadedInterval, StrokeStyle};
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("annotations", || {
        LineChart::from_series(
            &[
                NamedSeries::new(
                    "Measured",
                    &[(1., 2.), (3., 5.), (6., 3.), (9., 7.)],
                    (25, 113, 194),
                )
                .line_width(2)
                .marker(4, true),
                NamedSeries::new(
                    "Model",
                    &[(1., 3.), (3., 4.), (6., 5.), (9., 6.)],
                    (25, 113, 194),
                )
                .stroke_style(StrokeStyle::Dashed)
                .opacity(0.65),
            ],
            0.0..10.0,
            0.0..10.0,
        )
        .size((800, 440))
        .labels("Threshold and event windows", "Time (s)", "Value")
        .shaded_interval(ShadedInterval::x(2.0..4.0, (255, 236, 153)).opacity(0.35))
        .shaded_interval(ShadedInterval::y(8.0..10.0, (208, 235, 255)).opacity(0.5))
        .reference_rule(
            ReferenceRule::horizontal(8., (224, 49, 49))
                .width(3)
                .stroke_style(StrokeStyle::Dashed),
        )
        .reference_rule(
            ReferenceRule::vertical(7., (224, 49, 49))
                .opacity(0.65)
                .stroke_style(StrokeStyle::Dotted),
        )
        .render()
    })
}
