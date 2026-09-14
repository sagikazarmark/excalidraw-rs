use excaliplot::{NamedSlice, PieChart};
use serde_json::{Value, json};

fn elements(scene: &excaliplot::Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn clean_pie_and_donut_arc_vertices_stay_on_the_circle_without_pixel_rounding() {
    for hole in [0.0, 0.5] {
        let scene = PieChart::new(&[
            NamedSlice::new("Major", 75.0, (25, 113, 194)),
            NamedSlice::new("Narrow", 1.0, (230, 119, 0)),
            NamedSlice::new("Rest", 24.0, (47, 158, 68)),
        ])
        .donut(hole)
        .render()
        .unwrap();
        for fill in elements(&scene).iter().filter(|e| e["type"] == "line") {
            assert_eq!(fill["roughness"], 0);
            for point in fill["points"].as_array().unwrap() {
                let x = fill["x"].as_f64().unwrap() + point[0].as_f64().unwrap();
                let y = fill["y"].as_f64().unwrap() + point[1].as_f64().unwrap();
                let radius = (x - 213.0).hypot(y - 220.0);
                if hole == 0.0 && radius < 1e-9 {
                    continue;
                }
                let error = (radius - 144.0).abs().min(if hole > 0.0 {
                    (radius - 72.0).abs()
                } else {
                    f64::INFINITY
                });
                assert!(
                    error < 1e-9,
                    "arc vertex is off-circle by {error} scene units"
                );
            }
        }
        assert!(scene.diagnostics().vertices <= if hole == 0.0 { 190 } else { 375 });
    }
}

#[test]
fn pie_rejects_invalid_totals_unresolvable_wedges_and_crowded_labels_without_panics() {
    for values in [
        vec![],
        vec![0.0, 1.0],
        vec![-1.0, 2.0],
        vec![f64::NAN, 1.0],
        vec![f64::INFINITY, 1.0],
        vec![f64::MAX, f64::MAX],
        vec![1e-300, 1.0],
    ] {
        let slices: Vec<_> = values
            .iter()
            .map(|v| NamedSlice::new("A", *v, (0, 0, 255)))
            .collect();
        assert!(PieChart::new(&slices).render().is_err(), "{values:?}");
    }
    let slices = [
        NamedSlice::new("A", 1.0, (0, 0, 255)),
        NamedSlice::new("B", 1.0, (255, 0, 0)),
    ];
    for size in [(0, 0), (100, 100), (u32::MAX, 400)] {
        assert!(PieChart::new(&slices).size(size).render().is_err());
    }
    for hole in [f64::NAN, -0.1, 1.0, 0.9999, 0.000001] {
        assert!(PieChart::new(&slices).donut(hole).render().is_err());
    }
    for name in ["", " ", "漢", &"W".repeat(100)] {
        assert!(
            PieChart::new(&[NamedSlice::new(name, 1.0, (0, 0, 255))])
                .render()
                .is_err()
        );
    }
    assert!(
        PieChart::new(&slices)
            .title(&"W".repeat(100))
            .render()
            .is_err()
    );
    assert!(
        PieChart::new(&vec![slices[0].clone(); 20])
            .render()
            .is_err()
    );
    assert!(PieChart::new(&[slices[0].clone()]).render().is_ok());
    assert!(
        PieChart::new(&[slices[0].clone()])
            .donut(0.5)
            .render()
            .is_err()
    );
}

#[test]
fn donut_arcs_share_exact_radial_edges_and_leave_the_center_empty() {
    let scene = PieChart::new(&[
        NamedSlice::new("Major", 75.0, (255, 0, 0)),
        NamedSlice::new("Narrow", 1.0, (0, 0, 255)),
        NamedSlice::new("Rest", 24.0, (0, 128, 0)),
    ])
    .donut(0.5)
    .render()
    .unwrap();
    let elements = elements(&scene);
    let fills: Vec<_> = elements.iter().filter(|e| e["type"] == "line").collect();
    assert_eq!(fills.len(), 3);
    let world = |e: &Value| -> Vec<(f64, f64)> {
        e["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    e["x"].as_f64().unwrap() + p[0].as_f64().unwrap(),
                    e["y"].as_f64().unwrap() + p[1].as_f64().unwrap(),
                )
            })
            .collect()
    };
    let major = world(fills[0]);
    assert_eq!(major[0], (357.0, 220.0));
    assert_eq!(major[major.len() - 2], (285.0, 220.0));
    assert!(
        major
            .windows(2)
            .any(|p| p == [(213.0, 76.0), (213.0, 148.0)])
    );
    assert_eq!(world(fills[1])[0], (213.0, 76.0));
    for fill in fills {
        let points = world(fill);
        assert_eq!(points[0], *points.last().unwrap());
        assert!(points.windows(2).all(|p| p[0] != p[1]));
        for (x, y) in &points {
            let r = (x - 213.0).hypot(y - 220.0);
            assert!((r - 72.0).abs() <= 0.71 || (r - 144.0).abs() <= 0.71);
        }
        // Independent ray-casting oracle: no wedge may cover the donut center.
        let crossings = points
            .windows(2)
            .filter(|p| {
                let [(x1, y1), (x2, y2)] = [p[0], p[1]];
                (y1 > 220.0) != (y2 > 220.0) && 213.0 < (x2 - x1) * (220.0 - y1) / (y2 - y1) + x1
            })
            .count();
        assert_eq!(crossings % 2, 0);
    }
    assert!(scene.diagnostics().vertices <= 375);
}

#[test]
fn single_slice_is_a_disk_without_a_radial_cut_or_center_vertex() {
    let scene = PieChart::new(&[NamedSlice::new("All", 1.0, (0, 0, 255))])
        .render()
        .unwrap();
    let elements = elements(&scene);
    let fill = elements.iter().find(|e| e["type"] == "line").unwrap();
    for p in fill["points"].as_array().unwrap() {
        let x = fill["x"].as_f64().unwrap() + p[0].as_f64().unwrap();
        let y = fill["y"].as_f64().unwrap() + p[1].as_f64().unwrap();
        assert!(((x - 213.0).hypot(y - 220.0) - 144.0).abs() <= 0.71);
    }
}

#[test]
fn tall_multirow_pie_keeps_wedges_clear_of_its_legend() {
    let scene = PieChart::new(&vec![NamedSlice::new("A", 1.0, (0, 0, 255)); 14])
        .size((1200, 900))
        .render()
        .unwrap();
    let elements = elements(&scene);
    // Legend's independently specified left edge is 3/5 of the canvas (720).
    // Reserve at least 16 scene units for reliable selection of swatches.
    for fill in elements.iter().filter(|e| e["type"] == "line") {
        for point in fill["points"].as_array().unwrap() {
            assert!(fill["x"].as_f64().unwrap() + point[0].as_f64().unwrap() <= 704.0);
        }
    }
}

#[test]
fn pie_uses_root_coordinates_and_quarter_wedges_with_grouped_native_labels() {
    let scene = PieChart::new(&[
        NamedSlice::new("Half", 2.0, (255, 0, 0)),
        NamedSlice::new("Quarter", 1.0, (0, 0, 255)),
        NamedSlice::new("Rest", 1.0, (0, 128, 0)),
    ])
    .render()
    .unwrap();
    let elements = elements(&scene);
    let fills: Vec<_> = elements.iter().filter(|e| e["type"] == "line").collect();
    assert_eq!(fills.len(), 3);
    // Fixed 640x400 root layout: center (213,220), radius 144, clockwise from 3 o'clock.
    let half = fills[0];
    assert_eq!(half["x"], 213.0);
    assert_eq!(half["y"], 220.0);
    assert_eq!(half["width"], 288.0);
    assert_eq!(half["height"], 144.0);
    let points = half["points"].as_array().unwrap();
    assert_eq!(points[0], json!([0.0, 0.0]));
    assert_eq!(points[1], json!([144.0, 0.0]));
    assert!(points.contains(&json!([0.0, 144.0])));
    assert_eq!(points[points.len() - 2], json!([-144.0, 0.0]));
    assert_eq!(points.last().unwrap(), &points[0]);
    for (fill, name) in fills.iter().zip(["Half", "Quarter", "Rest"]) {
        assert_eq!(fill["strokeColor"], "transparent");
        let label = elements.iter().find(|e| e["text"] == name).unwrap();
        assert_eq!(label["groupIds"], fill["groupIds"]);
        assert_eq!(label["x"], 409.0);
        assert_eq!(fill["groupIds"].as_array().unwrap().len(), 2);
    }
    assert_eq!(scene.diagnostics().calls["fill_polygon_fractional"], 3);
    assert_eq!(scene.diagnostics().calls.get("draw_pixel"), None);
    assert_eq!(scene.diagnostics().calls.get("blit_bitmap"), None);
    assert!(scene.diagnostics().vertices <= 190);
}
