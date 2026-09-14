use excaliplot::{LineChart, Scene};

pub fn draw() -> Result<Scene, excaliplot::Error> {
    LineChart::new(
        &[
            (1.0, 2.0),
            (2.0, 5.0),
            (4.0, 3.0),
            (6.0, 8.0),
            (7.0, 6.0),
            (9.0, 9.0),
        ],
        0.0..10.0,
        0.0..10.0,
    )
    .labels("Six-point line", "Time (s)", "Value")
    .render()
}
