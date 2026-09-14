#[path = "../examples/support/line_chart.rs"]
mod line_chart;

use serde_json::Value;

#[test]
fn real_plotters_chart_is_native_bounded_and_has_one_six_point_series() {
    let scene = line_chart::draw().unwrap();
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let elements = doc["elements"].as_array().unwrap();
    assert!(elements.len() <= 80);
    assert_eq!(elements[0]["type"], "rectangle");
    assert_eq!(elements[0]["width"], 640.0);
    assert_eq!(elements[0]["height"], 400.0);
    assert_eq!(
        elements.iter().filter(|e| e["type"] == "rectangle").count(),
        1
    );
    assert!(
        elements
            .iter()
            .all(|e| ["line", "rectangle", "text"].contains(&e["type"].as_str().unwrap()))
    );
    let series: Vec<_> = elements
        .iter()
        .filter(|e| e["points"].as_array().is_some_and(|p| p.len() == 6))
        .collect();
    assert_eq!(series.len(), 1);
    assert_eq!(series[0]["strokeWidth"], 3);
    assert_eq!(series[0]["strokeColor"], "#1971c2");
    let absolute_points: Vec<_> = series[0]["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                series[0]["x"].as_f64().unwrap() + p[0].as_f64().unwrap(),
                series[0]["y"].as_f64().unwrap() + p[1].as_f64().unwrap(),
            )
        })
        .collect();
    // Worked fixture: plotting range x=112..615, y=311..64 after caption,
    // margins and label areas. Integer Plotters mapping preserves unequal X gaps.
    assert_eq!(
        absolute_points,
        [
            (162.0, 262.0),
            (212.0, 188.0),
            (313.0, 237.0),
            (413.0, 114.0),
            (464.0, 163.0),
            (564.0, 89.0)
        ]
    );
    let ticks_at = |axis: &str| -> Vec<(String, f64, f64)> {
        elements
            .iter()
            .filter(|e| {
                e["type"] == "text"
                    && e["text"].as_str().unwrap().parse::<u32>().is_ok()
                    && if axis == "x" {
                        e["y"] == 322.0
                    } else {
                        e["x"].as_f64().unwrap() < 100.0
                    }
            })
            .map(|e| {
                (
                    e["text"].as_str().unwrap().to_owned(),
                    (e["x"].as_f64().unwrap()
                        + if axis == "x" {
                            e["width"].as_f64().unwrap() / 2.0
                        } else {
                            e["width"].as_f64().unwrap()
                        })
                    .round(),
                    e["y"].as_f64().unwrap()
                        + if axis == "y" {
                            e["height"].as_f64().unwrap() / 2.0
                        } else {
                            0.0
                        },
                )
            })
            .collect()
    };
    assert_eq!(
        ticks_at("x"),
        [
            ("0", 112.0, 322.0),
            ("2", 212.0, 322.0),
            ("4", 313.0, 322.0),
            ("6", 413.0, 322.0),
            ("8", 514.0, 322.0),
            ("10", 615.0, 322.0)
        ]
        .map(|(text, x, y)| (text.to_owned(), x, y))
    );
    assert_eq!(
        ticks_at("y"),
        [
            ("0", 102.0, 311.0),
            ("2", 102.0, 262.0),
            ("4", 102.0, 213.0),
            ("6", 102.0, 163.0),
            ("8", 102.0, 114.0),
            ("10", 102.0, 64.0)
        ]
        .map(|(text, x, y)| (text.to_owned(), x, y))
    );
    for text in ["Six-point line", "Time (s)", "Value", "0", "10"] {
        assert!(elements.iter().any(|e| e["text"] == text), "missing {text}");
    }
    let y_label = elements.iter().find(|e| e["text"] == "Value").unwrap();
    assert_eq!(y_label["angle"], 3.0 * std::f64::consts::FRAC_PI_2);
    let stats = scene.diagnostics();
    for operation in ["draw_pixel", "blit_bitmap", "draw_circle", "fill_polygon"] {
        assert_eq!(stats.calls.get(operation), None, "unexpected {operation}");
    }
    assert_eq!(stats.calls["draw_rect"], 1);
    assert_eq!(stats.elements["rectangle"], 1);
    assert!(stats.vertices < 100);
}
