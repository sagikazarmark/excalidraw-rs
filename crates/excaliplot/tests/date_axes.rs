use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use excaliplot::{
    DateLineChart, DateScatterChart, Error, LineChart, Scene, TickFormat, UtcLineChart,
    UtcScatterChart,
};
use serde_json::Value;

fn date(month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2024, month, day).unwrap()
}

fn instant(day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2024, 12, day, hour, minute, 0)
        .unwrap()
}

#[test]
fn calendar_and_numeric_y_axes_share_measured_layout_and_fit_failures() {
    let start = date(2, 28);
    let end = date(3, 3);
    let dates = [(start, -1e9), (end, 1e9)];
    let numbers = [(0., -1e9), (4., 1e9)];
    for title in [
        "Shared layout",
        "A caption reaching into the measured Y tick column",
    ] {
        for size in [(1200, 400), (1600, 600)] {
            let calendar = elements(
                DateLineChart::new(&dates, start..end, -1e9..1e9)
                    .labels(title, "X", "Value")
                    .size(size)
                    .y_tick_format(TickFormat::Decimal { places: 9 })
                    .render()
                    .unwrap(),
            );
            let numeric = elements(
                LineChart::new(&numbers, 0.0..4.0, -1e9..1e9)
                    .labels(title, "X", "Value")
                    .size(size)
                    .y_tick_format(TickFormat::Decimal { places: 9 })
                    .render()
                    .unwrap(),
            );
            // Long Y labels dominate the left margin. Compare the Y geometry
            // and text placement; calendar endpoint labels need a wider right margin.
            for scene in [&calendar, &numeric] {
                assert!(scene.iter().any(|e| e["text"] == "1000000000.000000000"));
            }
            let geometry = |scene: &[Value]| -> Vec<Value> {
                scene
                    .iter()
                    .filter(|e| {
                        e["strokeColor"] == "#1971c2"
                            || e["text"]
                                .as_str()
                                .is_some_and(|s| s.ends_with(".000000000"))
                    })
                    .map(|e| serde_json::json!([e["type"], e["x"], e["y"], e["height"], e["text"]]))
                    .collect()
            };
            assert_eq!(geometry(&calendar), geometry(&numeric));
        }
    }
    let calendar = DateLineChart::new(&dates, start..end, -1e9..1e9)
        .size((400, 300))
        .y_tick_format(TickFormat::Decimal { places: 9 })
        .render()
        .err()
        .unwrap();
    let numeric = LineChart::new(&numbers, 0.0..4.0, -1e9..1e9)
        .size((400, 300))
        .y_tick_format(TickFormat::Decimal { places: 9 })
        .render()
        .err()
        .unwrap();
    assert!(calendar.to_string().contains("insufficient plot width"));
    assert_eq!(calendar.to_string(), numeric.to_string());

    let error = DateLineChart::new(&[(start, 0.), (end, 0.01)], start..end, 0.0..0.01)
        .y_tick_format(TickFormat::Decimal { places: 0 })
        .render()
        .err()
        .unwrap();
    assert!(error.to_string().contains("distinct labels"));
}

#[test]
fn utc_labels_remain_distinct_for_multiday_and_subsecond_ticks() {
    let start = instant(30, 0, 0);
    let end = start + Duration::days(2);
    let items = elements(
        UtcLineChart::new(&[(start, 1.), (end, 2.)], start..end, 0.0..3.0)
            .size((2000, 400))
            .render()
            .unwrap(),
    );
    assert!(items.iter().any(|e| e["text"] == "2024-12-30 08:00:00Z"));
    let start = start + Duration::nanoseconds(100);
    let end = start + Duration::nanoseconds(800);
    let items = elements(
        UtcLineChart::new(&[(start, 1.), (end, 2.)], start..end, 0.0..3.0)
            .size((2000, 400))
            .tick_density(3, 6)
            .render()
            .unwrap(),
    );
    assert!(
        items
            .iter()
            .any(|e| e["text"] == "2024-12-30 00:00:00.000000500Z")
    );
}

#[test]
fn utc_sub_day_ticks_and_duplicates_survive_year_boundary_and_composition() {
    let start = instant(31, 23, 0);
    let end = Utc.with_ymd_and_hms(2025, 1, 1, 1, 0, 0).unwrap();
    let points = [
        (start, 2.),
        (start + Duration::minutes(30), 5.),
        (start + Duration::minutes(30), 3.),
        (end, 4.),
    ];
    let mut line = UtcLineChart::new(&points, start..end, 0.0..6.0)
        .size((1400, 400))
        .tick_density(3, 6)
        .render()
        .unwrap();
    line.translate((40., 50.)).unwrap();
    let mut scene = Scene::new();
    scene.append(line).unwrap();
    let items = elements(scene);
    for text in ["2024-12-31 23:00:00Z", "2025-01-01 00:00:00Z"] {
        assert!(items.iter().any(|e| e["text"] == text), "{text}");
    }
    let line = items
        .iter()
        .find(|e| e["strokeColor"] == "#1971c2")
        .unwrap();
    let vertices = line["points"].as_array().unwrap();
    assert_eq!(vertices.len(), 4);
    assert_eq!(vertices[1][0], vertices[2][0]);
    assert_ne!(vertices[1][1], vertices[2][1]);
    let x = |i: usize| vertices[i][0].as_f64().unwrap();
    assert!((x(1) - x(3) / 4.).abs() <= 1.);
    // Scatter intentionally retains unordered exact duplicates as separate marks.
    let unordered = [points[3], points[1], points[1], points[0]];
    let scatter = elements(
        UtcScatterChart::new(&unordered, start..end, 0.0..6.0)
            .size((1400, 400))
            .tick_density(3, 6)
            .render()
            .unwrap(),
    );
    let marks: Vec<_> = scatter.iter().filter(|e| e["type"] == "ellipse").collect();
    assert_eq!(marks.len(), 4);
    assert_eq!(marks[1]["x"], marks[2]["x"]);
    assert_eq!(marks[1]["y"], marks[2]["y"]);
    assert_ne!(marks[1]["id"], marks[2]["id"]);
    assert!(marks[0]["x"].as_f64().unwrap() > marks[1]["x"].as_f64().unwrap());
}

#[test]
fn calendar_bounds_observations_and_typography_are_validated_before_drawing() {
    let a = date(2, 28);
    let b = date(3, 3);
    for bounds in [
        a..a,
        b..a,
        NaiveDate::MIN..b,
        a..NaiveDate::MAX,
        NaiveDate::from_ymd_opt(1600, 1, 1).unwrap()..b,
    ] {
        assert!(matches!(
            DateLineChart::new(&[(a, 1.), (b, 2.)], bounds.clone(), 0.0..3.0).render(),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            DateScatterChart::new(&[(a, 1.)], bounds, 0.0..3.0).render(),
            Err(Error::Invalid(_))
        ));
    }
    for points in [
        vec![],
        vec![(a, 1.)],
        vec![(a, 1.), (a, 1.)],
        vec![(b, 1.), (a, 2.)],
        vec![(a - Duration::days(1), 1.), (b, 2.)],
        vec![(a, f64::NAN), (b, 2.)],
        vec![(a, 1.), (b, 4.)],
    ] {
        assert!(matches!(
            DateLineChart::new(&points, a..b, 0.0..3.0).render(),
            Err(Error::Invalid(_))
        ));
    }
    for bounds in [
        0.0..0.0,
        3.0..0.0,
        f64::NAN..3.,
        0.0..f64::INFINITY,
        -1e10..3.,
    ] {
        assert!(matches!(
            DateLineChart::new(&[(a, 1.), (b, 2.)], a..b, bounds).render(),
            Err(Error::Invalid(_))
        ));
    }
    assert!(matches!(
        DateLineChart::new(&[(a, 1.), (b, 2.)], a..b, 0.0..3.0)
            .labels("λ", "Date", "Value")
            .render(),
        Err(Error::UnsupportedGlyph('λ'))
    ));
    // Constant Y and vertical segments on a repeated date are valid lines.
    for points in [[(a, 1.), (b, 1.)], [(a, 1.), (a, 2.)]] {
        assert!(DateLineChart::new(&points, a..b, 0.0..3.0).render().is_ok());
    }
    let start = instant(31, 23, 0);
    let end = start + Duration::hours(2);
    let leap = DateTime::from_timestamp(1_483_228_799, 1_000_000_000).unwrap();
    for bounds in [
        start..start,
        end..start,
        DateTime::<Utc>::MIN_UTC..end,
        start..DateTime::<Utc>::MAX_UTC,
        leap..end,
    ] {
        assert!(matches!(
            UtcScatterChart::new(&[(start, 1.)], bounds, 0.0..3.0).render(),
            Err(Error::Invalid(_))
        ));
    }
    for points in [
        vec![],
        vec![(start - Duration::seconds(1), 1.)],
        vec![(start, f64::INFINITY)],
        vec![(leap, 1.)],
    ] {
        assert!(matches!(
            UtcScatterChart::new(&points, start..end, 0.0..3.0).render(),
            Err(Error::Invalid(_))
        ));
    }
    assert!(matches!(
        UtcLineChart::new(&[(end, 1.), (start, 2.)], start..end, 0.0..3.0).render(),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn measured_tick_fit_mapped_collapse_and_full_marker_clearance_are_enforced() {
    let start = instant(31, 23, 0);
    let end = start + Duration::hours(2);
    let error = UtcLineChart::new(&[(start, 1.), (end, 2.)], start..end, 0.0..3.0)
        .size((900, 400))
        .tick_density(20, 6)
        .render()
        .err()
        .unwrap();
    assert!(error.to_string().contains("crowded"), "{error}");
    let error = UtcLineChart::new(
        &[(start, 1.), (start + Duration::nanoseconds(1), 1.)],
        start..end,
        0.0..3.0,
    )
    .size((1400, 400))
    .tick_density(3, 6)
    .render()
    .err()
    .unwrap();
    assert!(error.to_string().contains("collapses"), "{error}");
    let error = DateLineChart::new(
        &[(date(2, 28), 1.), (date(2, 29), 1.)],
        NaiveDate::from_ymd_opt(1800, 1, 1).unwrap()..date(3, 3),
        0.0..3.0,
    )
    .render()
    .err()
    .unwrap();
    assert!(error.to_string().contains("collapses"), "{error}");
    for (x, y) in [
        (date(2, 28), 1.),
        (date(3, 3), 1.),
        (date(3, 1), 0.),
        (date(3, 1), 3.),
    ] {
        let error = DateScatterChart::new(&[(x, y)], date(2, 28)..date(3, 3), 0.0..3.0)
            .marker(20, false)
            .render()
            .err()
            .unwrap();
        assert!(error.to_string().contains("marker crowds"), "{error}");
    }
    let items = elements(
        DateScatterChart::new(
            &[(date(3, 1), 1.), (date(2, 29), 2.), (date(2, 29), 2.)],
            date(2, 28)..date(3, 3),
            0.0..3.0,
        )
        .marker(20, false)
        .opacity(0.5)
        .render()
        .unwrap(),
    );
    let marks: Vec<_> = items.iter().filter(|e| e["type"] == "ellipse").collect();
    assert_eq!(marks.len(), 3);
    assert!(
        marks
            .iter()
            .all(|e| e["width"] == 40. && e["opacity"] == 50)
    );
    // The numeric route still has its original bounded-X behavior.
    assert!(
        LineChart::new(
            &[(1_700_000_000., 1.), (1_700_000_001., 2.)],
            1_700_000_000.0..1_700_000_002.0,
            0.0..3.0
        )
        .render()
        .is_err()
    );
    assert!(
        LineChart::new(&[(0., 1.), (1., 2.)], 0.0..2.0, 0.0..3.0)
            .render()
            .is_ok()
    );
}

fn elements(scene: Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn irregular_dates_use_elapsed_days_across_leap_day_and_month_boundary() {
    let points = [(date(2, 28), 2.), (date(2, 29), 5.), (date(3, 3), 3.)];
    let items = elements(
        DateLineChart::new(&points, date(2, 28)..date(3, 3), 0.0..6.0)
            .size((900, 400))
            .render()
            .unwrap(),
    );
    let line = items
        .iter()
        .find(|e| e["strokeColor"] == "#1971c2")
        .unwrap();
    let vertices = line["points"].as_array().unwrap();
    assert_eq!(vertices.len(), 3);
    let gap = |i: usize| vertices[i][0].as_f64().unwrap() - vertices[i - 1][0].as_f64().unwrap();
    assert!((gap(2) - 3. * gap(1)).abs() <= 3.);
    assert_eq!(line["groupIds"].as_array().unwrap().len(), 1);
    for label in ["2024-02-28", "2024-02-29", "2024-03-01", "2024-03-03"] {
        assert!(items.iter().any(|e| e["text"] == label), "{label}");
    }
}

#[test]
fn irregular_observation_example_exports_native_panels_and_protects_editor_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("date_axes.excalidraw");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args(["run", "--locked", "--quiet", "--example", "date_axes", "--"])
            .arg(&path)
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let doc: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let items = doc["elements"].as_array().unwrap();
    for title in [
        "Irregular daily observations",
        "UTC observations across midnight",
    ] {
        assert!(items.iter().any(|e| e["text"] == title));
    }
    assert!(
        items.iter().all(
            |e| ["text", "line", "ellipse", "rectangle"].contains(&e["type"].as_str().unwrap())
        )
    );
    std::fs::write(&path, b"editor work").unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&path).unwrap(), b"editor work");
}
