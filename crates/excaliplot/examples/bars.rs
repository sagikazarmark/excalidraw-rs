#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("bars", || {
        excaliplot::BarChart::new(
            &[
                ("North", 4.0),
                ("South", -3.0),
                ("East", 0.0),
                ("West", 2.0),
            ],
            -5.0..5.0,
        )
        .labels("Change by region", "Region", "Change")
        .render()
    })
}
