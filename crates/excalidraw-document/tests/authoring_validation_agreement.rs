//! What `Element::new` produces, `Document::validate` must accept — and what it
//! refuses, it must refuse for a stated reason.
//!
//! The profile × kind field table is spelled independently in `model.rs`
//! (`Element::new`, as default values), `validation.rs` (`required`, as names
//! plus a nullable sublist) and `migration.rs` (as defaults plus removability).
//! `semantics.rs` records that split as deliberate, and it is right about the
//! *differences*. The *agreements* had no guardrail: 47fd7a1 and 5554e84 are
//! the same drift found twice, each fixed on one rule, neither leaving a test
//! that would catch the next one. `assert_authored` in `tests/constructors.rs`
//! reaches five of the thirteen authorable kinds.
//!
//! This is a characterization test. It passes as written against the current
//! code — that is the point. It states today's answers, including the
//! deliberate asymmetries, so a later single-sourcing of the table cannot
//! quietly change one of them.

use excalidraw_document::{
    Document, Element, ElementId, ElementKind, Number, Profile, Purpose, Severity,
};

const PROFILES: [Profile; 2] = [Profile::V0_18_1, Profile::SnapshotAfa3a653];
const AUTHORING_PURPOSES: [Purpose; 2] = [Purpose::Author, Purpose::SelfContained];

/// Kinds `Element::new` refuses, and why. `selection` and `draw` are editor
/// transients in every profile; `stickynote` is snapshot-only.
fn refused(kind: &ElementKind, profile: Profile) -> bool {
    matches!(kind, ElementKind::Selection | ElementKind::Draw)
        || (*kind == ElementKind::Stickynote && profile == Profile::V0_18_1)
}

fn authored(kind: &ElementKind, profile: Profile) -> Option<Element> {
    Element::new(
        kind.clone(),
        profile,
        ElementId::from("e1"),
        Number::from(1_u64),
    )
    .ok()
}

fn scene_of(element: Element) -> Document {
    let mut document = Document::new("authoring-validation-agreement");
    document.set_elements(vec![element]);
    document
}

#[test]
fn every_authorable_kind_constructs_and_then_validates_clean() {
    let mut reached = 0;
    for profile in PROFILES {
        for wire in ElementKind::KNOWN {
            let kind = ElementKind::from_wire(wire);
            let element = authored(&kind, profile);

            if refused(&kind, profile) {
                assert!(
                    element.is_none(),
                    "{wire} on {profile:?}: Element::new should refuse it"
                );
                continue;
            }

            let element = element
                .unwrap_or_else(|| panic!("{wire} on {profile:?}: Element::new should accept it"));
            let document = scene_of(element);
            for purpose in AUTHORING_PURPOSES {
                let report = document.validate(profile, purpose);
                assert!(
                    report.is_valid(),
                    "{wire} on {profile:?} at {purpose:?}: authoring produced a record \
                     validation rejects — {:?}",
                    report.diagnostics
                );
            }
            reached += 1;
        }
    }
    // Thirteen authorable kinds on snapshot, twelve on V0_18_1 (no stickynote).
    assert_eq!(
        reached, 25,
        "the sweep must cover every kind on both profiles"
    );
}

#[test]
fn the_rejection_set_is_exactly_these_kinds() {
    for profile in PROFILES {
        let rejected: Vec<&str> = ElementKind::KNOWN
            .iter()
            .filter(|wire| authored(&ElementKind::from_wire(wire), profile).is_none())
            .copied()
            .collect();
        let expected: &[&str] = match profile {
            Profile::V0_18_1 => &["stickynote", "selection", "draw"],
            _ => &["selection", "draw"],
        };
        assert_eq!(rejected, expected, "rejection set for {profile:?}");
    }
}

/// `draw` and `selection` are an unconditional validation `Error`, at every
/// purpose including `Inspect`. Every other profile-compatibility rule grades
/// down to a warning under `Inspect`. That asymmetry is deliberate and nothing
/// pinned it, so a table refactor could normalize it away unnoticed.
#[test]
fn editor_transient_kinds_stay_errors_even_under_inspect() {
    for profile in PROFILES {
        for wire in ["draw", "selection"] {
            let document = Document::from_value(serde_json::json!({
                "type": "excalidraw", "version": 2, "source": "t",
                "elements": [{ "type": wire, "id": "e1" }],
                "appState": {}, "files": {}
            }))
            .unwrap();
            let report = document.validate(profile, Purpose::Inspect);
            assert!(
                report.diagnostics.iter().any(|d| d.code == "profile"
                    && d.severity == Severity::Error
                    && d.path.ends_with("/type")),
                "{wire} on {profile:?} under Inspect should still be an Error — {:?}",
                report.diagnostics
            );
        }
    }
}

/// The other direction is *not* an equivalence, and a naive "authoring rejects
/// it, so validation must too" test would be wrong. An unrecognised kind is a
/// hard authoring error but inspects clean: a preserving document is allowed to
/// carry a kind this crate does not know.
#[test]
fn an_unknown_kind_is_refused_by_authoring_yet_inspects_clean() {
    let kind = ElementKind::from_wire("notarealkind");
    assert!(matches!(kind, ElementKind::Unknown(_)));

    for profile in PROFILES {
        assert!(
            authored(&kind, profile).is_none(),
            "Element::new must refuse an unknown kind"
        );
        let document = Document::from_value(serde_json::json!({
            "type": "excalidraw", "version": 2, "source": "t",
            "elements": [{ "type": "notarealkind", "id": "e1" }],
            "appState": {}, "files": {}
        }))
        .unwrap();
        let report = document.validate(profile, Purpose::Inspect);
        assert!(
            report.is_valid(),
            "an unknown kind must inspect clean on {profile:?} — {:?}",
            report.diagnostics
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "unknown-kind" && d.severity == Severity::Warning),
            "and it must say so as a warning"
        );
    }
}
