//! Synthetic irregular observations: leap day/month boundary and sub-day UTC.
use chrono::{Duration, NaiveDate, TimeZone, Utc};
use excaliplot::{DateLineChart, Scene, UtcScatterChart};
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("date_axes", || {
        let date = |month, day| NaiveDate::from_ymd_opt(2024, month, day).unwrap();
        let points = [
            (date(2, 28), 2.),
            (date(2, 29), 5.),
            (date(3, 3), 3.),
            (date(3, 7), 6.),
        ];
        let mut scene = Scene::new();
        scene.append(
            DateLineChart::new(&points, date(2, 28)..date(3, 7), 0.0..8.0)
                .labels("Irregular daily observations", "Calendar date", "Value")
                .tick_density(3, 6)
                .render()?,
        )?;
        let start = Utc.with_ymd_and_hms(2024, 12, 31, 23, 0, 0).unwrap();
        let end = start + Duration::hours(2);
        let points = [
            (end, 4.),
            (start + Duration::minutes(30), 5.),
            (start + Duration::minutes(30), 3.),
            (start, 2.),
        ];
        let mut panel = UtcScatterChart::new(&points, start..end, 0.0..8.0)
            .labels("UTC observations across midnight", "UTC instant", "Value")
            .size((1400, 400))
            .tick_density(3, 6)
            .render()?;
        panel.place_at((0., 440.))?;
        scene.append(panel)?;
        Ok(scene)
    })
}
