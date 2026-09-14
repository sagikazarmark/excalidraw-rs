//! Protected same-color comparisons and bounded native dash performance probes.
use excaliplot::{
    FillStyle, LegendPosition, LineChart, NamedSeries, Overwrite, Scene, SketchStyle, StrokeStyle,
};
use serde_json::{Value, json};
use std::{io::Write, path::Path, time::Instant};

#[path = "support/destination.rs"]
mod destination;

fn record(
    folder: &Path,
    name: &str,
    count: usize,
    draw: impl FnOnce() -> Result<Scene, excaliplot::Error>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let start = Instant::now();
    let scene = draw()?;
    let bytes = scene.to_bytes()?;
    let generation_ms = start.elapsed().as_secs_f64() * 1000.;
    scene.write(folder.join(format!("{name}.excalidraw")), Overwrite::Refuse)?;
    let stats = scene.diagnostics();
    assert_eq!(stats.calls.get("draw_pixel"), None);
    assert_eq!(stats.calls.get("blit_bitmap"), None);
    Ok(
        json!({"name": name, "count": count, "generationMs": generation_ms,
        "bytes": bytes.len(), "elements": stats.elements, "vertices": stats.vertices, "calls": stats.calls}),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let destination = destination::resolve(destination, "series-dashes")?;
    let folder = destination.as_path();
    std::fs::create_dir(folder)?;
    let mut report = Vec::new();
    for roughness in 0..=2 {
        let sketch = SketchStyle::new(roughness, FillStyle::Solid)?;
        report.push(record(
            folder,
            &format!("comparison-{roughness}"),
            4,
            || {
                LineChart::from_series(
                    &[
                        NamedSeries::new(
                            "Solid",
                            &[(1., 2.), (3., 5.), (6., 3.), (9., 5.)],
                            (25, 113, 194),
                        )
                        .stroke_style(StrokeStyle::Solid),
                        NamedSeries::new(
                            "Dashed",
                            &[(1., 4.), (3., 7.), (6., 5.), (9., 7.)],
                            (25, 113, 194),
                        )
                        .stroke_style(StrokeStyle::Dashed),
                        NamedSeries::new(
                            "Dotted",
                            &[(1., 6.), (3., 9.), (6., 7.), (9., 9.)],
                            (25, 113, 194),
                        )
                        .stroke_style(StrokeStyle::Dotted),
                    ],
                    0.0..10.0,
                    0.0..10.0,
                )
                .size((800, 440))
                .labels("Native line patterns", "Time (s)", "Value")
                .sketch(sketch)
                .render()
            },
        )?);
        // Deliberately bounded independently of the solid-path 5,000-point probe.
        for count in [100, 1000] {
            let points: Vec<_> = (0..count)
                .map(|i| {
                    (
                        1. + 8. * i as f64 / (count - 1) as f64,
                        1. + 8. * ((i * 37) % count) as f64 / (count - 1) as f64,
                    )
                })
                .collect();
            for (name, style) in [
                ("dashed", StrokeStyle::Dashed),
                ("dotted", StrokeStyle::Dotted),
            ] {
                report.push(record(
                    folder,
                    &format!("{name}-{count}-{roughness}"),
                    count,
                    || {
                        LineChart::from_series(
                            &[NamedSeries::new("S", &points, (25, 113, 194)).stroke_style(style)],
                            0.0..10.0,
                            0.0..10.0,
                        )
                        .size((800, 440))
                        .legend(LegendPosition::Off)
                        .sketch(sketch)
                        .render()
                    },
                )?);
            }
        }
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(folder.join("generation.json"))?
        .write_all(&serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
