//! The font and roundness registries have one owner, and validation asks it.
//!
//! `validation::semantics` used to re-spell `KnownFont`'s ID ranges inline
//! (`1..=3 | 5..=9`, plus 10 on the snapshot profile) and `KnownRoundness`'s as
//! `1..=3`. Two spellings of one registry, with nothing failing if either side
//! gained an entry. These tests pin the agreement behaviourally, so the
//! registry stays the single place an ID is admitted.
//!
//! This is the `tests/remap.rs` guardrail applied to a second table pair.

use excalidraw_document::{
    Document, KnownFont, KnownRoundness, Number, Profile, Purpose, Severity,
};
use serde_json::{Value, json};

const PROFILES: [Profile; 2] = [Profile::V0_18_1, Profile::SnapshotAfa3a653];

/// Ids either side of the registry, including reserved 4 and the fallback-only
/// range above 10.
const IDS: std::ops::RangeInclusive<i64> = -1..=12;

fn scene(elements: Value) -> Document {
    Document::from_value(
        json!({"type":"excalidraw","version":2,"source":"test","elements":elements,"appState":{},"files":{}}),
    )
    .unwrap()
}

fn diagnosed(document: &Document, profile: Profile, code: &str) -> bool {
    document
        .validate(profile, Purpose::Author)
        .diagnostics
        .iter()
        .any(|d| d.code == code && d.severity == Severity::Error)
}

#[test]
fn the_font_registry_decides_which_ids_validation_admits() {
    for profile in PROFILES {
        for id in IDS {
            let admitted = KnownFont::from_number(profile, &Number::from(id)).is_some();
            let document = scene(json!([{"type":"text","fontFamily":id}]));
            assert_eq!(
                admitted,
                !diagnosed(&document, profile, "font-family"),
                "font ID {id} on {profile:?}: the registry and the validator disagree"
            );
        }
    }
}

#[test]
fn the_roundness_registry_decides_which_algorithms_validation_admits() {
    for profile in PROFILES {
        for id in IDS {
            let admitted = KnownRoundness::from_number(profile, &Number::from(id)).is_some();
            let document = scene(json!([{"type":"rectangle","roundness":{"type":id}}]));
            assert_eq!(
                admitted,
                !diagnosed(&document, profile, "roundness-type"),
                "roundness {id} on {profile:?}: the registry and the validator disagree"
            );
        }
    }
}

#[test]
fn reserved_and_fallback_only_font_ids_stay_outside_the_registry() {
    for profile in PROFILES {
        assert!(
            KnownFont::from_number(profile, &Number::from(4_i64)).is_none(),
            "ID 4 is reserved upstream"
        );
        assert!(KnownFont::from_number(profile, &Number::from(11_i64)).is_none());
    }
}

/// `to_number` answers "what is this font's upstream ID", `from_number` answers
/// "does this profile's registry carry that ID". They differ for `Assistant`,
/// which is snapshot-only — so the round trip is deliberately partial, and
/// `in_registry` is what says which half applies.
#[test]
fn every_font_round_trips_exactly_where_its_profile_carries_it() {
    for profile in PROFILES {
        for font in [
            KnownFont::Virgil,
            KnownFont::Helvetica,
            KnownFont::Cascadia,
            KnownFont::Excalifont,
            KnownFont::Nunito,
            KnownFont::LilitaOne,
            KnownFont::ComicShanns,
            KnownFont::LiberationSans,
            KnownFont::Assistant,
        ] {
            let decoded = KnownFont::from_number(profile, &font.to_number());
            assert_eq!(
                decoded.is_some(),
                font.in_registry(profile),
                "{font:?} on {profile:?}: in_registry and from_number disagree"
            );
            if let Some(decoded) = decoded {
                assert_eq!(decoded, font, "{font:?} decoded as a different font");
            }
        }
    }
    assert!(!KnownFont::Assistant.in_registry(Profile::V0_18_1));
    assert!(KnownFont::Assistant.in_registry(Profile::SnapshotAfa3a653));
}
