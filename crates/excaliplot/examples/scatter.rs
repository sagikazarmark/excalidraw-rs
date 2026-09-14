#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("scatter", || {
        excaliplot::ScatterChart::new(
            &[
                (1.0, 2.0),
                (7.0, 6.0),
                (4.0, 3.0),
                (6.0, 8.0),
                (2.0, 5.0),
                (9.0, 9.0),
            ],
            0.0..10.0,
            0.0..10.0,
        )
        .labels("Observed samples", "Time (s)", "Value")
        .opacity(0.7)
        .render()
    })
}
