//! Combined measured/model styling, threshold/window and explicit outlier callouts.
use excaliplot::{
    ArrowHead, ArrowStyle, Callout, LineChart, NamedSeries, ReferenceRule, ShadedInterval,
    StrokeStyle,
};
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("callouts", || {
        LineChart::from_series(
            &[
                NamedSeries::new(
                    "Measured",
                    &[(1., 2.), (3., 5.), (6., 8.), (9., 4.)],
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
        .labels("Explaining observations", "Time (s)", "Value")
        .shaded_interval(ShadedInterval::x(2.0..4.0, (255, 236, 153)).opacity(0.35))
        .reference_rule(
            ReferenceRule::horizontal(7., (224, 49, 49)).stroke_style(StrokeStyle::Dotted),
        )
        .callout(Callout::new((6., 8.), "Outlier", (40., -55.)))
        .callout(
            Callout::new((9., 4.), "Recovery", (-150., 45.)).arrow_style(ArrowStyle {
                head: ArrowHead::Triangle,
                color: (47, 158, 68),
                ..ArrowStyle::default()
            }),
        )
        .render()
    })
}
