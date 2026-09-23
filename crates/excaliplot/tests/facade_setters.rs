//! Every façade setter must reach the emitted scene.
//!
//! `AreaChart::opacity`/`baseline` and `ScatterChart::marker`/`opacity` used to
//! patch a `Marks` variant through `if let` with no `else`. On a variant
//! mismatch they accepted the call and did nothing — a public option that
//! silently stops working, invisible to every other test in the suite because
//! the mismatch was unreachable by construction rather than by type. The mark
//! payload now lives on the façade and is assembled in `render`, so the
//! assignment cannot miss. These assertions pin that it still arrives.

use excaliplot::{AreaChart, ScatterChart, Scene};
use serde_json::Value;

fn elements(scene: &Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

const AREA: [(f64, f64); 3] = [(1.0, 2.0), (3.0, 5.0), (6.0, 3.0)];
const DOTS: [(f64, f64); 2] = [(2.0, 2.0), (7.0, 6.0)];

fn area() -> AreaChart<'static> {
    AreaChart::new(&AREA, 0.0..10.0, 0.0..10.0)
}

fn scatter() -> ScatterChart<'static> {
    ScatterChart::new(&DOTS, 0.0..10.0, 0.0..10.0)
}

fn fill(chart: AreaChart<'_>) -> Value {
    elements(&chart.render().unwrap())
        .into_iter()
        .find(|e| e["backgroundColor"] == "#1971c2")
        .expect("the area fill")
}

fn mark(chart: ScatterChart<'_>) -> Value {
    elements(&chart.render().unwrap())
        .into_iter()
        .find(|e| e["type"] == "ellipse")
        .expect("a scatter mark")
}

#[test]
fn area_opacity_and_baseline_reach_the_emitted_fill() {
    let default = fill(area());
    assert_eq!(default["opacity"], 35, "the documented default fill alpha");

    assert_eq!(
        fill(area().opacity(0.6))["opacity"],
        60,
        "opacity must reach the fill, not just the builder"
    );
    assert_ne!(
        fill(area().baseline(1.0))["points"],
        default["points"],
        "baseline must move the fill's closing edge"
    );
}

#[test]
fn scatter_marker_and_opacity_reach_the_emitted_marks() {
    let default = mark(scatter());
    assert_eq!(default["opacity"], 100, "marks are opaque by default");

    assert_ne!(
        mark(scatter().marker(8, true))["width"],
        default["width"],
        "radius must reach the mark"
    );
    assert_ne!(
        mark(scatter().marker(5, false))["backgroundColor"],
        default["backgroundColor"],
        "fill must reach the mark"
    );
    assert_eq!(
        mark(scatter().opacity(0.5))["opacity"],
        50,
        "opacity must reach the mark"
    );
}

/// Identity and provenance differ per render: a fresh namespace drives `id` and
/// `groupIds`, and `updated` is a timestamp. Compare what the setters control.
fn shape(element: &Value) -> Vec<&Value> {
    [
        "type",
        "x",
        "y",
        "width",
        "height",
        "points",
        "opacity",
        "backgroundColor",
        "strokeColor",
        "fillStyle",
        "strokeWidth",
    ]
    .iter()
    .map(|key| &element[*key])
    .collect()
}

#[test]
fn facade_setters_compose_in_either_order() {
    let a = fill(area().opacity(0.6).baseline(1.0));
    let b = fill(area().baseline(1.0).opacity(0.6));
    assert_eq!(
        shape(&a),
        shape(&b),
        "area setters are independent assignments"
    );

    let a = mark(scatter().marker(7, false).opacity(0.5));
    let b = mark(scatter().opacity(0.5).marker(7, false));
    assert_eq!(
        shape(&a),
        shape(&b),
        "scatter setters are independent assignments"
    );
}
