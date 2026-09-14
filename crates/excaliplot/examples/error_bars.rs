//! Six synthetic observations with explicitly supplied limits, not calculated CIs.
use excaliplot::{ErrorBarChart, VerticalInterval};
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("error_bars", || {
        ErrorBarChart::new(
            &[
                VerticalInterval::new(1., 2., 3., 6.),
                VerticalInterval::new(3., 3., 5., 8.),
                VerticalInterval::new(5., 2., 6., 7.),
                VerticalInterval::new(7., 4., 5., 9.),
                VerticalInterval::new(9., 3., 3., 3.),
                VerticalInterval::new(3., 3., 5., 8.),
            ],
            "Supplied 95% CI",
            0.0..10.0,
            0.0..10.0,
        )
        .labels("Six supplied intervals", "Observation", "Estimate")
        .size((800, 440))
        .render()
    })
}
