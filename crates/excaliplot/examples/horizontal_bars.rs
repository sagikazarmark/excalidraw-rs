//! Synthetic signed category comparisons with measured, unrotated long labels.
use excaliplot::{BarChart, NamedBarSeries};
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("horizontal_bars", || {
        BarChart::from_series(
            &[
                "International customer support",
                "Domestic customer support",
                "Enterprise account management",
            ],
            &[
                NamedBarSeries::new("Before", &[4., -2., 0.], (25, 113, 194)),
                NamedBarSeries::new("After", &[2., -1., 3.], (224, 49, 49)),
            ],
            -5.0..5.0,
        )
        .horizontal()
        .size((1000, 440))
        .labels("Service workload change", "Team", "Change (hours)")
        .render()
    })
}
