//! `fixtures/plus-app-saved.json` is a file written by the Excalidraw+ app
//! (`source: https://app.excalidraw.com`) on 2026-09-23, kept byte for byte.
//! It is an excaliplot line chart, generated for the 0.18.1 profile, opened in
//! the app and saved unchanged.
//!
//! The app upgraded every element to its unreleased model on load, yet the
//! result matches neither profile: it carries the snapshot's fields but still
//! writes `lastCommittedPoint` on lines and arrows, which the snapshot profile
//! rejects. These tests pin that, so a change on either side shows up here.
//! Whether the snapshot profile should tolerate the field is undecided.

use excalidraw_document::{Document, Profile, Purpose, Severity};
use std::collections::BTreeMap;

fn saved() -> Document {
    Document::from_slice(include_bytes!("fixtures/plus-app-saved.json")).unwrap()
}

/// Diagnostics as `field code severity` → count.
fn summary(profile: Profile, purpose: Purpose) -> (bool, BTreeMap<String, usize>) {
    let report = saved().validate(profile, purpose);
    let mut counts = BTreeMap::new();
    for d in &report.diagnostics {
        let field = d.path.rsplit('/').next().unwrap();
        *counts
            .entry(format!("{field} {} {:?}", d.code, d.severity))
            .or_default() += 1;
    }
    (report.is_valid(), counts)
}

fn expected(entries: &[(&str, usize)]) -> BTreeMap<String, usize> {
    entries.iter().map(|(k, n)| ((*k).to_owned(), *n)).collect()
}

#[test]
fn the_app_upgrades_released_elements_to_the_snapshot_model() {
    let value = saved().into_value();
    let elements = value["elements"].as_array().unwrap();
    assert_eq!(elements.len(), 24);
    for e in elements {
        assert!(e["created"].is_null() && e.get("created").is_some(), "{e}");
        match e["type"].as_str().unwrap() {
            "line" => assert_eq!(e["polygon"], false),
            "text" => {
                assert!(e.get("baseFontSize").is_some_and(|v| v.is_null()));
                assert!(e.get("labelPosition").is_some_and(|v| v.is_null()));
            }
            _ => {}
        }
    }
    assert!(value["appState"].get("lockedMultiSelections").is_some());
}

#[test]
fn released_profile_rejects_the_upgraded_fields() {
    let (valid, counts) = summary(Profile::V0_18_1, Purpose::Author);
    assert!(!valid);
    assert_eq!(
        counts,
        expected(&[
            ("baseFontSize profile Error", 12),
            ("created profile Error", 24),
            ("labelPosition profile Error", 12),
            ("lockedMultiSelections profile Error", 1),
            ("polygon profile Error", 9),
        ])
    );
}

#[test]
fn snapshot_profile_rejects_only_the_legacy_last_committed_point() {
    let (valid, counts) = summary(Profile::SnapshotAfa3a653, Purpose::Author);
    assert!(!valid);
    assert_eq!(
        counts,
        expected(&[("lastCommittedPoint profile Error", 10)])
    );

    let (valid, counts) = summary(Profile::SnapshotAfa3a653, Purpose::Inspect);
    assert!(valid, "inspection downgrades the mismatch to warnings");
    assert_eq!(
        counts,
        expected(&[("lastCommittedPoint profile Warning", 10)])
    );
}

#[test]
fn every_legacy_field_sits_on_a_line_or_arrow() {
    let document = saved();
    let value = document.as_object();
    let elements = value["elements"].as_array().unwrap();
    let report = document.validate(Profile::SnapshotAfa3a653, Purpose::Author);
    for d in &report.diagnostics {
        assert_eq!(d.severity, Severity::Error);
        let index: usize = d.path.split('/').nth(2).unwrap().parse().unwrap();
        let kind = elements[index]["type"].as_str().unwrap();
        assert!(matches!(kind, "line" | "arrow"), "{}: {kind}", d.path);
    }
}
