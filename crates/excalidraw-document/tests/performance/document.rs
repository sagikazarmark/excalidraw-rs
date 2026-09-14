//! Bounded dependency-free baseline. Run `cargo bench -p excalidraw-document
//! --bench document -- --sizes=1000,10000,100000 --samples=3`.
//! Timings include dropping operation output; fixture generation is excluded.
use excalidraw_document::{
    Document, Element, ElementId, ElementKind, ExportMode, Number, Profile, Purpose,
};
use serde_json::{Value, json};
use std::{hint::black_box, time::Instant};

fn elapsed<T>(samples: usize, mut operation: impl FnMut() -> T) -> Value {
    drop(black_box(operation())); // Warm caches separately from measured samples.
    let mut millis = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        drop(black_box(operation()));
        millis.push(start.elapsed().as_secs_f64() * 1000.);
    }
    millis.sort_by(f64::total_cmp);
    json!({"minMs":millis[0],"medianMs":millis[samples/2],"maxMs":millis[samples-1]})
}

fn fixture(size: usize, family: &str) -> Document {
    let mut document = Document::new("benchmark");
    let template = Element::new(
        ElementKind::Rectangle,
        Profile::V0_18_1,
        ElementId::from("template"),
        Number::from(1_u64),
    )
    .unwrap();
    let count = if family == "dense-path" { 1 } else { size };
    let mut elements = Vec::with_capacity(count);
    for i in 0..count {
        let mut element = template.clone();
        element.set_raw("id", json!(format!("e{i}")));
        element.set_raw("x", json!(i));
        element.set_raw("width", json!(10));
        element.set_raw("height", json!(10));
        element.set_raw("customData", json!({"source":"synthetic","ordinal":i}));
        match family {
            "shared-image" => {
                for (key, value) in [
                    ("type", json!("image")),
                    ("fileId", json!("shared")),
                    ("status", json!("saved")),
                    ("scale", json!([1, 1])),
                    ("crop", Value::Null),
                ] {
                    element.set_raw(key, value);
                }
            }
            "frame-chain" => {
                element.set_raw("type", json!("frame"));
                element.set_raw("name", Value::Null);
                if i > 0 {
                    element.set_raw("frameId", json!(format!("e{}", i - 1)));
                }
            }
            "dense-path" => {
                element.set_raw("type", json!("line"));
                element.set_raw(
                    "points",
                    Value::Array((0..size).map(|i| json!([i, i % 100])).collect()),
                );
                for key in [
                    "startBinding",
                    "endBinding",
                    "startArrowhead",
                    "endArrowhead",
                    "lastCommittedPoint",
                ] {
                    element.set_raw(key, Value::Null);
                }
            }
            _ => {}
        }
        elements.push(element);
    }
    document.set_elements(elements);
    if family == "shared-image" {
        document
            .set_root(
                "files",
                json!({"shared":{"id":"shared","mimeType":"application/octet-stream","created":1,
            "dataURL":format!("data:application/octet-stream;base64,{}","A".repeat(1024*1024))}}),
            )
            .unwrap();
    }
    document
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    // Cargo test invokes harness-free bench binaries with no flags, while
    // cargo bench supplies --bench. Keep ordinary all-target tests bounded.
    if args.is_empty() || args.iter().any(|arg| arg == "--test") {
        for family in ["shapes", "dense-path", "shared-image", "frame-chain"] {
            let document = fixture(2, family);
            assert_eq!(
                Document::from_slice(&document.to_vec().unwrap()).unwrap(),
                document
            );
            assert!(
                document
                    .validate(Profile::V0_18_1, Purpose::Inspect)
                    .is_valid()
            );
        }
        return;
    }
    let mut sizes = vec![1000, 10000, 100000];
    let mut samples = 3;
    let mut families = vec![
        "shapes".to_owned(),
        "dense-path".to_owned(),
        "shared-image".to_owned(),
    ];
    // Cargo supplies --bench; other arguments are explicit and bounded.
    for arg in args {
        if let Some(v) = arg.strip_prefix("--sizes=") {
            sizes = v
                .split(',')
                .map(|n| n.parse::<usize>().expect("integer size"))
                .collect();
        } else if let Some(v) = arg.strip_prefix("--samples=") {
            samples = v.parse::<usize>().expect("integer samples");
        } else if let Some(v) = arg.strip_prefix("--families=") {
            families = v.split(',').map(str::to_owned).collect();
        } else if arg != "--bench" {
            panic!("unknown argument {arg}");
        }
    }
    assert!((1..=20).contains(&samples));
    assert!(sizes.iter().all(|s| (1..=100000).contains(s)));
    for family in families {
        assert!(["shapes", "dense-path", "shared-image", "frame-chain"].contains(&family.as_str()));
        for &size in &sizes {
            let document = fixture(size, &family);
            let bytes = document.to_vec().unwrap();
            let mut report = json!({"family":family,"size":size,"inputBytes":bytes.len(),"samples":samples,
                "architecture":std::env::consts::ARCH,"os":std::env::consts::OS,
                "profile":"release", "timingIncludesDrop":true});
            report["parse"] = elapsed(samples, || Document::from_slice(black_box(&bytes)).unwrap());
            report["encode"] = elapsed(samples, || black_box(&document).to_vec().unwrap());
            report["validate"] = elapsed(samples, || {
                let result = black_box(&document).validate(Profile::V0_18_1, Purpose::Inspect);
                assert!(result.is_valid(), "benchmark fixture invalid: {result:?}");
                result
            });
            report["project"] = elapsed(samples, || {
                black_box(&document)
                    .project_native(Profile::V0_18_1, ExportMode::Local, "benchmark")
                    .unwrap()
            });
            println!("{report}");
        }
    }
}
