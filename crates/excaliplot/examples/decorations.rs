//! Protected fixtures: borders, one/many-diagram frames, and framed siblings.
use excaliplot::{
    BorderStyle, FillStyle, LineChart, NamedSeries, Overwrite, Scene, SketchStyle, StrokeStyle,
};

#[path = "support/destination.rs"]
mod destination;

fn chart(roughness: u8) -> Result<Scene, excaliplot::Error> {
    LineChart::from_series(
        &[
            NamedSeries::new(
                "Measured",
                &[(0., 20.), (1., 80.), (2., 30.)],
                (25, 113, 194),
            ),
            NamedSeries::new("Target", &[(0., 40.), (1., 40.), (2., 40.)], (224, 49, 49)),
        ],
        0.0..2.0,
        0.0..100.0,
    )
    .labels("Latency", "Time", "Milliseconds")
    .size((500, 340))
    .sketch(SketchStyle::new(roughness, FillStyle::Solid)?)
    .render()
}

fn pair() -> Result<Scene, excaliplot::Error> {
    let mut scene = chart(0)?;
    let mut second = chart(1)?;
    second.place_at((540., 0.))?;
    scene.append(second)?;
    Ok(scene)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let destination = destination::resolve(destination, "decorations")?;
    std::fs::create_dir(&destination)?;
    let write = |name: &str, scene: Scene| {
        scene.write(
            destination.join(format!("{name}.excalidraw")),
            Overwrite::Refuse,
        )
    };
    for (index, (name, stroke)) in [
        ("solid", StrokeStyle::Solid),
        ("dashed", StrokeStyle::Dashed),
        ("dotted", StrokeStyle::Dotted),
    ]
    .into_iter()
    .enumerate()
    {
        let mut scene = chart(index as u8)?;
        scene.add_border(
            12.,
            BorderStyle {
                color: (112, 72, 232),
                width: 4,
                stroke,
            },
        )?;
        write(name, scene)?;
    }
    let mut bordered = pair()?;
    bordered.add_border(16., BorderStyle::default())?;
    write("border-assembly", bordered)?;
    let mut single = chart(0)?;
    single.add_frame(16., Some("Single diagram"))?;
    write("frame-single", single)?;
    let mut multiple = pair()?;
    multiple.add_frame(16., Some("Several diagrams"))?;
    write("frame-assembly", multiple)?;
    let mut combined = chart(0)?;
    combined.add_border(
        12.,
        BorderStyle {
            width: 8,
            ..BorderStyle::default()
        },
    )?;
    combined.add_frame(16., Some("Border inside frame"))?;
    write("border-in-frame", combined)?;
    let mut siblings = Scene::new();
    for (index, name) in ["Clean panel", "Sketch panel"].into_iter().enumerate() {
        let mut panel = chart(index as u8)?;
        panel.add_frame(16., Some(name))?;
        panel.place_at((index as f64 * 580., 0.))?;
        siblings.append(panel)?;
    }
    write("frame-siblings", siblings)?;
    let mut outer = Scene::new();
    for index in 0..2 {
        let mut panel = chart(index)?;
        panel.add_frame(16., Some("Panel"))?;
        panel.place_at((f64::from(index) * 580., 0.))?;
        outer.append(panel)?;
    }
    outer.add_border(
        24.,
        BorderStyle {
            stroke: StrokeStyle::Dashed,
            ..BorderStyle::default()
        },
    )?;
    write("border-around-frames", outer)?;
    Ok(())
}
