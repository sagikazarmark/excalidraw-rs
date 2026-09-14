//! Synthetic supplied frequencies: unequal signed-X bins, zero and repeated counts.
use excaliplot::HistogramChart;
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("histogram", || {
        HistogramChart::new(
            &[-10., -8., -4., 0., 2., 6., 10.],
            &[4., 8., 0., 6., 8., 3.],
            -10.0..10.0,
            0.0..10.0,
        )
        .labels("Unequal-bin frequencies", "Measurement", "Count")
        .size((800, 440))
        .render()
    })
}
