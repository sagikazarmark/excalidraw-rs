use excaliplot::{ExcalidrawBackend, LineChart, Overwrite, Scene};
use plotters::prelude::*;
use plotters_backend::text_anchor::{HPos, Pos, VPos};
use std::path::PathBuf;

#[path = "support/destination.rs"]
mod destination;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args_os().nth(1).map(PathBuf::from);
    let destination = destination::resolve(destination, "gallery")?;
    std::fs::create_dir_all(&destination)?;
    let points = [
        (1.0, 2.0),
        (2.0, 5.0),
        (4.0, 3.0),
        (6.0, 8.0),
        (7.0, 6.0),
        (9.0, 9.0),
    ];
    LineChart::new(&points, 0.0..10.0, 0.0..10.0)
        .labels("Six-point line", "Time (s)", "Value")
        .render()?
        .write(destination.join("line.excalidraw"), Overwrite::Refuse)?;
    LineChart::new(
        &[(0.0, 2.0), (1.0, 2.0), (1.0, 2.0), (4.0, 2.0)],
        0.0..5.0,
        0.0..4.0,
    )
    .labels("Constant values and repeats", "Time", "Value")
    .render()?
    .write(destination.join("constant.excalidraw"), Overwrite::Refuse)?;
    LineChart::new(
        &[(-3.0, -2.0), (-1.0, 1.0), (0.0, 0.0), (3.0, 2.0)],
        -4.0..4.0,
        -3.0..3.0,
    )
    .labels("Signed numeric ranges", "X", "Y")
    .render()?
    .write(destination.join("signed.excalidraw"), Overwrite::Refuse)?;
    let mut scene = Scene::new();
    {
        let root = ExcalidrawBackend::new(&mut scene, (1100, 1500))?.into_drawing_area();
        root.fill(&WHITE)?;
        let corpus = [
            "i",
            "WWWW",
            "AV To WA",
            "fi fl office",
            "0123456789",
            "+1 -2 −3.5",
            "two  spaces",
            "gypqj",
            "Café",
            "A longer label with spaces and digits 123",
        ];
        for (index, text) in corpus.iter().enumerate() {
            root.draw(&Text::new(
                *text,
                (40, 30 + index as i32 * 35),
                ("Excalifont", 20),
            ))?;
        }
        for (row, (h, h_name)) in [
            (HPos::Left, "left"),
            (HPos::Center, "center"),
            (HPos::Right, "right"),
        ]
        .into_iter()
        .enumerate()
        {
            for (subrow, (v, v_name)) in [
                (VPos::Top, "top"),
                (VPos::Center, "center"),
                (VPos::Bottom, "bottom"),
            ]
            .into_iter()
            .enumerate()
            {
                for (column, rotation) in [
                    FontTransform::None,
                    FontTransform::Rotate90,
                    FontTransform::Rotate180,
                    FontTransform::Rotate270,
                ]
                .into_iter()
                .enumerate()
                {
                    let (x, y) = (
                        140 + column as i32 * 260,
                        450 + (row * 3 + subrow) as i32 * 115,
                    );
                    root.draw(&Text::new(
                        format!("{h_name}/{v_name} {}", column * 90),
                        (x - 80, y - 48),
                        ("Excalifont", 12),
                    ))?;
                    root.draw(&PathElement::new([(x - 5, y), (x + 5, y)], RED))?;
                    root.draw(&PathElement::new([(x, y - 5), (x, y + 5)], RED))?;
                    let style = TextStyle::from(("Excalifont", 19).into_font())
                        .pos(Pos::new(h, v))
                        .transform(rotation);
                    root.draw_text("AV gyp", &style, (x, y))?;
                }
            }
        }
        root.present()?;
    }
    scene.write(destination.join("typography.excalidraw"), Overwrite::Refuse)?;
    let mut units = Scene::new();
    {
        let root = ExcalidrawBackend::new(&mut units, (640, 400))?.into_drawing_area();
        for (index, text) in [
            "°",
            "±",
            "20°C",
            "20 ± 2 °C",
            "−5.0°C ±0.2",
            "Café: 20°C (±2)",
            " ° ± ",
            "°±°",
        ]
        .iter()
        .enumerate()
        {
            root.draw(&Text::new(
                *text,
                (40, 30 + index as i32 * 35),
                ("Excalifont", 20),
            ))?;
        }
    }
    units.write(
        destination.join("units-corpus.excalidraw"),
        Overwrite::Refuse,
    )?;
    println!("Gallery written to {}", destination.display());
    Ok(())
}
