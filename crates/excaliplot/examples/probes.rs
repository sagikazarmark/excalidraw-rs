use excaliplot::{BarChart, LineChart, NamedSeries, Overwrite, ScatterChart, Scene};
use serde_json::json;
use std::{path::Path, time::Instant};

#[path = "support/destination.rs"]
mod destination;

fn record(
    folder: &Path,
    name: &str,
    draw: impl FnOnce() -> Result<Scene, excaliplot::Error>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let start = Instant::now();
    let scene = draw()?;
    let bytes = scene.to_bytes()?;
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    scene.write(folder.join(format!("{name}.excalidraw")), Overwrite::Refuse)?;
    let stats = scene.diagnostics();
    assert_eq!(stats.calls.get("draw_pixel"), None);
    assert_eq!(stats.calls.get("blit_bitmap"), None);
    Ok(
        json!({"name": name, "bytes": bytes.len(), "generationMs": ms, "elements": stats.elements,
        "vertices": stats.vertices, "calls": stats.calls}),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let folder = destination::resolve(folder, "probes")?;
    let folder = folder.as_path();
    std::fs::create_dir(folder)?;
    let mut report = Vec::new();
    for count in [100, 1000, 5000] {
        // Deterministic, ordered X, bounded Y: same data for both cost models.
        let points: Vec<_> = (0..count)
            .map(|i| {
                (
                    1.0 + 8.0 * i as f64 / (count - 1) as f64,
                    1.0 + 8.0 * ((i * 37) % count) as f64 / (count - 1) as f64,
                )
            })
            .collect();
        report.push(record(folder, &format!("scatter-{count}"), || {
            ScatterChart::new(&points, 0.0..10.0, 0.0..10.0)
                .marker(2, true)
                .opacity(0.7)
                .render()
        })?);
        report.push(record(folder, &format!("line-{count}"), || {
            LineChart::new(&points, 0.0..10.0, 0.0..10.0).render()
        })?);
    }
    for (name, data, bounds) in [
        ("bars-positive", vec![("A", 2.0), ("B", 4.0)], 0.0..5.0),
        ("bars-negative", vec![("A", -2.0), ("B", -4.0)], -5.0..0.0),
        ("bars-constant", vec![("A", 2.0), ("B", 2.0)], 0.0..5.0),
        ("bars-zero", vec![("A", 0.0), ("B", 0.0)], -5.0..5.0),
    ] {
        report.push(record(folder, name, || {
            BarChart::new(&data, bounds).render()
        })?);
    }
    report.push(record(folder, "scatter-constant", || {
        ScatterChart::new(&[(1.0, 2.0), (4.0, 2.0), (8.0, 2.0)], 0.0..10.0, 0.0..10.0).render()
    })?);
    report.push(record(folder, "scatter-legend", || {
        let a = [(2.0, 3.0), (4.0, 5.0)];
        let b = [(6.0, 7.0), (8.0, 5.0)];
        ScatterChart::from_series(
            &[
                NamedSeries::new("Measured", &a, (25, 113, 194)),
                NamedSeries::new("Reference", &b, (224, 49, 49)),
            ],
            0.0..10.0,
            0.0..10.0,
        )
        .size((800, 440))
        .marker(7, false)
        .opacity(0.5)
        .render()
    })?);
    report.push(record(folder, "scatter-boundaries", || {
        ScatterChart::new(
            &[(0.0, 0.0), (10.0, 10.0), (0.0, 10.0), (10.0, 0.0)],
            0.0..10.0,
            0.0..10.0,
        )
        .marker(5, false)
        .render()
    })?);
    let bytes = serde_json::to_vec_pretty(&report)?;
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(folder.join("generation.json"))?
        .write_all(&bytes)?;
    println!("{}", String::from_utf8(bytes)?);
    Ok(())
}
