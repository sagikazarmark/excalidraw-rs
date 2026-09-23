//! PROTOTYPE: throwaway, not a supported example. Delete once the design settles.
//!
//! Question: can an agent regenerate an excaliplot chart *inside* an existing
//! `.excalidraw` document with today's crates and no library change?
//!
//! Every chart element carries `customData.excaliplot` in the existing
//! `Provenance` shape: role `chart`, `sourceKey` = chart id, and attributes
//! `{specRev, ordinal, count, fingerprint}`. Replacing a chart finds its elements
//! by that tag, keeps the old top-left, removes them, renders the new spec there,
//! and repairs whatever the user attached to the old chart. The spec itself lives
//! in the agent's store, never in the scene.
//!
//! Run: `cargo run -p excaliplot --example prototype_chart_regenerate`
//! Writes one file per stage to `output/prototype-chart-regenerate/`,
//! overwriting them on every run, so each step can be opened in the editor.

use excalidraw_document::{Document, Profile, Purpose};
use excaliplot::{ArrowStyle, LineChart, Provenance, Scene};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

const DIR: &str = "output/prototype-chart-regenerate";

/// What the agent keeps in its own store under the chart id.
struct Spec {
    title: &'static str,
    points: Vec<(f64, f64)>,
    x: (f64, f64),
    y: (f64, f64),
}

// ---------------------------------------------------------------- tagging

/// Fields that change when a chart is merely moved, reordered, bound to,
/// grouped or pasted. Anything else changing means the user edited the element.
const VOLATILE: &[&str] = &[
    "id",
    "seed",
    "x",
    "y",
    "version",
    "versionNonce",
    "updated",
    "customData",
    "frameId",
    "groupIds",
    "boundElements",
    "index",
    "locked",
    "link",
];

fn render(chart: &str, rev: u64, spec: &Spec) -> Res<Vec<Value>> {
    let mut scene = LineChart::new(&spec.points, spec.x.0..spec.x.1, spec.y.0..spec.y.1)
        .labels(spec.title, "Time", "Value")
        .render()?;
    scene.add_frame(16.0, Some(&format!("chart {chart}")))?;
    let doc: Value = serde_json::from_slice(&scene.to_bytes()?)?;
    let mut elements = doc["elements"].as_array().unwrap().clone();
    let count = elements.len();
    for (ordinal, e) in elements.iter_mut().enumerate() {
        let attributes = json!({
            "specRev": rev, "ordinal": ordinal, "count": count, "fingerprint": fingerprint(e),
        });
        let provenance = Provenance::new("chart", Some(chart), attributes)?;
        e["customData"] = json!({ "excaliplot": provenance });
    }
    Ok(elements)
}

struct Tag {
    chart: String,
    rev: u64,
    ordinal: u64,
    count: u64,
    fingerprint: String,
}

fn tag(e: &Value) -> Option<Tag> {
    let p = &e["customData"]["excaliplot"];
    if p["role"] != "chart" {
        return None;
    }
    let a = &p["attributes"];
    Some(Tag {
        chart: p["sourceKey"].as_str()?.into(),
        rev: a["specRev"].as_u64()?,
        ordinal: a["ordinal"].as_u64()?,
        count: a["count"].as_u64()?,
        fingerprint: a["fingerprint"].as_str()?.into(),
    })
}

/// FNV-1a over a number-normalized rendering, so `640` and `640.0` agree and
/// the value is stable across toolchains (unlike `DefaultHasher`).
fn fingerprint(e: &Value) -> String {
    let mut o = e.as_object().cloned().unwrap_or_default();
    for k in VOLATILE {
        o.remove(*k);
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in canonical(&Value::Object(o)).bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn canonical(v: &Value) -> String {
    let join = |parts: Vec<String>| parts.join(",");
    match v {
        Value::Number(n) => format!("{:?}", n.as_f64().unwrap()),
        Value::Array(a) => format!("[{}]", join(a.iter().map(canonical).collect())),
        Value::Object(o) => format!(
            "{{{}}}",
            join(
                o.iter()
                    .map(|(k, v)| format!("{k:?}:{}", canonical(v)))
                    .collect()
            )
        ),
        other => other.to_string(),
    }
}

// ------------------------------------------------------------------ find

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: (f64, f64),
    max: (f64, f64),
}

fn bounds<'a>(elements: impl IntoIterator<Item = &'a Value>) -> Bounds {
    let mut b = Bounds {
        min: (f64::MAX, f64::MAX),
        max: (f64::MIN, f64::MIN),
    };
    for e in elements {
        let (x, y) = (e["x"].as_f64().unwrap(), e["y"].as_f64().unwrap());
        let extents: Vec<(f64, f64)> = match e["points"].as_array() {
            Some(points) => points
                .iter()
                .map(|p| (x + p[0].as_f64().unwrap(), y + p[1].as_f64().unwrap()))
                .collect(),
            None => {
                let (w, h) = (e["width"].as_f64().unwrap(), e["height"].as_f64().unwrap());
                vec![(x, y), (x + w, y + h)]
            }
        };
        for (px, py) in extents {
            b.min = (b.min.0.min(px), b.min.1.min(py));
            b.max = (b.max.0.max(px), b.max.1.max(py));
        }
    }
    b
}

fn live(e: &Value) -> bool {
    e["isDeleted"] != true
}

/// Everything the tag tells us about one chart in a document.
struct Found {
    /// Indices of every tagged element, tombstones included.
    all: Vec<usize>,
    live: Vec<usize>,
    rev: u64,
    count: u64,
    bounds: Bounds,
    /// User edited the element itself, beyond moving or reordering it.
    modified: Vec<String>,
    /// Ordinals with no live element: the user deleted part of the chart.
    missing: Vec<u64>,
    /// Ordinals with more than one live element: the chart was copy-pasted.
    duplicated: Vec<u64>,
}

fn charts(doc: &Value) -> BTreeMap<String, Found> {
    let mut out: BTreeMap<String, Found> = BTreeMap::new();
    let mut seen: BTreeMap<String, BTreeMap<u64, usize>> = BTreeMap::new();
    let elements = doc["elements"].as_array().unwrap();
    for (i, e) in elements.iter().enumerate() {
        let Some(t) = tag(e) else { continue };
        let f = out.entry(t.chart.clone()).or_insert(Found {
            all: vec![],
            live: vec![],
            rev: t.rev,
            count: t.count,
            bounds: bounds([]),
            modified: vec![],
            missing: vec![],
            duplicated: vec![],
        });
        f.all.push(i);
        if !live(e) {
            continue;
        }
        f.live.push(i);
        f.rev = f.rev.max(t.rev);
        *seen
            .entry(t.chart)
            .or_default()
            .entry(t.ordinal)
            .or_default() += 1;
        if fingerprint(e) != t.fingerprint {
            f.modified.push(e["id"].as_str().unwrap().into());
        }
    }
    for (chart, f) in &mut out {
        let ordinals = seen.remove(chart).unwrap_or_default();
        f.missing = (0..f.count).filter(|o| !ordinals.contains_key(o)).collect();
        f.duplicated = ordinals
            .into_iter()
            .filter(|(_, n)| *n > 1)
            .map(|(o, _)| o)
            .collect();
        f.bounds = bounds(f.live.iter().map(|&i| &elements[i]));
    }
    out
}

// --------------------------------------------------------------- replace

#[derive(Debug)]
struct Report {
    chart: String,
    rev: (u64, u64),
    old: Bounds,
    new: Bounds,
    /// Edits the regeneration discarded.
    lost_edits: Vec<String>,
    missing: Vec<u64>,
    /// Untagged elements whose references to the old chart were changed.
    repairs: Vec<String>,
    /// User groups that included the chart, carried onto the new elements.
    user_groups: Vec<String>,
}

fn insert(doc: &mut Value, chart: &str, spec: &Spec, at: (f64, f64)) -> Res<()> {
    if charts(doc).contains_key(chart) {
        return Err(format!("chart {chart:?} already exists; replace it instead").into());
    }
    let mut new = render(chart, 1, spec)?;
    move_to(&mut new, at);
    doc["elements"].as_array_mut().unwrap().extend(new);
    Ok(())
}

fn move_to(elements: &mut [Value], at: (f64, f64)) {
    let b = bounds(elements.iter());
    let (dx, dy) = (at.0 - b.min.0, at.1 - b.min.1);
    for e in elements {
        e["x"] = json!(e["x"].as_f64().unwrap() + dx);
        e["y"] = json!(e["y"].as_f64().unwrap() + dy);
    }
}

fn replace(doc: &mut Value, chart: &str, spec: &Spec) -> Res<Report> {
    let mut all = charts(doc);
    let found = all
        .remove(chart)
        .ok_or_else(|| format!("no chart {chart:?} in document"))?;
    if !found.duplicated.is_empty() {
        return Err(format!(
            "chart {chart:?} appears more than once ({} of {} elements repeat): it was \
             copied; ask which copy to replace",
            found.duplicated.len(),
            found.count
        )
        .into());
    }
    if found.live.is_empty() {
        return Err(format!("chart {chart:?} was deleted by the user").into());
    }
    let elements = doc["elements"].as_array().unwrap();
    let removed: BTreeSet<String> = found
        .all
        .iter()
        .map(|&i| elements[i]["id"].as_str().unwrap().to_owned())
        .collect();
    let old_frame = found
        .live
        .iter()
        .map(|&i| &elements[i])
        .find(|e| e["type"] == "frame")
        .map(|e| e["id"].as_str().unwrap().to_owned());

    // Groups the user formed around the chart and something else, such as
    // "chart + my note". The generator's own groups never include untagged
    // elements, so any group shared with one is the user's.
    let untagged_groups: BTreeSet<&str> = elements
        .iter()
        .filter(|e| live(e) && tag(e).is_none())
        .flat_map(|e| {
            e["groupIds"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
        })
        .collect();
    let mut user_groups: Vec<String> = vec![];
    for &i in &found.live {
        for g in elements[i]["groupIds"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
        {
            if untagged_groups.contains(g) && !user_groups.iter().any(|u| u == g) {
                user_groups.push(g.to_owned());
            }
        }
    }

    let mut new = render(chart, found.rev + 1, spec)?;
    move_to(&mut new, found.bounds.min);
    for e in &mut new {
        let groups = e["groupIds"].as_array_mut().unwrap();
        groups.extend(user_groups.iter().map(|g| json!(g)));
    }
    let new_frame = new
        .iter()
        .find(|e| e["type"] == "frame")
        .map(|e| e["id"].clone());
    let new_bounds = bounds(new.iter());

    let mut repairs = vec![];
    let mut out = Vec::with_capacity(elements.len());
    let mut new = Some(new);
    for e in elements {
        let id = e["id"].as_str().unwrap();
        if removed.contains(id) {
            // Keep the chart's painter position: new elements go where the old began.
            out.extend(new.take().into_iter().flatten());
            continue;
        }
        let mut e = e.clone();
        for key in ["startBinding", "endBinding"] {
            if let Some(target) = e[key]["elementId"].as_str()
                && removed.contains(target)
            {
                repairs.push(format!(
                    "{id}: {key} pointed at chart element {target}; unbound"
                ));
                e[key] = Value::Null;
            }
        }
        if let Some(target) = e["containerId"].as_str()
            && removed.contains(target)
        {
            repairs.push(format!(
                "{id}: was bound inside chart element {target}; unbound"
            ));
            e["containerId"] = Value::Null;
        }
        if let Some(bound) = e["boundElements"].as_array_mut() {
            let before = bound.len();
            bound.retain(|b| !removed.contains(b["id"].as_str().unwrap_or_default()));
            if bound.len() != before {
                repairs.push(format!(
                    "{id}: dropped {} binding(s) to the old chart",
                    before - bound.len()
                ));
            }
        }
        if let Some(frame) = e["frameId"].as_str()
            && removed.contains(frame)
        {
            if Some(frame) == old_frame.as_deref()
                && let Some(new_frame) = &new_frame
            {
                repairs.push(format!("{id}: moved into the new chart frame"));
                e["frameId"] = new_frame.clone();
            } else {
                repairs.push(format!("{id}: its frame {frame} was removed; now unframed"));
                e["frameId"] = Value::Null;
            }
        }
        out.push(e);
    }
    doc["elements"] = Value::Array(out);

    Ok(Report {
        chart: chart.into(),
        rev: (found.rev, found.rev + 1),
        old: found.bounds,
        new: new_bounds,
        lost_edits: found.modified,
        missing: found.missing,
        repairs,
        user_groups,
    })
}

// ------------------------------------------------ simulated editor actions
//
// These stand in for what a person does in Excalidraw between agent runs. Each
// bumps `version`, as the editor does on every change.

fn bump(e: &mut Value) {
    e["version"] = json!(e["version"].as_u64().unwrap() + 1);
}

fn tagged_mut<'a>(doc: &'a mut Value, chart: &'a str) -> impl Iterator<Item = &'a mut Value> {
    doc["elements"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(move |e| live(e) && tag(e).is_some_and(|t| t.chart == chart))
}

fn user_moves_chart(doc: &mut Value, chart: &str, by: (f64, f64)) {
    for e in tagged_mut(doc, chart) {
        e["x"] = json!(e["x"].as_f64().unwrap() + by.0);
        e["y"] = json!(e["y"].as_f64().unwrap() + by.1);
        bump(e);
    }
}

fn user_recolors_series(doc: &mut Value, chart: &str) -> String {
    let e = tagged_mut(doc, chart)
        .find(|e| e["type"] == "line" && e["points"].as_array().unwrap().len() > 2)
        .unwrap();
    e["strokeColor"] = json!("#e03131");
    bump(e);
    e["id"].as_str().unwrap().into()
}

fn user_deletes_one_label(doc: &mut Value, chart: &str) {
    let e = tagged_mut(doc, chart)
        .find(|e| e["type"] == "text")
        .unwrap();
    e["isDeleted"] = json!(true);
    bump(e);
}

/// Bind the document's first arrow's end to the chart's background rectangle.
fn user_binds_arrow(doc: &mut Value, chart: &str) {
    let arrow = find_mut(doc, |e| e["type"] == "arrow")["id"].clone();
    let target = tagged_mut(doc, chart)
        .find(|e| e["type"] == "rectangle")
        .unwrap();
    target["boundElements"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": arrow, "type": "arrow"}));
    bump(target);
    let target = target["id"].clone();
    let arrow = find_mut(doc, |e| e["type"] == "arrow");
    arrow["endBinding"] = json!({"elementId": target, "focus": 0.0, "gap": 4.0});
    bump(arrow);
}

fn user_adds_note_in_frame(doc: &mut Value, chart: &str) -> Res<()> {
    let frame = tagged_mut(doc, chart)
        .find(|e| e["type"] == "frame")
        .unwrap();
    let (frame_id, x, y) = (
        frame["id"].clone(),
        frame["x"].as_f64().unwrap(),
        frame["y"].as_f64().unwrap(),
    );
    let mut scene = Scene::new();
    scene.add_note("spike = deploy", (x + 420.0, y + 60.0), 16.0)?;
    let mut note = elements_of(&scene)?.remove(0);
    note["frameId"] = frame_id;
    note["groupIds"] = json!([]);
    doc["elements"].as_array_mut().unwrap().push(note);
    Ok(())
}

/// Select the chart plus the first note and press Ctrl+G.
fn user_groups_chart_with_note(doc: &mut Value, chart: &str) {
    for e in tagged_mut(doc, chart) {
        e["groupIds"]
            .as_array_mut()
            .unwrap()
            .push(json!("user-group-1"));
        bump(e);
    }
    let note = find_mut(doc, |e| e["type"] == "text" && tag(e).is_none());
    note["groupIds"]
        .as_array_mut()
        .unwrap()
        .push(json!("user-group-1"));
    bump(note);
}

/// Copy the chart and paste it below. The editor issues fresh element and group
/// ids but copies `customData` verbatim, so the tag comes along.
fn user_pastes_copy(doc: &mut Value, chart: &str) {
    let copies: Vec<Value> = tagged_mut(doc, chart)
        .map(|e| {
            let mut c = e.clone();
            c["id"] = json!(format!("{}-copy", c["id"].as_str().unwrap()));
            if let Some(f) = c["frameId"].as_str() {
                c["frameId"] = json!(format!("{f}-copy"));
            }
            for g in c["groupIds"].as_array_mut().unwrap() {
                *g = json!(format!("{}-copy", g.as_str().unwrap()));
            }
            c["y"] = json!(c["y"].as_f64().unwrap() + 600.0);
            c["boundElements"] = json!([]);
            c
        })
        .collect();
    doc["elements"].as_array_mut().unwrap().extend(copies);
}

fn find_mut(doc: &mut Value, pred: impl Fn(&Value) -> bool) -> &mut Value {
    doc["elements"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|e| pred(e))
        .unwrap()
}

// ------------------------------------------------------------ walkthrough

fn elements_of(scene: &Scene) -> Res<Vec<Value>> {
    let doc: Value = serde_json::from_slice(&scene.to_bytes()?)?;
    Ok(doc["elements"].as_array().unwrap().clone())
}

fn show(stage: &str, doc: &Value) -> Res<()> {
    let path = format!("{DIR}/{stage}.excalidraw");
    std::fs::write(&path, serde_json::to_vec_pretty(doc)?)?;
    let report = Document::from_value(doc.clone())?.validate(Profile::V0_18_1, Purpose::Author);
    let elements = doc["elements"].as_array().unwrap();
    let untagged = elements
        .iter()
        .filter(|e| live(e) && tag(e).is_none())
        .count();
    println!("\n== {stage}  ({path})");
    println!("   untagged live elements: {untagged}");
    for (id, f) in charts(doc) {
        let b = f.bounds;
        println!(
            "   chart {id:?} rev {}: {}/{} live, at ({:.0},{:.0}) size {:.0}x{:.0}",
            f.rev,
            f.live.len(),
            f.count,
            b.min.0,
            b.min.1,
            b.max.0 - b.min.0,
            b.max.1 - b.min.1
        );
        println!(
            "     modified {:?}  missing ordinals {:?}  duplicated ordinals: {}",
            f.modified,
            f.missing,
            f.duplicated.len()
        );
    }
    if report.is_valid() {
        println!("   validation (0.18.1, author): ok");
    } else {
        println!(
            "   validation (0.18.1, author): {} diagnostic(s)",
            report.diagnostics.len()
        );
        for d in report.diagnostics.iter().take(5) {
            println!("     {d:?}");
        }
    }
    Ok(())
}

fn print_report(r: &Report) {
    println!("   replaced {:?}: rev {} -> {}", r.chart, r.rev.0, r.rev.1);
    println!(
        "   old ({:.0},{:.0}) {:.0}x{:.0}  ->  new ({:.0},{:.0}) {:.0}x{:.0}",
        r.old.min.0,
        r.old.min.1,
        r.old.max.0 - r.old.min.0,
        r.old.max.1 - r.old.min.1,
        r.new.min.0,
        r.new.min.1,
        r.new.max.0 - r.new.min.0,
        r.new.max.1 - r.new.min.1
    );
    println!("   WARN edits discarded on: {:?}", r.lost_edits);
    println!(
        "   note: user had deleted ordinals {:?}; they are back",
        r.missing
    );
    println!("   user groups carried over: {:?}", r.user_groups);
    for repair in &r.repairs {
        println!("   repair: {repair}");
    }
}

fn main() -> Res<()> {
    std::fs::create_dir_all(DIR)?;
    let v1 = Spec {
        title: "Latency",
        points: vec![(0., 2.), (1., 5.), (2., 3.), (3., 4.)],
        x: (0., 3.),
        y: (0., 6.),
    };
    let v2 = Spec {
        title: "Latency (p99, ms)",
        points: vec![
            (0., 20.),
            (1., 55.),
            (2., 31.),
            (3., 42.),
            (4., 90.),
            (5., 38.),
        ],
        x: (0., 5.),
        y: (0., 100.),
    };

    // A document the user already has: a note and an arrow.
    let mut base = Scene::new();
    base.add_note("Q3 latency review", (0., 0.), 24.)?;
    base.add_arrow((0., 200.), (400., 200.), ArrowStyle::default())?;
    let mut doc: Value = serde_json::from_slice(&base.to_bytes()?)?;
    show("0-base", &doc)?;

    insert(&mut doc, "latency", &v1, (420., 0.))?;
    show("1-inserted", &doc)?;

    user_moves_chart(&mut doc, "latency", (150., 80.));
    let recolored = user_recolors_series(&mut doc, "latency");
    user_deletes_one_label(&mut doc, "latency");
    user_binds_arrow(&mut doc, "latency");
    user_adds_note_in_frame(&mut doc, "latency")?;
    user_groups_chart_with_note(&mut doc, "latency");
    println!("\n-- user moved the chart by (150,80), recolored {recolored}, deleted a label,");
    println!(
        "   bound the arrow to the chart, added a note inside its frame, grouped it with their note"
    );
    show("2-user-edited", &doc)?;

    let report = replace(&mut doc, "latency", &v2)?;
    show("3-replaced", &doc)?;
    print_report(&report);

    // Replacing again with no user edits in between must be clean.
    let report = replace(&mut doc, "latency", &v2)?;
    println!("\n-- replace again, no edits in between");
    println!(
        "   lost edits {:?}, repairs {:?}",
        report.lost_edits, report.repairs
    );

    user_pastes_copy(&mut doc, "latency");
    println!("\n-- user copy-pasted the chart");
    show("4-duplicated", &doc)?;
    match replace(&mut doc, "latency", &v1) {
        Ok(_) => println!("   UNEXPECTED: replace went through"),
        Err(e) => println!("   replace refused: {e}"),
    }
    Ok(())
}
