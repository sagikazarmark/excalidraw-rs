//! Ordered synthetic envelope with caller-supplied limits and center values.
use excaliplot::BandChart;
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("bands", || {
        BandChart::new(
            &[1., 2., 4., 6., 8., 9.],
            &[2., 3., 2., 4., 3., 4.],
            &[5., 7., 6., 9., 7., 6.],
            "Supplied range",
            0.0..10.0,
            0.0..10.0,
        )
        .center(&[3., 5., 4., 6., 5., 5.])
        .boundary_lines(true)
        .labels("Ordered supplied envelope", "Time", "Value")
        .size((800, 440))
        .render()
    })
}
