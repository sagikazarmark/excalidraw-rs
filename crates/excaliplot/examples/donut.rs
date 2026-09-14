#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("donut", || {
        excaliplot::PieChart::new(&[
            excaliplot::NamedSlice::new("Major", 75.0, (25, 113, 194)),
            excaliplot::NamedSlice::new("Narrow", 1.0, (230, 119, 0)),
            excaliplot::NamedSlice::new("Rest", 24.0, (47, 158, 68)),
        ])
        .title("Editable donut")
        .donut(0.5)
        .render()
    })
}
