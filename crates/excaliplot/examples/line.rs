#[path = "support/line_chart.rs"]
mod line_chart;
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("line", line_chart::draw)
}
