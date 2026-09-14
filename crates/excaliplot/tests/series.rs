use excaliplot::{FillStyle, LineChart, NamedSeries, SketchStyle};
use serde_json::Value;

#[test]
fn named_series_and_delayed_legend_share_explicit_nested_groups_even_with_equal_colors() {
    let first = [(1.0, 2.0), (4.0, 3.0), (9.0, 8.0)];
    let second = [(1.0, 8.0), (4.0, 5.0), (9.0, 2.0)];
    let scene = LineChart::from_series(
        &[
            NamedSeries::new("Alpha", &first, (25, 113, 194)),
            NamedSeries::new("Beta", &second, (25, 113, 194)),
        ],
        0.0..10.0,
        0.0..10.0,
    )
    .labels("Two series", "Time", "Value")
    .sketch(SketchStyle::new(1, FillStyle::Hachure).unwrap())
    .render()
    .unwrap();
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let elements = doc["elements"].as_array().unwrap();
    let outer = &elements[0]["groupIds"][0];
    assert!(outer.is_string());
    assert!(
        elements
            .iter()
            .all(|e| e["groupIds"].as_array().unwrap().last() == Some(outer))
    );
    let paths: Vec<_> = elements
        .iter()
        .filter(|e| e["points"].as_array().is_some_and(|p| p.len() == 3))
        .collect();
    assert_eq!(paths.len(), 2);
    assert_ne!(paths[0]["groupIds"][0], paths[1]["groupIds"][0]);
    for (path, name) in paths.iter().zip(["Alpha", "Beta"]) {
        assert_eq!(path["roughness"], 1);
        assert_eq!(path["strokeColor"], "#1971c2");
        let members: Vec<_> = elements
            .iter()
            .filter(|e| e["groupIds"] == path["groupIds"])
            .collect();
        assert_eq!(members.len(), 3, "series path, legend swatch, legend label");
        assert!(members.iter().any(|e| e["text"] == name));
        assert!(members.iter().all(|e| e["strokeColor"] == "#1971c2"));
    }
    assert_eq!(
        elements[0]["fillStyle"], "solid",
        "finite white background stays solid"
    );
    assert_eq!(elements[0]["roughness"], 0);
    assert!(elements.len() <= 80);
}

#[test]
fn sketch_variants_preserve_geometry_seeds_and_source_paints_with_fresh_group_ids() {
    let points = [(1.0, 2.0), (4.0, 3.0), (9.0, 8.0)];
    let render = |roughness| {
        let scene = LineChart::from_series(
            &[NamedSeries::new("Alpha", &points, (25, 113, 194))],
            0.0..10.0,
            0.0..10.0,
        )
        .sketch(SketchStyle::new(roughness, FillStyle::Solid).unwrap())
        .render()
        .unwrap();
        serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()
    };
    let clean = render(0);
    for roughness in [1, 2] {
        let sketch = render(roughness);
        for (a, b) in clean["elements"]
            .as_array()
            .unwrap()
            .iter()
            .zip(sketch["elements"].as_array().unwrap())
        {
            assert_ne!(a["id"], b["id"]);
            assert_ne!(a["groupIds"], b["groupIds"]);
            for field in [
                "x",
                "y",
                "width",
                "height",
                "angle",
                "points",
                "text",
                "strokeColor",
                "strokeWidth",
                "seed",
            ] {
                assert_eq!(a[field], b[field], "{field}");
            }
            assert!(b["seed"].as_u64().unwrap() > 0);
        }
    }
}

#[test]
fn invalid_named_series_and_overflowing_legends_fail_as_complete_chart_errors() {
    let points = [(1.0, 2.0), (4.0, 3.0)];
    assert!(
        LineChart::from_series(&[], 0.0..10.0, 0.0..10.0)
            .render()
            .is_err()
    );
    for series in [
        NamedSeries::new("", &points, (0, 0, 0)),
        NamedSeries::new("Ω", &points, (0, 0, 0)),
        NamedSeries::new("Missing", &[], (0, 0, 0)),
        NamedSeries::new("Outside", &[(11.0, 2.0), (12.0, 3.0)], (0, 0, 0)),
    ] {
        assert!(
            LineChart::from_series(&[series], 0.0..10.0, 0.0..10.0)
                .render()
                .is_err()
        );
    }
    let long = "W".repeat(80);
    assert!(
        LineChart::from_series(
            &[NamedSeries::new(&long, &points, (0, 0, 0))],
            0.0..10.0,
            0.0..10.0
        )
        .render()
        .is_err()
    );
    assert!(
        LineChart::from_series(
            &vec![NamedSeries::new("Many", &points, (0, 0, 0)); 12],
            0.0..10.0,
            0.0..10.0
        )
        .render()
        .is_err()
    );
}

#[test]
fn title_must_fit_the_caption_area_after_reserving_legend_width() {
    let points = [(1.0, 2.0), (9.0, 8.0)];
    let title = "W".repeat(27);
    let series = [
        NamedSeries::new("Measured", &points, (25, 113, 194)),
        NamedSeries::new("Reference", &points, (230, 119, 0)),
    ];
    assert!(
        LineChart::from_series(&series, 0.0..10.0, 0.0..10.0)
            .labels(&title, "X", "Y")
            .render()
            .is_err()
    );
    assert!(
        LineChart::from_series(&series, 0.0..10.0, 0.0..10.0)
            .size((800, 440))
            .labels(&title, "X", "Y")
            .render()
            .is_ok()
    );
}
