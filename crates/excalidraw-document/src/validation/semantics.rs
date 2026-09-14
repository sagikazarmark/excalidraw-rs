//! Pinned field applicability and semantic checks. These diagnose data without
//! reproducing restore's lossy defaults or imposing drawing-tool size limits.
use super::{Severity, ValidationReport};
use crate::{Number, Object, Profile, Purpose, element};
use serde_json::Value;

const TEXT: &[&str] = &[
    "fontSize",
    "fontFamily",
    "baseFontSize",
    "text",
    "originalText",
    "textAlign",
    "verticalAlign",
    "containerId",
    "autoResize",
    "lineHeight",
    "labelPosition",
];
const LINEAR: &[&str] = &[
    "startBinding",
    "endBinding",
    "startArrowhead",
    "endArrowhead",
];
const ELBOW: &[&str] = &["fixedSegments", "startIsSpecial", "endIsSpecial"];
const FREEHAND: &[&str] = &["pressures", "simulatePressure", "strokeOptions"];
const IMAGE: &[&str] = &["fileId", "status", "scale", "crop"];

/// Unknown keys remain extensions; common fields apply to every known kind.
pub(crate) fn applies(kind: &str, field: &str, object: &Object) -> bool {
    if TEXT.contains(&field) {
        return kind == "text";
    }
    if LINEAR.contains(&field) {
        return matches!(kind, "line" | "arrow" | "draw");
    }
    if ELBOW.contains(&field) {
        return kind == "arrow" && object.get("elbowed") == Some(&Value::Bool(true));
    }
    if FREEHAND.contains(&field) {
        return kind == "freedraw";
    }
    if IMAGE.contains(&field) {
        return kind == "image";
    }
    match field {
        "points" | "lastCommittedPoint" => matches!(kind, "line" | "arrow" | "draw" | "freedraw"),
        "polygon" => kind == "line",
        "elbowed" => kind == "arrow",
        "name" => matches!(kind, "frame" | "magicframe"),
        "baseHeight" => kind == "stickynote",
        _ => true,
    }
}

pub(super) fn bindable_kinds(profile: Profile) -> &'static [&'static str] {
    match profile {
        Profile::V0_18_1 => &[
            "rectangle",
            "diamond",
            "ellipse",
            "text",
            "image",
            "iframe",
            "embeddable",
            "frame",
            "magicframe",
        ],
        Profile::SnapshotAfa3a653 => &[
            "rectangle",
            "diamond",
            "ellipse",
            "text",
            "image",
            "iframe",
            "embeddable",
            "frame",
            "magicframe",
            "stickynote",
        ],
    }
}

pub(super) fn is_bound_text(object: &Object) -> bool {
    object.get("type").and_then(Value::as_str) == Some("text")
        && object.get("containerId").is_some_and(|v| !v.is_null())
}

pub(crate) fn compatibility_severity(purpose: Purpose) -> Severity {
    if purpose == Purpose::Inspect {
        Severity::Warning
    } else {
        Severity::Error
    }
}

pub(super) fn check(
    report: &mut ValidationReport,
    object: &Object,
    path: &str,
    kind: &str,
    profile: Profile,
    purpose: Purpose,
) {
    if !crate::ElementKind::KNOWN.contains(&kind) {
        return;
    }
    for field in element::FIELDS {
        if object.contains_key(*field) && !applies(kind, field, object) {
            if kind == "arrow" && ELBOW.contains(field) {
                report.issue(
                    format!("{path}/{field}"),
                    "inactive-routing",
                    Severity::Warning,
                    "inactive elbow metadata retained by editor arrow-type conversion",
                );
                continue;
            }
            report.issue(
                format!("{path}/{field}"),
                "field-kind",
                compatibility_severity(purpose),
                format!("{field} does not belong to {kind} in the pinned model"),
            );
        }
    }
    if let Some(Value::Number(version)) = object.get("version")
        && crate::Number(version.clone())
            .as_safe_integer()
            .is_ok_and(|v| v <= 0)
    {
        report.issue(
            format!("{path}/version"),
            "range",
            compatibility_severity(purpose),
            "element revision must be positive",
        );
    }
    if profile == Profile::SnapshotAfa3a653 {
        if object.contains_key("lastCommittedPoint") {
            report.issue(
                format!("{path}/lastCommittedPoint"),
                "profile",
                compatibility_severity(purpose),
                "legacy interaction field is absent from the snapshot model",
            );
        }
        for field in ["startArrowhead", "endArrowhead"] {
            if applies(kind, field, object)
                && matches!(
                    object.get(field).and_then(Value::as_str),
                    Some("dot" | "crowfoot_one" | "crowfoot_many" | "crowfoot_one_or_many")
                )
            {
                report.issue(
                    format!("{path}/{field}"),
                    "profile",
                    compatibility_severity(purpose),
                    "legacy arrowhead requires explicit migration to snapshot spelling",
                );
            }
        }
    }
    if kind == "text" {
        for key in ["fontSize", "lineHeight", "baseFontSize"] {
            positive(report, object, path, key);
        }
        if let Some(Value::Number(value)) = object.get("fontFamily") {
            match Number(value.clone()).as_safe_integer() {
                Ok(id)
                    if matches!(id, 1..=3 | 5..=9)
                        || (id == 10 && profile == Profile::SnapshotAfa3a653) => {}
                _ => report.issue(
                    format!("{path}/fontFamily"),
                    "font-family",
                    compatibility_severity(purpose),
                    "font ID is not in the selected editor's primary font registry",
                ),
            }
        }
    }
    if let Some(roundness) = object.get("roundness").and_then(Value::as_object) {
        if let Some(Value::Number(value)) = roundness.get("type")
            && !matches!(Number(value.clone()).as_safe_integer(), Ok(1..=3))
        {
            report.issue(
                format!("{path}/roundness/type"),
                "roundness-type",
                compatibility_severity(purpose),
                "unknown roundness algorithm (expected 1, 2 or 3)",
            );
        }
        nonnegative(report, roundness, &format!("{path}/roundness"), "value");
    }
    match kind {
        "freedraw" => freehand(report, object, path, profile, purpose),
        "image" => image(report, object, path),
        "arrow" if object.get("elbowed") == Some(&Value::Bool(true)) => {
            elbow(report, object, path, purpose)
        }
        "line" if object.get("polygon") == Some(&Value::Bool(true)) => {
            if let Some(points) = points(object)
                && (points.len() < 4 || !points_close(points[0], points[points.len() - 1], 1e-4))
            {
                report.issue(
                    format!("{path}/polygon"),
                    "polygon-closure",
                    compatibility_severity(purpose),
                    "polygon needs at least four points with matching first/last endpoints",
                );
            }
        }
        _ => {}
    }
}

fn pair(value: &Value) -> Option<[f64; 2]> {
    let array = value.as_array()?;
    if array.len() != 2 {
        return None;
    }
    let result = [array[0].as_f64()?, array[1].as_f64()?];
    result.iter().all(|v| v.is_finite()).then_some(result)
}
fn points(object: &Object) -> Option<Vec<[f64; 2]>> {
    object.get("points")?.as_array()?.iter().map(pair).collect()
}
fn points_close(a: [f64; 2], b: [f64; 2], tolerance: f64) -> bool {
    (a[0] - b[0]).abs() < tolerance && (a[1] - b[1]).abs() < tolerance
}

fn freehand(
    report: &mut ValidationReport,
    object: &Object,
    path: &str,
    profile: Profile,
    purpose: Purpose,
) {
    let options = object.get("strokeOptions").and_then(Value::as_object);
    // Snapshot constant-width rendering ignores pressure samples entirely.
    let constant = profile == Profile::SnapshotAfa3a653
        && options
            .and_then(|o| o.get("variability"))
            .and_then(Value::as_str)
            == Some("constant");
    if let Some(pressures) = object.get("pressures").and_then(Value::as_array) {
        for (i, pressure) in pressures.iter().enumerate() {
            if let Some(value) = pressure.as_f64()
                && !(0.0..=1.0).contains(&value)
            {
                report.issue(
                    format!("{path}/pressures/{i}"),
                    "range",
                    Severity::Error,
                    "pressure is outside 0..1",
                );
            }
        }
        if !constant
            && object.get("simulatePressure") == Some(&Value::Bool(false))
            && let Some(points) = object.get("points").and_then(Value::as_array)
            && points.len() != pressures.len()
        {
            report.issue(format!("{path}/pressures"),"pressure-count",compatibility_severity(purpose),"recorded pressure count differs from point count; editor may use fallback pressure");
        }
    }
    if let Some(streamline) = options
        .and_then(|o| o.get("streamline"))
        .and_then(Value::as_f64)
        && !(0.0..=1.0).contains(&streamline)
    {
        report.issue(
            format!("{path}/strokeOptions/streamline"),
            "range",
            compatibility_severity(purpose),
            "streamline is outside the renderer's normalized 0..1 domain",
        );
    }
}

fn elbow(report: &mut ValidationReport, object: &Object, path: &str, purpose: Purpose) {
    let points = points(object);
    if let Some(points) = &points {
        for (i, pair) in points.windows(2).enumerate() {
            if pair[0][0] != pair[1][0] && pair[0][1] != pair[1][1] {
                report.issue(
                    format!("{path}/points/{}", i + 1),
                    "elbow-axis",
                    compatibility_severity(purpose),
                    "elbow segment is neither horizontal nor vertical",
                );
            }
        }
    }
    let Some(segments) = object.get("fixedSegments").and_then(Value::as_array) else {
        return;
    };
    let mut indices = std::collections::BTreeSet::new();
    for (i, segment) in segments.iter().enumerate() {
        let p = format!("{path}/fixedSegments/{i}");
        let Some(Value::Number(index)) = segment.get("index") else {
            continue;
        };
        let Ok(index) = Number(index.clone()).as_safe_integer() else {
            report.issue(
                format!("{p}/index"),
                "segment-index",
                Severity::Error,
                "expected integral segment index",
            );
            continue;
        };
        if index < 1
            || points
                .as_ref()
                .is_some_and(|points| index as u64 >= points.len() as u64)
        {
            report.issue(
                format!("{p}/index"),
                "segment-index",
                Severity::Error,
                "segment index must address points[index - 1] and points[index]",
            );
            continue;
        }
        if !indices.insert(index) {
            report.issue(
                format!("{p}/index"),
                "duplicate-segment",
                Severity::Error,
                "segment index occurs more than once",
            );
        }
        let start = segment.get("start").and_then(pair);
        let end = segment.get("end").and_then(pair);
        if let (Some(start), Some(end)) = (start, end)
            && start[0] != end[0]
            && start[1] != end[1]
        {
            report.issue(
                &p,
                "elbow-axis",
                compatibility_severity(purpose),
                "fixed segment is neither horizontal nor vertical",
            );
        }
        if let Some(points) = &points {
            for (name, actual, expected) in [
                ("start", start, points[index as usize - 1]),
                ("end", end, points[index as usize]),
            ] {
                if actual.is_some_and(|actual| !points_close(actual, expected, 1e-4)) {
                    report.issue(
                        format!("{p}/{name}"),
                        "segment-points",
                        compatibility_severity(purpose),
                        "fixed segment endpoint disagrees with the indexed path point",
                    );
                }
            }
            if index == 1 || index as usize == points.len() - 1 {
                report.issue(format!("{p}/index"),"terminal-segment",compatibility_severity(purpose),"terminal fixed segments are transient editor state, not stable authored routing");
            }
        }
    }
}

fn image(report: &mut ValidationReport, object: &Object, path: &str) {
    if let Some(scale) = object.get("scale").and_then(Value::as_array) {
        for (i, value) in scale.iter().enumerate() {
            if value.as_f64() == Some(0.) {
                report.issue(
                    format!("{path}/scale/{i}"),
                    "range",
                    Severity::Error,
                    "image scale must be nonzero (negative flips are supported)",
                );
            }
        }
    }
    let Some(crop) = object.get("crop").and_then(Value::as_object) else {
        return;
    };
    let p = format!("{path}/crop");
    for (key, natural) in [("x", "naturalWidth"), ("y", "naturalHeight")] {
        if let Some(origin) = crop.get(key).and_then(Value::as_f64) {
            let tolerance = crop
                .get(natural)
                .and_then(Value::as_f64)
                .filter(|n| n.is_finite() && *n > 0.)
                .unwrap_or(0.)
                * (8. * f64::EPSILON);
            if origin < -tolerance {
                report.issue(
                    format!("{p}/{key}"),
                    "range",
                    Severity::Error,
                    "expected nonnegative crop origin (within floating-point tolerance)",
                );
            }
        }
    }
    for key in ["width", "height", "naturalWidth", "naturalHeight"] {
        positive(report, crop, &p, key);
    }
    for (origin, extent, natural) in [
        ("x", "width", "naturalWidth"),
        ("y", "height", "naturalHeight"),
    ] {
        if let (Some(origin), Some(size), Some(limit)) = (
            crop.get(origin).and_then(Value::as_f64),
            crop.get(extent).and_then(Value::as_f64),
            crop.get(natural).and_then(Value::as_f64),
        ) && [origin, size, limit].iter().all(|n| n.is_finite())
            && size > 0.
            && limit > 0.
        {
            // A scale-relative epsilon tolerates crop arithmetic (0.1 + 0.2),
            // while subtraction avoids overflow in origin + size.
            let tolerance = limit.abs().max(origin.abs()).max(size.abs()) * (8. * f64::EPSILON);
            if origin > limit + tolerance || size - (limit - origin) > tolerance {
                report.issue(
                    format!("{p}/{extent}"),
                    "crop-bounds",
                    Severity::Error,
                    "crop extends beyond natural image dimensions",
                );
            }
        }
    }
}

fn positive(report: &mut ValidationReport, object: &Object, path: &str, key: &str) {
    if let Some(value) = object.get(key).and_then(Value::as_f64)
        && value <= 0.
    {
        report.issue(
            format!("{path}/{key}"),
            "range",
            Severity::Error,
            "expected a positive value",
        );
    }
}
fn nonnegative(report: &mut ValidationReport, object: &Object, path: &str, key: &str) {
    if let Some(value) = object.get(key).and_then(Value::as_f64)
        && value < 0.
    {
        report.issue(
            format!("{path}/{key}"),
            "range",
            Severity::Error,
            "expected a nonnegative value",
        );
    }
}
