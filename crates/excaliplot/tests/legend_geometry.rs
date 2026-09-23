//! The drawn legend geometry, which nothing else pins.
//!
//! The chart suite asserts absolute coordinates in 220 places, but none of them
//! is a legend row. Shifting `LEGEND_TOP` from 90 to 82 — the consolidation
//! `cartesian.rs` warns against by name, and which moves every legend row in
//! both the numeric charts and the pie — passed the whole suite in silence.
//! These tests are the guard that was missing.

use excaliplot::{LegendPosition, LineChart, NamedSeries, NamedSlice, PieChart, Scene};
use serde_json::Value;

fn elements(scene: &Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

/// Y of each legend label, in draw order.
fn legend_rows(scene: &Scene, names: &[&str]) -> Vec<f64> {
    let items = elements(scene);
    names
        .iter()
        .map(|name| {
            items
                .iter()
                .find(|e| e["type"] == "text" && e["text"] == *name)
                .unwrap_or_else(|| panic!("no legend label {name}"))["y"]
                .as_f64()
                .unwrap()
        })
        .collect()
}

#[test]
fn the_numeric_legend_places_its_first_row_at_ninety_and_pitches_by_thirty() {
    let points = [(0.0, 0.0), (1.0, 1.0)];
    let series = [
        NamedSeries::new("alpha", &points, (25, 113, 194)),
        NamedSeries::new("beta", &points, (200, 30, 30)),
    ];
    let scene = LineChart::from_series(&series, 0.0..1.0, 0.0..1.0)
        .render()
        .unwrap();
    assert_eq!(legend_rows(&scene, &["alpha", "beta"]), vec![90.0, 120.0]);
}

#[test]
fn the_pie_legend_shares_the_numeric_origin_and_pitch() {
    // `cartesian.rs` records that the pie and the numeric charts place a row
    // identically from the same two numbers. That shared placement is what
    // makes them safe to share; pin it so a change to one is a change to both.
    let slices = [
        NamedSlice::new("alpha", 1.0, (25, 113, 194)),
        NamedSlice::new("beta", 2.0, (200, 30, 30)),
    ];
    let scene = PieChart::new(&slices).render().unwrap();
    assert_eq!(legend_rows(&scene, &["alpha", "beta"]), vec![90.0, 120.0]);
}

#[test]
fn the_numeric_and_pie_legend_budgets_are_deliberately_different() {
    // Two rules, not two spellings of one: the numeric charts reserve the X
    // label area below the legend and the pie, which has no X labels, reserves
    // only the margin. At the default 400-unit height that is 246 against 286,
    // so the pie fits exactly one more row. Swapping the two budgets — the
    // obvious "consolidation" — changes both of these counts.
    let points = [(0.0, 0.0), (1.0, 1.0)];
    let names: Vec<String> = (0..12).map(|i| format!("series {i}")).collect();
    let fits_chart = |n: usize| {
        let series: Vec<NamedSeries> = (0..n)
            .map(|i| NamedSeries::new(&names[i], &points, (25, 113, 194)))
            .collect();
        LineChart::from_series(&series, 0.0..1.0, 0.0..1.0)
            .render()
            .is_ok()
    };
    let fits_pie = |n: usize| {
        let slices: Vec<NamedSlice> = (0..n)
            .map(|i| NamedSlice::new(&names[i], 1.0, (25, 113, 194)))
            .collect();
        PieChart::new(&slices).render().is_ok()
    };

    assert!(
        fits_chart(8) && !fits_chart(9),
        "numeric legend fits eight rows"
    );
    assert!(fits_pie(9) && !fits_pie(10), "pie legend fits nine rows");
}

#[test]
fn the_band_and_interval_legends_keep_their_own_eight_unit_offset() {
    // `cartesian.rs` records this divergence as load-bearing: these two draw a
    // single row at 82 rather than 90. Unifying the two origins is the change
    // the comment forbids, so pin the number it protects.
    let scene = excaliplot::ErrorBarChart::new(
        &[excaliplot::VerticalInterval::new(2., 2., 3., 8.)],
        "Supplied range",
        0.0..10.0,
        0.0..10.0,
    )
    .legend(LegendPosition::Right)
    .render()
    .unwrap();
    let label = elements(&scene)
        .into_iter()
        .find(|e| e["text"] == "Supplied range")
        .expect("interval legend label");
    // The schematic's top is the literal 82, and the label sits one stem gap
    // below it — 13 units for the default four-unit filled marker on a
    // two-unit stroke. Raising the origin to the shared 90 moves this to 103.
    assert_eq!(label["y"].as_f64().unwrap(), 95.0);
}
