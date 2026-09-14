//! Eighteen realistic synthetic scenarios, each at roughness 0, 1, and 2.
//! Compose fresh helper scenes through the owned scene model.
use excaliplot::{
    AreaChart, BarChart, ExcalidrawBackend, FillStyle, LineChart, NamedBarSeries, NamedSeries,
    NamedSlice, Overwrite, PieChart, ScatterChart, Scene, SketchStyle,
};
use plotters::prelude::*;
use std::{ffi::OsStr, path::PathBuf};

#[path = "support/destination.rs"]
mod destination;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const SIZE: (u32, u32) = (960, 460);
const LEFT: i32 = 40;
const TOP: i32 = 260;
const COLUMN: i32 = 1000;
const ROW: i32 = 580;
const BLUE: (u8, u8, u8) = (25, 113, 194);
const RED: (u8, u8, u8) = (224, 49, 49);
const GREEN: (u8, u8, u8) = (47, 158, 68);
const ORANGE: (u8, u8, u8) = (230, 119, 0);
const PURPLE: (u8, u8, u8) = (112, 72, 232);

const NOTES: [&str; 18] = [
    "01 / LINE - Checkout API; deploy at +20 min, rollback at +35, recovery by +50.",
    "02 / LINE - Same incident; HTTP 500 peaks at 34.5% of requests at +30 min.",
    "03 / BAR - Last 24 hours; p95 of successful requests, grouped by API route.",
    "04 / BAR - Monthly cloud-cost change versus August; negative values are savings.",
    "05 / SCATTER - One-minute API samples; latency rises sharply above 75% CPU.",
    "06 / SCATTER - Query samples; adding an index reduces latency at similar row counts.",
    "07 / AREA - Weekday edge traffic; lunchtime and evening peaks, five-minute averages.",
    "08 / AREA - Same incident; retries build a queue, drained after the +35 min rollback.",
    "09 / PIE - September cloud bill: USD 48,000 total; labels show cost and share.",
    "10 / PIE - One weekday: 12 million requests; labels show region volume and share.",
    "11 / DONUT - CDN cache outcomes: 2 million requests during the busiest hour.",
    "12 / DONUT - Release snapshot: 240 service instances, including failed health checks.",
    "13 / OVERLAPPING AREA - Inbound/outbound edge throughput; translucent fills show both series.",
    "14 / STACKED AREA - Same incident counts as row 02; band thickness is requests per minute.",
    "15 / 100% STACKED AREA - Same counts normalized at each sample; HTTP 500 share rises during the incident.",
    "16 / GROUPED BAR - Successful-request p95 by route, before and after connection-pool tuning.",
    "17 / STACKED BAR - Monthly cloud spend by service; September matches the USD 48,000 pie in row 09.",
    "18 / 100% STACKED BAR - Same monthly spend normalized by month, highlighting changes in cost mix.",
];

fn samples(values: &[f64], step: f64) -> Vec<(f64, f64)> {
    values
        .iter()
        .enumerate()
        .map(|(i, &y)| (i as f64 * step, y))
        .collect()
}

fn charts(style: SketchStyle) -> Result<Vec<Scene>> {
    // One-minute percentile windows sampled every five minutes, starting at 09:00 UTC.
    let p95 = samples(
        &[
            118., 121., 116., 125., 210., 480., 720., 610., 310., 165., 128., 122., 119.,
        ],
        5.,
    );
    let p99 = samples(
        &[
            245., 258., 242., 270., 510., 1150., 1680., 1420., 790., 380., 275., 260., 248.,
        ],
        5.,
    );
    // Status counts for those same one-minute windows; total traffic stays near 1,000 rps.
    let ok = samples(
        &[
            59400., 60500., 59900., 61100., 56200., 47100., 39700., 43800., 54400., 59000., 60500.,
            60800., 60100.,
        ],
        5.,
    );
    let missing = samples(
        &[
            360., 380., 350., 390., 410., 420., 430., 410., 390., 370., 360., 350., 360.,
        ],
        5.,
    );
    let errors = samples(
        &[
            40., 45., 38., 55., 4200., 13700., 21100., 16800., 6100., 900., 85., 50., 42.,
        ],
        5.,
    );
    let status_rates: Vec<Vec<_>> = [&ok, &missing, &errors]
        .iter()
        .map(|series| {
            series
                .iter()
                .enumerate()
                .map(|(i, &(x, count))| (x, 100. * count / (ok[i].1 + missing[i].1 + errors[i].1)))
                .collect()
        })
        .collect();
    let traffic = samples(
        &[
            1.8, 1.4, 1.1, 0.9, 0.8, 1.0, 1.7, 3.1, 4.8, 6.2, 7.1, 8.4, 9.2, 8.5, 7.6, 7.3, 7.9,
            9.6, 11.2, 10.5, 8.8, 6.5, 4.2, 2.7, 1.9,
        ],
        1.,
    );
    let queue = samples(
        &[
            120., 90., 110., 140., 900., 3100., 6400., 8200., 6100., 3200., 950., 180., 100.,
        ],
        5.,
    );
    let cpu = [
        (22., 82.),
        (28., 89.),
        (31., 85.),
        (35., 98.),
        (39., 94.),
        (43., 108.),
        (47., 104.),
        (51., 120.),
        (55., 115.),
        (58., 133.),
        (61., 127.),
        (64., 149.),
        (67., 160.),
        (70., 182.),
        (73., 198.),
        (76., 245.),
        (79., 290.),
        (82., 365.),
        (85., 430.),
        (88., 555.),
        (91., 680.),
        (94., 810.),
    ];
    // Thousands of rows examined versus wall-clock milliseconds; deliberately unordered.
    let before = [
        (12., 92.),
        (3., 36.),
        (28., 184.),
        (8., 65.),
        (45., 285.),
        (18., 121.),
        (65., 422.),
        (35., 230.),
        (85., 548.),
        (55., 350.),
        (95., 618.),
        (75., 486.),
    ];
    let after = [
        (12., 24.),
        (3., 12.),
        (28., 39.),
        (8., 20.),
        (45., 56.),
        (18., 30.),
        (65., 76.),
        (35., 48.),
        (85., 99.),
        (55., 65.),
        (95., 113.),
        (75., 87.),
    ];
    let inbound = samples(
        &[
            0.6, 0.4, 0.3, 0.7, 1.8, 2.4, 3.1, 2.8, 2.6, 3.8, 2.2, 1.1, 0.6,
        ],
        2.,
    );
    let outbound = samples(
        &[
            1.4, 1.0, 0.8, 1.6, 3.5, 5.2, 6.4, 5.7, 6.1, 8.2, 5.3, 2.7, 1.5,
        ],
        2.,
    );
    // Keep request counts in thousands so fixed-width numeric tick labels fit.
    let status_counts: Vec<Vec<_>> = [&ok, &missing, &errors]
        .iter()
        .map(|s| s.iter().map(|&(x, y)| (x, y / 1000.)).collect())
        .collect();
    let statuses = [
        NamedSeries::new("200 OK", &status_counts[0], GREEN),
        NamedSeries::new("404 Not Found", &status_counts[1], ORANGE),
        NamedSeries::new("500 Server Error", &status_counts[2], RED),
    ];
    let costs = [
        NamedBarSeries::new("Compute", &[24.8, 21.6, 22.3, 23.1], BLUE),
        NamedBarSeries::new("Database", &[10.6, 12.0, 12.5, 13.2], PURPLE),
        NamedBarSeries::new("Storage", &[6.6, 7.2, 7.6, 8.0], GREEN),
        NamedBarSeries::new("Egress", &[5.7, 4.8, 5.0, 5.3], ORANGE),
        NamedBarSeries::new("Logs", &[3.5, 2.4, 2.5, 2.6], RED),
    ];

    Ok(vec![
        LineChart::from_series(
            &[
                NamedSeries::new("p95", &p95, BLUE),
                NamedSeries::new("p99", &p99, RED),
            ],
            0.0..60.0,
            0.0..2000.0,
        )
        .labels(
            "HTTP tail latency during a bad deploy",
            "Minutes since 09:00 UTC",
            "Latency (ms)",
        )
        .size(SIZE)
        .sketch(style)
        .render()?,
        LineChart::from_series(
            &[
                NamedSeries::new("200 OK", &status_rates[0], GREEN),
                NamedSeries::new("404 Not Found", &status_rates[1], ORANGE),
                NamedSeries::new("500 Server Error", &status_rates[2], RED),
            ],
            0.0..60.0,
            0.0..100.0,
        )
        .labels(
            "HTTP status mix during the incident",
            "Minutes since 09:00 UTC",
            "Requests (%)",
        )
        .size(SIZE)
        .sketch(style)
        .render()?,
        BarChart::new(
            &[
                ("/auth", 85.),
                ("/search", 240.),
                ("/cart", 135.),
                ("/pay", 410.),
                ("/orders", 175.),
                ("/health", 12.),
            ],
            0.0..500.0,
        )
        .labels("API route latency", "Route", "p95 latency (ms)")
        .size(SIZE)
        .sketch(style)
        .render()?,
        BarChart::new(
            &[
                ("Compute", -3.2),
                ("Database", 1.4),
                ("Storage", 0.6),
                ("Egress", -0.9),
                ("Logs", -1.1),
            ],
            -4.0..2.0,
        )
        .labels(
            "Cloud cost change after optimization",
            "Service",
            "Change (kUSD)",
        )
        .size(SIZE)
        .sketch(style)
        .render()?,
        ScatterChart::new(&cpu, 0.0..100.0, 0.0..900.0)
            .labels(
                "API saturation: CPU versus latency",
                "CPU utilization (%)",
                "p95 latency (ms)",
            )
            .marker(5, true)
            .opacity(0.75)
            .size(SIZE)
            .sketch(style)
            .render()?,
        ScatterChart::from_series(
            &[
                NamedSeries::new("Before index", &before, RED),
                NamedSeries::new("After index", &after, GREEN),
            ],
            0.0..100.0,
            0.0..700.0,
        )
        .labels(
            "Database query latency after indexing",
            "Rows examined (thousands)",
            "Query time (ms)",
        )
        .marker(5, false)
        .size(SIZE)
        .sketch(style)
        .render()?,
        AreaChart::new(&traffic, 0.0..24.0, 0.0..12.0)
            .labels(
                "Daily edge request volume",
                "Hour of day (UTC)",
                "Requests/s (thousands)",
            )
            .size(SIZE)
            .sketch(style)
            .render()?,
        AreaChart::new(&queue, 0.0..60.0, 0.0..10000.0)
            .labels(
                "Retry queue buildup and recovery",
                "Minutes since 09:00 UTC",
                "Queued jobs",
            )
            .size(SIZE)
            .sketch(style)
            .render()?,
        PieChart::new(&[
            NamedSlice::new("Compute $21.6k / 45%", 21600., BLUE),
            NamedSlice::new("Database $12k / 25%", 12000., PURPLE),
            NamedSlice::new("Storage $7.2k / 15%", 7200., GREEN),
            NamedSlice::new("Egress $4.8k / 10%", 4800., ORANGE),
            NamedSlice::new("Logs $2.4k / 5%", 2400., RED),
        ])
        .title("Monthly cloud spend by service")
        .size(SIZE)
        .sketch(style)
        .render()?,
        PieChart::new(&[
            NamedSlice::new("N. America 5.4M / 45%", 5.4, BLUE),
            NamedSlice::new("Europe 3.6M / 30%", 3.6, PURPLE),
            NamedSlice::new("Asia Pacific 2.4M / 20%", 2.4, GREEN),
            NamedSlice::new("S. America 0.6M / 5%", 0.6, ORANGE),
        ])
        .title("Daily requests by region")
        .size(SIZE)
        .sketch(style)
        .render()?,
        PieChart::new(&[
            NamedSlice::new("Hit 1.68M / 84%", 1680000., GREEN),
            NamedSlice::new("Miss 220k / 11%", 220000., ORANGE),
            NamedSlice::new("Bypass 100k / 5%", 100000., PURPLE),
        ])
        .title("CDN cache effectiveness")
        .donut(0.58)
        .size(SIZE)
        .sketch(style)
        .render()?,
        PieChart::new(&[
            NamedSlice::new("Updated 180 / 75%", 180., GREEN),
            NamedSlice::new("Pending 36 / 15%", 36., BLUE),
            NamedSlice::new("Draining 18 / 7.5%", 18., ORANGE),
            NamedSlice::new("Failed 6 / 2.5%", 6., RED),
        ])
        .title("Rolling deployment progress")
        .donut(0.58)
        .size(SIZE)
        .sketch(style)
        .render()?,
        AreaChart::from_series(
            &[
                NamedSeries::new("Outbound", &outbound, BLUE),
                NamedSeries::new("Inbound", &inbound, ORANGE),
            ],
            0.0..24.0,
            0.0..10.0,
        )
        .labels(
            "Edge network throughput",
            "Hour of day (UTC)",
            "Throughput (Gbps)",
        )
        .opacity(0.3)
        .size(SIZE)
        .sketch(style)
        .render()?,
        AreaChart::from_series(&statuses, 0.0..60.0, 0.0..70.0)
            .stacked()
            .opacity(0.65)
            .labels(
                "HTTP request volume by status",
                "Minutes since 09:00 UTC",
                "Requests/min (thousands)",
            )
            .size(SIZE)
            .sketch(style)
            .render()?,
        AreaChart::from_series(&statuses, 0.0..60.0, 0.0..100.0)
            .percent_stacked()
            .opacity(0.65)
            .labels(
                "HTTP status share during the incident",
                "Minutes since 09:00 UTC",
                "Requests (%)",
            )
            .size(SIZE)
            .sketch(style)
            .render()?,
        BarChart::from_series(
            &["/auth", "/search", "/cart", "/pay", "/orders"],
            &[
                NamedBarSeries::new("Before tuning", &[110., 310., 180., 540., 225.], RED),
                NamedBarSeries::new("After tuning", &[85., 240., 135., 410., 175.], GREEN),
            ],
            0.0..600.0,
        )
        .labels(
            "API latency before and after tuning",
            "Route",
            "p95 latency (ms)",
        )
        .size(SIZE)
        .sketch(style)
        .render()?,
        BarChart::from_series(&["Aug", "Sep", "Oct", "Nov"], &costs, 0.0..60.0)
            .stacked()
            .labels("Monthly cloud spend by service", "Month", "Spend (kUSD)")
            .size(SIZE)
            .sketch(style)
            .render()?,
        BarChart::from_series(&["Aug", "Sep", "Oct", "Nov"], &costs, 0.0..100.0)
            .percent_stacked()
            .labels("Monthly cloud cost mix", "Month", "Spend (%)")
            .size(SIZE)
            .sketch(style)
            .render()?,
    ])
}

fn headings() -> Result<Scene> {
    let mut scene = Scene::new();
    {
        let root =
            ExcalidrawBackend::new(&mut scene, (3040, (TOP + NOTES.len() as i32 * ROW) as u32))?
                .into_drawing_area();
        root.draw(&Text::new(
            "Operations chart gallery",
            (LEFT, 30),
            ("Excalifont", 36),
        ))?;
        root.draw(&Text::new(
            format!(
                "{} realistic synthetic scenarios / 6 chart types / {} editable charts",
                NOTES.len(),
                NOTES.len() * 3
            ),
            (LEFT, 85),
            ("Excalifont", 22),
        ))?;
        root.draw(&Text::new(
            "Compare each row left to right. Identical data, scales and solid fills; only roughness changes.",
            (LEFT, 125), ("Excalifont", 20),
        ))?;
        for (column, label) in [
            "Roughness 0 / clean",
            "Roughness 1 / sketch",
            "Roughness 2 / extra sketch",
        ]
        .iter()
        .enumerate()
        {
            root.draw(&Text::new(
                *label,
                (LEFT + column as i32 * COLUMN, 185),
                ("Excalifont", 26),
            ))?;
        }
        for (row, note) in NOTES.iter().enumerate() {
            root.draw(&Text::new(
                *note,
                (LEFT, TOP + row as i32 * ROW - 28),
                ("Excalifont", 18),
            ))?;
        }
    }
    Ok(scene)
}

fn main() -> Result<()> {
    let mut overwrite = false;
    let mut destination = None;
    for arg in std::env::args_os().skip(1) {
        if arg == OsStr::new("--overwrite") {
            overwrite = true;
        } else if arg == OsStr::new("--help") {
            println!(
                "Usage: cargo run --locked --example operations_gallery -- [--overwrite] [destination.excalidraw]\nDefault: output/operations-gallery.excalidraw"
            );
            return Ok(());
        } else if arg.to_string_lossy().starts_with('-') || destination.is_some() {
            return Err("expected [--overwrite] [destination.excalidraw]".into());
        } else {
            destination = Some(PathBuf::from(arg));
        }
    }
    let destination = destination::resolve(destination, "operations-gallery.excalidraw")?;
    let mut document = headings()?;
    for roughness in 0..=2 {
        for (row, mut scene) in charts(SketchStyle::new(roughness, FillStyle::Solid)?)?
            .into_iter()
            .enumerate()
        {
            // This gallery's coordinates are offsets from each chart's original origin.
            scene.translate((
                f64::from(LEFT + i32::from(roughness) * COLUMN),
                f64::from(TOP + row as i32 * ROW),
            ))?;
            document.append(scene)?;
        }
    }
    document.write(
        &destination,
        if overwrite {
            Overwrite::Allow
        } else {
            Overwrite::Refuse
        },
    )?;
    println!(
        "Wrote {}: {} scenarios x 3 roughness levels, {} elements, {} bytes",
        destination.display(),
        NOTES.len(),
        document.diagnostics().elements.values().sum::<usize>(),
        document.to_bytes()?.len()
    );
    Ok(())
}
