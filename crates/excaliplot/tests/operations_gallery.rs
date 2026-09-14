use serde_json::Value;
use std::{collections::HashSet, process::Command};

#[test]
fn gallery_composes_independent_charts_with_identical_roughness_variants() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("operations.excalidraw");
    let run = |overwrite: bool| {
        let mut command = Command::new(env!("CARGO"));
        command.args(["run", "--locked", "--example", "operations_gallery", "--"]);
        if overwrite {
            command.arg("--overwrite");
        }
        command.arg(&destination).output().unwrap()
    };
    let output = run(false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&std::fs::read(&destination).unwrap()).unwrap();
    assert_eq!(document["type"], "excalidraw");
    let elements = document["elements"].as_array().unwrap();
    let ids: HashSet<_> = elements.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids.len(),
        elements.len(),
        "composition must preserve unique identities"
    );
    let backgrounds: Vec<_> = elements
        .iter()
        .filter(|e| e["type"] == "rectangle" && e["backgroundColor"] == "#ffffff")
        .collect();
    assert_eq!(backgrounds.len(), 54);
    let mut all_groups = HashSet::new();
    let mut clean = Vec::new();
    for (index, background) in backgrounds.iter().enumerate() {
        let column = index / 18;
        let row = index % 18;
        let x = 40.0 + column as f64 * 1000.0;
        let y = 260.0 + row as f64 * 580.0;
        assert_eq!(background["x"], x);
        assert_eq!(background["y"], y);
        assert_eq!(background["width"], 960.0);
        assert_eq!(background["height"], 460.0);
        let outer = background["groupIds"].as_array().unwrap().last().unwrap();
        let chart: Vec<_> = elements
            .iter()
            .filter(|e| e["groupIds"].as_array().unwrap().last() == Some(outer))
            .collect();
        assert!(chart.len() > 10);
        let groups: HashSet<_> = chart
            .iter()
            .flat_map(|e| e["groupIds"].as_array().unwrap())
            .map(|g| g.as_str().unwrap())
            .collect();
        for group in groups {
            assert!(
                all_groups.insert(group),
                "groups must not link different charts"
            );
        }
        let normalized: Vec<_> = chart
            .iter()
            .map(|element| {
                let is_background = element["id"] == background["id"];
                let mut element = (*element).clone();
                let object = element.as_object_mut().unwrap();
                for key in ["id", "updated"] {
                    object.remove(key);
                }
                // Retain nesting depth while ignoring the intentionally fresh group IDs.
                let depth = object["groupIds"].as_array().unwrap().len();
                object.insert("groupIds".into(), depth.into());
                for (key, offset) in [("x", x), ("y", y)] {
                    let local = object[key].as_f64().unwrap() - offset;
                    object.insert(key.into(), ((local * 1e6).round() / 1e6).into());
                }
                // Backgrounds are clean regardless of the selected chart style.
                assert_eq!(object["roughness"], if is_background { 0 } else { column });
                object.insert("roughness".into(), 0.into());
                element
            })
            .collect();
        if column == 0 {
            clean.push(normalized);
        } else {
            assert_eq!(
                normalized, clean[row],
                "only roughness and placement should differ"
            );
        }
    }
    std::fs::write(&destination, b"manual edit").unwrap();
    assert!(!run(false).status.success());
    assert_eq!(std::fs::read(&destination).unwrap(), b"manual edit");
    assert!(run(true).status.success());
}
