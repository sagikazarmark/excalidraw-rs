use excaliplot::{
    AreaChart, BarChart, LineChart, NamedBarSeries, NamedSeries, ScatterChart, Scene,
};
use serde_json::Value;

#[test]
fn automatic_range_demo_composes_three_comparisons_and_protects_edits() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ranges.excalidraw");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args(["run", "--locked", "--example", "auto_ranges", "--"])
            .arg(&path)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let doc: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let elements = doc["elements"].as_array().unwrap();
    for title in [
        "Automatic irregular X",
        "Automatic constant Y",
        "Explicit comparison",
    ] {
        assert!(elements.iter().any(|e| e["text"] == title));
    }
    let lines: Vec<_> = elements
        .iter()
        .filter(|e| e["type"] == "line" && e["strokeColor"] == "#1971c2")
        .collect();
    assert_eq!(lines.len(), 3);
    assert!(
        lines
            .iter()
            .all(|e| e["points"].as_array().unwrap().len() == 6)
    );
    assert_ne!(lines[0]["groupIds"], lines[1]["groupIds"]);
    assert_ne!(lines[1]["groupIds"], lines[2]["groupIds"]);
    std::fs::write(&path, "manual edits").unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&path).unwrap(), b"manual edits");
}

// Public helper -> serialized native geometry/text. Identity and timestamps are
// intentionally excluded: independent renders allocate independent scenes.
fn content(scene: Scene) -> Vec<Value> {
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    doc["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let mut e = e.clone();
            for key in ["id", "seed", "versionNonce", "updated", "groupIds"] {
                e.as_object_mut().unwrap().remove(key);
            }
            e
        })
        .collect()
}

#[test]
fn domain_edges_limit_padding_without_clipping_and_keep_useful_fit_errors() {
    for (points, x) in [
        (vec![(1e9, 5.)], 950_000_000.0..1e9),
        (vec![(-1e9, 5.)], -1e9..-950_000_000.0),
        (vec![(-1e9, 5.), (1e9, 5.)], -1e9..1e9),
        (vec![(0., 5.), (1e-9, 5.)], -1e-6..0.000001001),
    ] {
        let auto = ScatterChart::auto(&points).size((16384, 400)).render();
        let explicit = ScatterChart::new(&points, x, 4.0..6.0)
            .size((16384, 400))
            .render();
        match (auto, explicit) {
            (Ok(auto), Ok(explicit)) => assert_eq!(content(auto), content(explicit)),
            (Err(auto), Err(explicit)) => {
                assert_eq!(auto.to_string(), explicit.to_string());
                assert!(auto.to_string().contains("tick label"), "{auto}");
            }
            _ => panic!("automatic domain-edge range must match the worked explicit range"),
        }
    }
    for points in [vec![(1e9 + 1., 5.)], vec![(0., -1e9 - 1.)]] {
        let error = ScatterChart::auto(&points).render().err().unwrap();
        assert!(error.to_string().contains("within ±1e9"), "{error}");
    }
    assert_eq!(
        content(ScatterChart::auto(&[(0., 1e9)]).render().unwrap()),
        content(
            ScatterChart::new(&[(0., 1e9)], -1.0..1.0, 950_000_000.0..1e9)
                .render()
                .unwrap()
        )
    );
    let series = [
        NamedBarSeries::new("A", &[6e8], (25, 113, 194)),
        NamedBarSeries::new("B", &[6e8], (224, 49, 49)),
    ];
    let error = BarChart::auto_from_series(&["A"], &series)
        .stacked()
        .render()
        .err()
        .unwrap();
    assert!(error.to_string().contains("within ±1e9"), "{error}");
}

#[test]
fn automatic_ranges_retain_input_validation_and_resolution_rules() {
    for points in [
        vec![],
        vec![(0., 1.)],
        vec![(0., 1.); 3],
        vec![(2., 1.), (1., 2.)],
        vec![(0., f64::NAN), (1., 2.)],
    ] {
        assert!(LineChart::auto(&points).render().is_err());
    }
    for points in [
        vec![],
        vec![(f64::INFINITY, 0.)],
        vec![(0., f64::NEG_INFINITY)],
    ] {
        assert!(ScatterChart::auto(&points).render().is_err());
    }
    for points in [
        vec![],
        vec![(0., 1.)],
        vec![(0., 0.), (1., 0.)],
        vec![(0., -1.), (1., 1.)],
        vec![(1., 1.), (0., 2.)],
        vec![(0., 1.), (0., 2.)],
    ] {
        assert!(AreaChart::auto(&points).render().is_err());
    }
    assert!(LineChart::auto_from_series(&[]).render().is_err());
    assert!(ScatterChart::auto_from_series(&[]).render().is_err());
    assert!(AreaChart::auto_from_series(&[]).stacked().render().is_err());
    assert!(BarChart::auto(&[]).render().is_err());
    assert!(
        BarChart::auto_from_series(&[], &[])
            .stacked()
            .render()
            .is_err()
    );
    assert!(
        AreaChart::auto(&[(0., 2.), (1., 3.)])
            .baseline(f64::NAN)
            .render()
            .is_err()
    );
    assert!(
        AreaChart::auto(&[(0., 2.), (1., 3.)])
            .baseline(1.)
            .stacked()
            .render()
            .is_err()
    );
    assert!(BarChart::auto(&[("A", -1.)]).stacked().render().is_err());
    assert!(
        BarChart::auto(&[("A", 0.)])
            .percent_stacked()
            .render()
            .is_err()
    );
    let misaligned = [NamedBarSeries::new("A", &[1.], (25, 113, 194))];
    assert!(
        BarChart::auto_from_series(&["A", "B"], &misaligned)
            .render()
            .is_err()
    );
    let a = [(0., 1.), (1., 2.)];
    let b = [(0., 1.), (2., 2.)];
    let series = [
        NamedSeries::new("A", &a, (25, 113, 194)),
        NamedSeries::new("B", &b, (224, 49, 49)),
    ];
    assert!(
        AreaChart::auto_from_series(&series)
            .stacked()
            .render()
            .is_err()
    );
    let overflow = [
        NamedBarSeries::new("A", &[f64::MAX], (25, 113, 194)),
        NamedBarSeries::new("B", &[f64::MAX], (224, 49, 49)),
    ];
    assert!(
        BarChart::auto_from_series(&["A"], &overflow)
            .percent_stacked()
            .render()
            .is_err()
    );
    let tiny = [(0., 1e-12), (1e-12, 1e-12)];
    let ordinary = [(0., 1.), (10., 10.)];
    let series = [
        NamedSeries::new("A", &tiny, (25, 113, 194)),
        NamedSeries::new("B", &ordinary, (224, 49, 49)),
    ];
    assert!(
        LineChart::auto_from_series(&series)
            .render()
            .err()
            .unwrap()
            .to_string()
            .contains("collapses")
    );
    assert!(
        BarChart::auto(&[("A", 1e-12), ("B", 10.)])
            .render()
            .err()
            .unwrap()
            .to_string()
            .contains("collapses")
    );
    assert!(
        AreaChart::auto_from_series(&series)
            .render()
            .err()
            .unwrap()
            .to_string()
            .contains("collapses")
    );
}

#[test]
fn automatic_stacks_contain_totals_and_percent_endpoints() {
    let a = [(0., 10.), (10., 20.)];
    let b = [(0., 30.), (10., 20.)];
    let series = [
        NamedSeries::new("A", &a, (25, 113, 194)),
        NamedSeries::new("B", &b, (224, 49, 49)),
    ];
    assert_eq!(
        content(
            AreaChart::auto_from_series(&series)
                .stacked()
                .render()
                .unwrap()
        ),
        content(
            AreaChart::from_series(&series, -0.5..10.5, -2.0..42.0)
                .stacked()
                .render()
                .unwrap()
        )
    );
    assert_eq!(
        content(
            AreaChart::auto_from_series(&series)
                .percent_stacked()
                .render()
                .unwrap()
        ),
        content(
            AreaChart::from_series(&series, -0.5..10.5, -5.0..105.0)
                .percent_stacked()
                .render()
                .unwrap()
        )
    );
    let bars = [
        NamedBarSeries::new("A", &[10., 20.], (25, 113, 194)),
        NamedBarSeries::new("B", &[30., 20.], (224, 49, 49)),
    ];
    assert_eq!(
        content(
            BarChart::auto_from_series(&["A", "B"], &bars)
                .stacked()
                .render()
                .unwrap()
        ),
        content(
            BarChart::from_series(&["A", "B"], &bars, -2.0..42.0)
                .stacked()
                .render()
                .unwrap()
        )
    );
    assert_eq!(
        content(
            BarChart::auto_from_series(&["A", "B"], &bars)
                .percent_stacked()
                .render()
                .unwrap()
        ),
        content(
            BarChart::from_series(&["A", "B"], &bars, -5.0..105.0)
                .percent_stacked()
                .render()
                .unwrap()
        )
    );
    // Percent input is raw counts, not axis coordinates.
    assert_eq!(
        content(
            BarChart::auto(&[("A", 1e100)])
                .percent_stacked()
                .render()
                .unwrap()
        ),
        content(
            BarChart::new(&[("A", 1e100)], -5.0..105.0)
                .percent_stacked()
                .render()
                .unwrap()
        )
    );
}

#[test]
fn constants_and_unordered_repeated_scatter_keep_their_observations() {
    for (points, x, y) in [
        (vec![(2., 5.)], 1.0..3.0, 4.0..6.0),
        (vec![(0., 100.); 3], -1.0..1.0, 95.0..105.0),
        (
            vec![(10., -5.), (-10., -5.), (-10., -5.)],
            -11.0..11.0,
            -6.0..-4.0,
        ),
    ] {
        assert_eq!(
            content(ScatterChart::auto(&points).render().unwrap()),
            content(ScatterChart::new(&points, x, y).render().unwrap())
        );
    }
    for (points, x, y) in [
        ([(0., 5.), (10., 5.)], -0.5..10.5, 4.0..6.0),
        ([(5., -10.), (5., 10.)], 4.0..6.0, -11.0..11.0),
    ] {
        assert_eq!(
            content(LineChart::auto(&points).render().unwrap()),
            content(LineChart::new(&points, x, y).render().unwrap())
        );
    }
    let a = [(0., 0.)];
    let b = [(20., 20.)];
    let series = [
        NamedSeries::new("A", &a, (25, 113, 194)),
        NamedSeries::new("B", &b, (224, 49, 49)),
    ];
    assert_eq!(
        content(ScatterChart::auto_from_series(&series).render().unwrap()),
        content(
            ScatterChart::from_series(&series, -1.0..21.0, -1.0..21.0)
                .render()
                .unwrap()
        )
    );
    // Padding is data-space convenience, not a guarantee of marker clearance.
    let error = ScatterChart::auto(&[(0., 0.), (10., 10.)])
        .tick_density(3, 3)
        .marker(20, false)
        .size((400, 300))
        .render()
        .err()
        .unwrap();
    assert!(error.to_string().contains("marker crowds"), "{error}");
}

#[test]
fn automatic_lines_contain_all_series_and_preserve_irregular_x_and_signed_values() {
    let a = [(0., -10.), (2., 0.), (20., 10.)];
    let b = [(-20., 5.), (10., 30.), (20., 5.)];
    let series = [
        NamedSeries::new("A", &a, (25, 113, 194)),
        NamedSeries::new("B", &b, (224, 49, 49)),
    ];
    let auto = content(LineChart::auto_from_series(&series).render().unwrap());
    // Worked bounds: X extrema -20..20, Y extrema -10..30, 5% = 2 each side.
    let explicit = content(
        LineChart::from_series(&series, -22.0..22.0, -12.0..32.0)
            .render()
            .unwrap(),
    );
    assert_eq!(auto, explicit);
    let path = auto
        .iter()
        .find(|e| {
            e["type"] == "line"
                && e["strokeColor"] == "#1971c2"
                && e["points"].as_array().unwrap().len() == 3
        })
        .unwrap();
    let points = path["points"].as_array().unwrap();
    assert!(points[2][0].as_f64().unwrap() > 9. * points[1][0].as_f64().unwrap());
    assert!(LineChart::new(&a, 0.0..19.0, -10.0..10.0).render().is_err());
}

#[test]
fn automatic_fills_include_configured_baselines_and_signed_bars() {
    let points = [(0., 5.), (10., 10.)];
    assert_eq!(
        content(AreaChart::auto(&points).baseline(-10.).render().unwrap()),
        content(
            AreaChart::new(&points, -0.5..10.5, -11.0..11.0)
                .baseline(-10.)
                .render()
                .unwrap()
        )
    );
    let below = [(0., -10.), (10., -5.)];
    let series = [
        NamedSeries::new("A", &points, (25, 113, 194)),
        NamedSeries::new("B", &below, (224, 49, 49)),
    ];
    assert_eq!(
        content(AreaChart::auto_from_series(&series).render().unwrap()),
        content(
            AreaChart::from_series(&series, -0.5..10.5, -11.0..11.0)
                .render()
                .unwrap()
        )
    );
    for (data, bounds) in [
        ([("A", -10.), ("B", 10.)], -11.0..11.0),
        ([("A", 10.), ("B", 20.)], -1.0..21.0),
        ([("A", -20.), ("B", -10.)], -21.0..1.0),
        ([("A", 0.), ("B", 0.)], -1.0..1.0),
    ] {
        assert_eq!(
            content(BarChart::auto(&data).render().unwrap()),
            content(BarChart::new(&data, bounds).render().unwrap())
        );
    }
    let bars = [
        NamedBarSeries::new("A", &[-10., 0.], (25, 113, 194)),
        NamedBarSeries::new("B", &[10., 20.], (224, 49, 49)),
    ];
    assert_eq!(
        content(
            BarChart::auto_from_series(&["A", "B"], &bars)
                .render()
                .unwrap()
        ),
        content(
            BarChart::from_series(&["A", "B"], &bars, -11.5..21.5)
                .render()
                .unwrap()
        )
    );
}
