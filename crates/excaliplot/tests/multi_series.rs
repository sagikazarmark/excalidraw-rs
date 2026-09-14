use excaliplot::{AreaChart, BarChart, FillStyle, NamedBarSeries, NamedSeries, Scene, SketchStyle};
use serde_json::Value;

const BLUE: (u8, u8, u8) = (25, 113, 194);
const RED: (u8, u8, u8) = (224, 49, 49);

fn elements(scene: Scene) -> Vec<Value> {
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    doc["elements"].as_array().unwrap().clone()
}

fn vertices(element: &Value) -> Vec<(f64, f64)> {
    element["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                element["x"].as_f64().unwrap() + p[0].as_f64().unwrap(),
                element["y"].as_f64().unwrap() + p[1].as_f64().unwrap(),
            )
        })
        .collect()
}

fn number(element: &Value, key: &str) -> f64 {
    element[key].as_f64().unwrap()
}

fn assert_series_group(elements: &[Value], name: &str, marks: &[&Value]) {
    let label = elements.iter().find(|e| e["text"] == name).unwrap();
    let groups = label["groupIds"].as_array().unwrap();
    assert_eq!(groups.len(), 2);
    assert!(marks.iter().all(|e| e["groupIds"] == label["groupIds"]));
    assert!(elements.iter().any(|e| e["type"] == "rectangle"
        && e["groupIds"] == label["groupIds"]
        && e["height"] == 14.0));
}

#[test]
fn overlapping_areas_keep_independent_samples_opacity_and_legend_groups() {
    let data = [
        NamedSeries::new("Inbound", &[(0., 2.), (4., 7.), (10., 3.)], BLUE),
        NamedSeries::new("Outbound", &[(1., 4.), (7., 2.), (9., 6.)], RED),
    ];
    for roughness in 0..=2 {
        let elements = elements(
            AreaChart::from_series(&data, 0.0..10.0, 0.0..10.0)
                .opacity(0.5)
                .sketch(SketchStyle::new(roughness, FillStyle::Solid).unwrap())
                .render()
                .unwrap(),
        );
        let fills: Vec<_> = elements
            .iter()
            .filter(|e| e["type"] == "line" && e["backgroundColor"] != "transparent")
            .collect();
        assert_eq!(fills.len(), 2);
        for (fill, (name, color)) in fills
            .iter()
            .zip([("Inbound", "#1971c2"), ("Outbound", "#e03131")])
        {
            assert_eq!(fill["backgroundColor"], color);
            assert_eq!(fill["opacity"], 50);
            assert_eq!(fill["roughness"], roughness);
            let points = vertices(fill);
            assert_eq!(points.first(), points.last());
            assert_eq!(
                points[3].1, points[4].1,
                "both areas close to the common baseline"
            );
            let border = elements
                .iter()
                .find(|e| e["type"] == "line" && e["strokeColor"] == color)
                .unwrap();
            assert_eq!(border["opacity"], 100);
            assert_eq!(vertices(border), points[..3]);
            assert_series_group(&elements, name, &[fill, border]);
        }
        assert_ne!(fills[0]["groupIds"][0], fills[1]["groupIds"][0]);
        assert_eq!(fills[0]["groupIds"][1], fills[1]["groupIds"][1]);
    }
}

#[test]
fn stacked_areas_share_exact_boundaries_and_normalize_each_sample() {
    let data = [
        NamedSeries::new("A", &[(0., 2.), (5., 4.), (10., 1.)], BLUE),
        NamedSeries::new("B", &[(0., 6.), (5., 4.), (10., 3.)], RED),
    ];
    for percent in [false, true] {
        let chart = AreaChart::from_series(
            &data,
            0.0..10.0,
            if percent { 0.0..100.0 } else { 0.0..10.0 },
        );
        let elements = elements(
            if percent {
                chart.percent_stacked()
            } else {
                chart.stacked()
            }
            .render()
            .unwrap(),
        );
        let fills: Vec<_> = elements
            .iter()
            .filter(|e| e["type"] == "line" && e["backgroundColor"] != "transparent")
            .collect();
        assert_eq!(fills.len(), 2);
        let lower = vertices(fills[0]);
        let upper = vertices(fills[1]);
        assert_eq!(lower.first(), lower.last());
        assert_eq!(upper.first(), upper.last());
        assert_eq!(
            lower[..3],
            upper[3..6].iter().copied().rev().collect::<Vec<_>>()
        );
        let baseline = lower[3].1;
        let axis = elements
            .iter()
            .find(|e| {
                e["type"] == "line"
                    && e["strokeColor"] == "#000000"
                    && e["width"] == 0.0
                    && number(e, "height") > 100.
            })
            .unwrap();
        let plot_top = number(axis, "y");
        let plot_height = baseline - plot_top;
        for (i, expected) in if percent {
            [25., 50., 25.]
        } else {
            [20., 40., 10.]
        }
        .into_iter()
        .enumerate()
        {
            assert!(((baseline - lower[i].1) / plot_height * 100. - expected).abs() < 0.5);
        }
        if percent {
            assert!(upper[..3].iter().all(|p| p.1 == plot_top));
        } else {
            assert!(((baseline - upper[0].1) / plot_height - 0.8).abs() < 0.005);
            assert!(((baseline - upper[2].1) / plot_height - 0.4).abs() < 0.005);
        }
        assert_series_group(&elements, "A", &[fills[0]]);
        assert_series_group(&elements, "B", &[fills[1]]);
    }
}

#[test]
fn grouped_bars_are_side_by_side_signed_and_grouped_with_their_legend() {
    let data = [
        NamedBarSeries::new("Before", &[4., -2.], BLUE),
        NamedBarSeries::new("After", &[2., -1.], RED),
    ];
    let elements = elements(
        BarChart::from_series(&["Read", "Write"], &data, -5.0..5.0)
            .render()
            .unwrap(),
    );
    let bars: Vec<_> = elements
        .iter()
        .filter(|e| {
            e["type"] == "rectangle"
                && number(e, "height") != 14.
                && e["backgroundColor"] != "#ffffff"
        })
        .collect();
    assert_eq!(bars.len(), 4);
    assert!(number(bars[0], "x") + number(bars[0], "width") < number(bars[2], "x"));
    assert!(number(bars[2], "x") + number(bars[2], "width") < number(bars[1], "x"));
    let zero = number(bars[0], "y") + number(bars[0], "height");
    assert_eq!(number(bars[2], "y") + number(bars[2], "height"), zero);
    assert_eq!(number(bars[1], "y"), zero);
    assert_eq!(number(bars[3], "y"), zero);
    assert_series_group(&elements, "Before", &bars[..2]);
    assert_series_group(&elements, "After", &bars[2..]);
}

#[test]
fn stacked_bars_meet_without_gaps_and_skip_zero_segments() {
    let data = [
        NamedBarSeries::new("A", &[2., 4., 0.], BLUE),
        NamedBarSeries::new("B", &[6., 4., 3.], RED),
    ];
    for percent in [false, true] {
        let chart = BarChart::from_series(
            &["Jan", "Feb", "Mar"],
            &data,
            if percent { 0.0..100.0 } else { 0.0..10.0 },
        );
        let elements = elements(
            if percent {
                chart.percent_stacked()
            } else {
                chart.stacked()
            }
            .render()
            .unwrap(),
        );
        let bars: Vec<_> = elements
            .iter()
            .filter(|e| {
                e["type"] == "rectangle"
                    && number(e, "height") != 14.
                    && e["backgroundColor"] != "#ffffff"
            })
            .collect();
        assert_eq!(bars.len(), 5);
        for i in 0..2 {
            assert_eq!(bars[i]["x"], bars[i + 2]["x"]);
            assert_eq!(bars[i]["width"], bars[i + 2]["width"]);
            assert_eq!(
                number(bars[i], "y"),
                number(bars[i + 2], "y") + number(bars[i + 2], "height")
            );
        }
        if percent {
            assert_eq!(bars[2]["y"], bars[3]["y"]);
            assert_eq!(bars[3]["y"], bars[4]["y"]);
            assert!(
                (number(bars[0], "height")
                    / (number(bars[0], "height") + number(bars[2], "height"))
                    - 0.25)
                    .abs()
                    < 0.005
            );
        }
    }
}

#[test]
fn multi_series_rejects_invalid_alignment_totals_styles_and_crowding() {
    let a = [(0., 2.), (5., 4.), (10., 1.)];
    for b in [
        vec![(0., 1.), (10., 1.)],
        vec![(0., 1.), (6., 1.), (10., 1.)],
        vec![(0., 1.), (5., -1.), (10., 1.)],
    ] {
        assert!(
            AreaChart::from_series(
                &[
                    NamedSeries::new("A", &a, BLUE),
                    NamedSeries::new("B", &b, RED)
                ],
                0.0..10.0,
                -5.0..10.0
            )
            .stacked()
            .render()
            .is_err()
        );
    }
    for value in [f64::NAN, f64::INFINITY, -1.] {
        let values = [value, 2.];
        assert!(
            BarChart::from_series(
                &["A", "B"],
                &[NamedBarSeries::new("S", &values, BLUE)],
                0.0..100.0
            )
            .percent_stacked()
            .render()
            .is_err()
        );
    }
    assert!(
        BarChart::from_series(
            &["A", "B"],
            &[NamedBarSeries::new("S", &[1.], BLUE)],
            0.0..10.0
        )
        .render()
        .is_err()
    );
    assert!(
        BarChart::from_series(&["A"], &[], 0.0..10.0)
            .render()
            .is_err()
    );
    assert!(
        AreaChart::from_series(&[], 0.0..10.0, 0.0..10.0)
            .render()
            .is_err()
    );
    let series = [
        NamedBarSeries::new("A", &[6., 0.], BLUE),
        NamedBarSeries::new("B", &[6., 0.], RED),
    ];
    assert!(
        BarChart::from_series(&["A", "B"], &series, 0.0..10.0)
            .stacked()
            .render()
            .is_err()
    );
    assert!(
        BarChart::from_series(&["A", "B"], &series, 0.0..100.0)
            .percent_stacked()
            .render()
            .is_err()
    );
    let series = [
        NamedSeries::new("A", &a, BLUE),
        NamedSeries::new("B", &a, RED),
    ];
    assert!(
        AreaChart::from_series(&series, 0.0..10.0, 0.0..5.0)
            .stacked()
            .render()
            .is_err()
    );
    assert!(
        AreaChart::from_series(&series, 0.0..10.0, 0.0..90.0)
            .percent_stacked()
            .render()
            .is_err()
    );
    assert!(
        AreaChart::from_series(&series, 0.0..10.0, 0.0..10.0)
            .baseline(1.)
            .stacked()
            .render()
            .is_err()
    );
    for opacity in [f64::NAN, -1., 0., 0.004, 1.01] {
        assert!(
            AreaChart::from_series(&series, 0.0..10.0, 0.0..10.0)
                .opacity(opacity)
                .render()
                .is_err()
        );
    }
    let zeros = [NamedSeries::new("A", &[(0., 0.), (1., 1.)], BLUE)];
    assert!(
        AreaChart::from_series(&zeros, 0.0..1.0, 0.0..100.0)
            .percent_stacked()
            .render()
            .is_err()
    );
    let huge = [
        NamedBarSeries::new("A", &[f64::MAX], BLUE),
        NamedBarSeries::new("B", &[f64::MAX], RED),
    ];
    assert!(
        BarChart::from_series(&["A"], &huge, 0.0..100.0)
            .percent_stacked()
            .render()
            .is_err()
    );
    // Label layout must reserve legend width before assessing category crowding.
    let data = [NamedBarSeries::new("Long legend label", &[1.; 5], BLUE)];
    assert!(
        BarChart::from_series(&["Category"; 5], &data, 0.0..10.0)
            .render()
            .is_err()
    );
    // Nonzero stacked bands cannot silently disappear at integer drawing resolution.
    let tiny = [
        NamedBarSeries::new("A", &[1.], BLUE),
        NamedBarSeries::new("B", &[1e-10], RED),
    ];
    assert!(
        BarChart::from_series(&["A"], &tiny, 0.0..10.0)
            .stacked()
            .render()
            .is_err()
    );
    let tiny_area = [
        NamedSeries::new("A", &[(0., 1.), (1., 1.)], BLUE),
        NamedSeries::new("B", &[(0., 1e-10), (1., 1e-10)], RED),
    ];
    assert!(
        AreaChart::from_series(&tiny_area, 0.0..1.0, 0.0..10.0)
            .stacked()
            .render()
            .is_err()
    );
}

#[test]
fn decimal_stack_totals_accept_roundoff_but_reject_real_overflow() {
    for second in [0.2, 0.200001] {
        let second_values = [second];
        let bars = [
            NamedBarSeries::new("A", &[0.1], BLUE),
            NamedBarSeries::new("B", &second_values, RED),
        ];
        let b = [(0., second), (1., second)];
        let areas = [
            NamedSeries::new("A", &[(0., 0.1), (1., 0.1)], BLUE),
            NamedSeries::new("B", &b, RED),
        ];
        assert_eq!(
            BarChart::from_series(&["Jan"], &bars, 0.0..0.3)
                .stacked()
                .render()
                .is_ok(),
            second == 0.2
        );
        assert_eq!(
            AreaChart::from_series(&areas, 0.0..1.0, 0.0..0.3)
                .stacked()
                .render()
                .is_ok(),
            second == 0.2
        );
    }
    let tiny = [
        NamedBarSeries::new("A", &[1.], BLUE),
        NamedBarSeries::new("B", &[1e-20], RED),
    ];
    assert!(
        BarChart::from_series(&["Jan"], &tiny, 0.0..10.0)
            .stacked()
            .render()
            .is_err()
    );
}
