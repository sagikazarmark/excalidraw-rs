use excaliplot::AreaChart;
use serde_json::{Value, json};

#[test]
fn area_rejects_ambiguous_or_collapsed_fills_and_out_of_range_baselines() {
    for points in [
        vec![],
        vec![(1.0, 2.0)],
        vec![(1.0, 2.0), (1.0, 3.0)],
        vec![(2.0, 2.0), (1.0, 3.0)],
        vec![(1.0, f64::NAN), (2.0, 3.0)],
        vec![(1.0, 2.0), (11.0, 3.0)],
        vec![(1.0, 0.0), (2.0, 0.0)],
        vec![(1.0, 1e-8), (2.0, 1e-8)],
        vec![(1.0, 2.0), (1.0000001, 3.0)],
    ] {
        assert!(
            AreaChart::new(&points, 0.0..10.0, 0.0..10.0)
                .render()
                .is_err(),
            "{points:?}"
        );
    }
    for baseline in [f64::NAN, -1.0, 11.0, 3.0] {
        assert!(
            AreaChart::new(&[(1.0, 2.0), (2.0, 4.0)], 0.0..10.0, 0.0..10.0)
                .baseline(baseline)
                .render()
                .is_err()
        );
    }
    for (points, baseline) in [
        (vec![(1.0, 2.0), (1.0, 2.0), (2.0, 2.0)], 0.0),
        (vec![(1.0, 2.0), (2.0, 4.0)], 5.0),
    ] {
        assert!(
            AreaChart::new(&points, 0.0..10.0, 0.0..10.0)
                .baseline(baseline)
                .render()
                .is_ok()
        );
    }
}

#[test]
fn area_has_a_closed_fill_and_only_the_data_edge_is_outlined() {
    let scene = AreaChart::new(&[(0.0, 2.0), (5.0, 8.0), (10.0, 4.0)], 0.0..10.0, 0.0..10.0)
        .render()
        .unwrap();
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let elements = doc["elements"].as_array().unwrap();
    let fill = elements
        .iter()
        .find(|e| e["backgroundColor"] == "#1971c2")
        .unwrap();
    let border = elements
        .iter()
        .find(|e| e["strokeColor"] == "#1971c2")
        .unwrap();
    assert_eq!(fill["type"], "line");
    assert_eq!(fill["strokeColor"], "transparent");
    assert_eq!(fill["opacity"], 35);
    assert_eq!(fill["x"], 112.0);
    // Existing known-data layout: plot corners (112,311)..(615,64).
    assert_eq!(fill["y"], 262.0);
    assert_eq!(fill["width"], 503.0);
    assert_eq!(fill["height"], 197.0);
    assert_eq!(
        fill["points"],
        json!([
            [0.0, 0.0],
            [251.0, -148.0],
            [503.0, -49.0],
            [503.0, 49.0],
            [0.0, 49.0],
            [0.0, 0.0]
        ])
    );
    assert_eq!(
        border["points"],
        json!([[0.0, 0.0], [251.0, -148.0], [503.0, -49.0]])
    );
    assert_eq!(border["backgroundColor"], "transparent");
    assert_eq!(border["opacity"], 100);
    assert_eq!(fill["groupIds"], border["groupIds"]);
    assert_eq!(scene.diagnostics().calls["fill_polygon"], 1);
    assert_eq!(scene.diagnostics().calls.get("draw_pixel"), None);
    assert_eq!(scene.diagnostics().calls.get("blit_bitmap"), None);
}
