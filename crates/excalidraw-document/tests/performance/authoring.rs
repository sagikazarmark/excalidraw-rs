//! Release workload baseline: cargo bench -p excalidraw-document --bench authoring
//! -- --sizes=100,1000,5000 --samples=3. No args / --test runs a tiny smoke suite.
//! JSON lines retain every sample, including workload size and timing measurements.
use excalidraw_document::{
    ArrowEndpoint, BindingGeometry, Document, Element, ElementId, ElementKind, Error, GroupId,
    IdMap, OpaquePolicy, Profile, Purpose, SceneAuthor, TextContent,
};
use serde_json::{Value, json};
use std::{hint::black_box, time::Instant};

const PROFILE: Profile = Profile::V0_18_1;
const COMPONENT_SIZE: usize = 10;
const ASSET_BYTES: usize = 1024 * 1024;
const ROLES: [&str; COMPONENT_SIZE] = [
    "a", "la", "b", "lb", "c", "lc", "ab", "bc", "asset", "frame",
];

fn id(component: usize, role: &str) -> ElementId {
    ElementId(format!("{component}-{role}"))
}

fn geometry() -> BindingGeometry {
    BindingGeometry::Release {
        focus: 0_i64.into(),
        gap: 1_u64.into(),
        fixed_point: None,
    }
}

fn valid(document: &Document) {
    let report = document.validate(PROFILE, Purpose::Author);
    assert!(
        report.is_valid(),
        "invalid authoring fixture/output: {report:?}"
    );
}

fn count(document: &Document) -> usize {
    document.as_object()["elements"].as_array().unwrap().len()
}

fn by_id<'a>(document: &'a Document, target: &ElementId) -> &'a Value {
    document.as_object()["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == target.0)
        .unwrap()
}

/// Independent workflow cards: three labeled boxes, two bound arrows, a preview
/// (rectangle or shared image), and a frame. All nine children share one group;
/// label follows container, children precede frame, and all references are local.
/// Raw relationship assembly avoids O(n) author transactions during construction.
fn fixture(size: usize, shared_asset: bool) -> Document {
    assert!(size >= COMPONENT_SIZE && size.is_multiple_of(COMPONENT_SIZE));
    let mut document = Document::new("authoring-benchmark");
    if shared_asset {
        // A real SVG data URL, padded to a 1 MiB payload. One resource, many images.
        let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='120' height='60'><rect width='120' height='60' fill='blue'/></svg>";
        let payload = format!("{svg}{}", " ".repeat(ASSET_BYTES - svg.len()));
        document
            .set_root(
                "files",
                json!({"shared": {
                    "id":"shared", "mimeType":"image/svg+xml", "created":1,
                    "dataURL":format!("data:image/svg+xml,{payload}")
                }}),
            )
            .unwrap();
    }
    let mut elements = Vec::with_capacity(size);
    for component in 0..size / COMPONENT_SIZE {
        let base_x = (component % 10 * 800) as i64;
        let base_y = (component / 10 * 300) as i64;
        for role in ROLES {
            let kind = match role {
                "la" | "lb" | "lc" => ElementKind::Text,
                "ab" | "bc" => ElementKind::Arrow,
                "frame" => ElementKind::Frame,
                "asset" if shared_asset => ElementKind::Image,
                _ => ElementKind::Rectangle,
            };
            let mut e = Element::new(kind, PROFILE, id(component, role), 1_u64.into()).unwrap();
            let column = match role {
                "b" | "lb" | "bc" => 1,
                "c" | "lc" => 2,
                _ => 0,
            };
            let label = role.starts_with('l');
            let arrow = matches!(role, "ab" | "bc");
            let dx = if label {
                10
            } else if arrow {
                161
            } else {
                0
            };
            let dy = match role {
                "la" | "lb" | "lc" => 18,
                "ab" | "bc" => 30,
                "asset" => 100,
                _ => 0,
            };
            e.set_raw("x", json!(base_x + column * 220 + dx));
            e.set_raw("y", json!(base_y + dy));
            e.set_raw("width", json!(160));
            e.set_raw("height", json!(60));
            if role == "frame" {
                e.set_raw("x", json!(base_x - 20));
                e.set_raw("y", json!(base_y - 20));
                e.set_raw("width", json!(660));
                e.set_raw("height", json!(200));
                e.set_raw("name", json!(format!("Workflow {component}")));
            } else {
                e.set_raw("frameId", json!(id(component, "frame").0));
                e.set_raw("groupIds", json!([format!("group-{component}")]));
            }
            match role {
                "a" | "b" | "c" => {
                    let mut refs =
                        vec![json!({"id":id(component, &format!("l{role}")).0,"type":"text"})];
                    for arrow in match role {
                        "a" => &["ab"][..],
                        "b" => &["ab", "bc"][..],
                        _ => &["bc"][..],
                    } {
                        refs.push(json!({"id":id(component, arrow).0,"type":"arrow"}));
                    }
                    e.set_raw("boundElements", json!(refs));
                }
                "la" | "lb" | "lc" => {
                    e.set_text_content(TextContent::plain(
                        format!("Step {component} {}", &role[1..]),
                        140_u64.into(),
                        25_u64.into(),
                    ))
                    .unwrap();
                    e.set_raw("containerId", json!(id(component, &role[1..]).0));
                }
                "ab" | "bc" => {
                    e.set_path(vec![
                        [0_i64.into(), 0_i64.into()],
                        [58_i64.into(), 0_i64.into()],
                    ])
                    .unwrap();
                    for (key, target) in [("startBinding", &role[..1]), ("endBinding", &role[1..])]
                    {
                        e.set_raw(
                            key,
                            json!({"elementId":id(component, target).0,"focus":0,"gap":1}),
                        );
                    }
                }
                "asset" if shared_asset => {
                    e.set_raw("fileId", json!("shared"));
                    e.set_raw("status", json!("saved"));
                }
                _ => {}
            }
            elements.push(e);
        }
    }
    document.author(PROFILE).unwrap().insert(elements).unwrap();
    valid(&document);
    assert_eq!(count(&document), size);
    document
}

fn measure(samples: usize, mut sample: impl FnMut() -> f64) -> Value {
    black_box(sample()); // One untimed warm-up, with the same fresh-state setup.
    let raw: Vec<_> = (0..samples).map(|_| sample()).collect();
    let mut sorted = raw.clone();
    sorted.sort_by(f64::total_cmp);
    json!({"samplesMs":raw,"minMs":sorted[0],"medianMs":sorted[samples/2],"maxMs":sorted[samples-1]})
}

fn timed<T>(operation: impl FnOnce() -> T) -> f64 {
    let start = Instant::now();
    drop(black_box(operation()));
    start.elapsed().as_secs_f64() * 1000.
}

#[derive(Clone, Copy)]
enum Edit {
    Replace,
    Bind,
    Rebind,
    TranslateLocal,
    TranslateAll,
    Group,
    Reorder,
    Duplicate,
}

impl Edit {
    fn name(self) -> &'static str {
        match self {
            Self::Replace => "replace_text",
            Self::Bind => "bind_arrow",
            Self::Rebind => "rebind_arrow",
            Self::TranslateLocal => "translate_connected_component",
            Self::TranslateAll => "translate_connected_all",
            Self::Group => "group",
            Self::Reorder => "reorder",
            Self::Duplicate => "duplicate_shared_files",
        }
    }
}

fn edit_sample(source: &Document, edit: Edit, include_session: bool) -> f64 {
    // Reset, selection, mapping and content preparation are outside the timer.
    let mut document = source.clone();
    let components = count(source) / COMPONENT_SIZE;
    let component = (components - 1) / 2;
    let a = id(component, "a");
    let label = id(component, "la");
    let arrow = id(component, "ab");
    let target = id(component, "c");
    let local = vec![a.clone()];
    let all: Vec<_> = (0..components).map(|i| id(i, "frame")).collect();
    let mapping = IdMap {
        elements: ROLES
            .iter()
            .map(|role| {
                let old = id(component, role);
                (old.clone(), ElementId(format!("copy-{}", old.0)))
            })
            .collect(),
        groups: [(
            GroupId(format!("group-{component}")),
            GroupId("copy-group".into()),
        )]
        .into(),
        ..IdMap::default()
    };
    let content = TextContent::plain("Updated workflow step", 150_u64.into(), 25_u64.into());
    let binding = geometry();
    let delta = [17_i64.into(), (-9_i64).into()];
    let group = GroupId("outer-group".into());
    if matches!(edit, Edit::Bind) {
        document
            .author(PROFILE)
            .unwrap()
            .unbind_arrow(&arrow, ArrowEndpoint::End)
            .unwrap();
    }
    let operation = |author: &mut SceneAuthor<'_>| match edit {
        Edit::Replace => author.replace_text(&label, content).unwrap(),
        Edit::Bind | Edit::Rebind => author
            .bind_arrow(&arrow, ArrowEndpoint::End, &target, binding)
            .unwrap(),
        Edit::TranslateLocal => author.translate_connected(&local, delta).unwrap(),
        Edit::TranslateAll => author.translate_connected(&all, delta).unwrap(),
        Edit::Group => author.group(&local, group).unwrap(),
        Edit::Reorder => author.reorder(&local, None).unwrap(),
        Edit::Duplicate => assert!(
            author
                .duplicate(&local, &mapping, delta, OpaquePolicy::Reject)
                .unwrap()
                .is_empty()
        ),
    };
    let ms = if include_session {
        timed(|| operation(&mut document.author(PROFILE).unwrap()))
    } else {
        let mut author = document.author(PROFILE).unwrap();
        timed(|| operation(&mut author))
    };
    // Postconditions and independent validation are outside the timer, including
    // smoke mode. Transactions' own candidate validation remains inside it.
    valid(&document);
    assert_eq!(
        count(&document),
        count(source)
            + if matches!(edit, Edit::Duplicate) {
                COMPONENT_SIZE
            } else {
                0
            }
    );
    match edit {
        Edit::Replace => assert_eq!(by_id(&document, &label)["text"], "Updated workflow step"),
        Edit::Bind | Edit::Rebind => assert_eq!(
            by_id(&document, &arrow)["endBinding"]["elementId"],
            target.0
        ),
        Edit::TranslateLocal | Edit::TranslateAll => {
            let moved = document.as_object()["elements"]
                .as_array()
                .unwrap()
                .iter()
                .zip(source.as_object()["elements"].as_array().unwrap())
                .filter(|(after, before)| after["x"].as_f64() != before["x"].as_f64())
                .count();
            assert_eq!(
                moved,
                if matches!(edit, Edit::TranslateAll) {
                    count(source)
                } else {
                    9
                }
            );
        }
        Edit::Group => assert_eq!(
            by_id(&document, &a)["groupIds"],
            json!([format!("group-{component}"), "outer-group"])
        ),
        Edit::Reorder => assert_eq!(
            document.as_object()["elements"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["id"],
            id(component, "frame").0
        ),
        Edit::Duplicate => {
            assert_eq!(document.as_object()["files"], source.as_object()["files"]);
            assert_eq!(
                by_id(&document, &ElementId(format!("copy-{}", label.0)))["containerId"],
                format!("copy-{}", a.0)
            );
        }
    }
    black_box(&document);
    ms // Dropping the final/reset document is excluded.
}

fn insert_sample(source: &Document, repeated: bool) -> f64 {
    let mut empty = source.clone();
    empty.set_elements(vec![]); // Shared file is already registered in both cases.
    let elements = source.elements().unwrap();
    let batches: Vec<_> = if repeated {
        elements
            .chunks(COMPONENT_SIZE)
            .map(<[Element]>::to_vec)
            .collect()
    } else {
        vec![elements]
    };
    let mut author = empty.author(PROFILE).unwrap();
    let ms = timed(|| {
        for batch in batches {
            author.insert(batch).unwrap();
        }
    });
    valid(author.document());
    assert_eq!(author.document(), source);
    ms
}

fn multi_edit_sample(source: &Document, mixed: bool, batched: bool) -> (f64, Document) {
    // Identical owned inputs and fresh session preparation for both paths. The
    // batch callback itself (including queue allocation/argument copies) is timed.
    let mut document = source.clone();
    let component = (count(source) / COMPONENT_SIZE - 1) / 2;
    let texts: Vec<_> = (0..if mixed { 1 } else { 20 })
        .map(|i| {
            let label = if mixed {
                id(component, "la")
            } else {
                id(i / 3, ["la", "lb", "lc"][i % 3])
            };
            let content = TextContent::plain(
                format!("Updated {}", label.0),
                150_u64.into(),
                25_u64.into(),
            );
            (label, content)
        })
        .collect();
    let arrow = id(component, "ab");
    let target = id(component, "c");
    let local = vec![id(component, "a")];
    let binding = geometry();
    let group = GroupId("outer-group".into());
    let mut author = document.author(PROFILE).unwrap();
    let ms = timed(|| {
        if batched {
            author
                .batch(|batch| {
                    for (label, content) in texts {
                        batch.replace_text(&label, content)?;
                    }
                    if mixed {
                        batch.bind_arrow(&arrow, ArrowEndpoint::End, &target, binding)?;
                        batch.group(&local, group)?;
                    }
                    Ok(())
                })
                .unwrap();
        } else {
            for (label, content) in texts {
                author.replace_text(&label, content).unwrap();
            }
            if mixed {
                author
                    .bind_arrow(&arrow, ArrowEndpoint::End, &target, binding)
                    .unwrap();
                author.group(&local, group).unwrap();
            }
        }
    });
    valid(&document);
    assert_eq!(count(&document), count(source));
    assert_eq!(document.as_object()["files"], source.as_object()["files"]);
    let changed_labels = document.as_object()["elements"]
        .as_array()
        .unwrap()
        .iter()
        .zip(source.as_object()["elements"].as_array().unwrap())
        .filter(|(after, before)| after["text"] != before["text"])
        .count();
    assert_eq!(changed_labels, if mixed { 1 } else { 20 });
    if mixed {
        assert_eq!(
            by_id(&document, &arrow)["endBinding"]["elementId"],
            target.0
        );
        assert_eq!(
            by_id(&document, &local[0])["groupIds"],
            json!([format!("group-{component}"), "outer-group"])
        );
    }
    (ms, document)
}

/// Untimed rollback checks on every realistic fixture, including its shared file.
fn check_batch_rollback(source: &Document) {
    let mut document = source.clone();
    let mut author = document.author(PROFILE).unwrap();
    for failure in 0..3 {
        let result = author.batch(|batch| {
            batch.replace_text(
                &id(0, "la"),
                TextContent::plain("Discarded", 150_u64.into(), 25_u64.into()),
            )?;
            batch.bind_arrow(&id(0, "ab"), ArrowEndpoint::End, &id(0, "c"), geometry())?;
            batch.group(&[id(0, "a")], GroupId("discarded-group".into()))?;
            match failure {
                0 => Err(Error {
                    path: "/callback".into(),
                    message: "cancelled".into(),
                }),
                1 => batch.replace_text(
                    &ElementId("missing-label".into()),
                    TextContent::plain("Invalid", 150_u64.into(), 25_u64.into()),
                ),
                // Execution succeeds, but two labels on one box fail validation.
                _ => batch.bind_label(&id(0, "lb"), &id(0, "a")),
            }
        });
        assert!(result.is_err());
        assert_eq!(author.document(), source);
        valid(author.document());
    }
    // The same session remains usable, and a failing single edit still rolls back.
    author
        .batch(|batch| {
            batch.replace_text(
                &id(0, "la"),
                TextContent::plain("Recovered", 150_u64.into(), 25_u64.into()),
            )
        })
        .unwrap();
    assert_eq!(by_id(author.document(), &id(0, "la"))["text"], "Recovered");
    let before = author.document().clone();
    assert!(
        author
            .bind_arrow(
                &id(0, "ab"),
                ArrowEndpoint::End,
                &id(0, "missing"),
                geometry()
            )
            .is_err()
    );
    assert_eq!(author.document(), &before);
    valid(author.document());
}

fn run(size: usize, shared_asset: bool, samples: usize, print: bool) {
    let source = fixture(size, shared_asset);
    let mut report = json!({
        "family":if shared_asset { "framed-workflows-shared-asset" } else { "framed-workflows" },
        "size":size,"components":size/COMPONENT_SIZE,"inputBytes":source.to_vec().unwrap().len(),
        "sharedAssetPayloadBytes":if shared_asset { ASSET_BYTES } else { 0 },
        "samples":samples,"warmups":1,"profile":if cfg!(debug_assertions) { "debug" } else { "release" },
        "architecture":std::env::consts::ARCH,"os":std::env::consts::OS,
        "timing":{"fixtureAndResetIncluded":false,"finalDocumentDropIncluded":false,
            "transactionValidationIncluded":true,"sessionIncludedUnlessNamed":false,
            "baselineOutputDropIncluded":true,"batchQueueConstructionIncluded":true,
            "multiEditInputPreparationIncluded":false},
        "multiEditWorkloads":{"text_edits_20":{"distinctLabels":20,"edits":20},
            "mixed_text_rebind_group":{"distinctLabels":1,"edits":3}},
        "operations":{}
    });
    let ops = &mut report["operations"];
    ops["clone"] = measure(samples, || timed(|| source.clone()));
    ops["validate_author"] = measure(samples, || {
        timed(|| {
            let report = source.validate(PROFILE, Purpose::Author);
            assert!(report.is_valid());
            report
        })
    });
    ops["clone_validate_author"] = measure(samples, || {
        timed(|| {
            let copy = source.clone();
            valid(&copy);
            copy
        })
    });
    ops["author_session"] = measure(samples, || {
        let mut copy = source.clone();
        timed(|| {
            black_box(copy.author(PROFILE).unwrap());
        })
    });
    ops["insert_batch"] = measure(samples, || insert_sample(&source, false));
    if size <= 1000 {
        ops["insert_repeated_components"] = measure(samples, || insert_sample(&source, true));
    }
    for edit in [
        Edit::Replace,
        Edit::Bind,
        Edit::Rebind,
        Edit::TranslateLocal,
        Edit::TranslateAll,
        Edit::Group,
        Edit::Reorder,
        Edit::Duplicate,
    ] {
        ops[edit.name()] = measure(samples, || edit_sample(&source, edit, false));
    }
    ops["replace_text_session_included"] =
        measure(samples, || edit_sample(&source, Edit::Replace, true));
    for (mixed, name) in [(false, "text_edits_20"), (true, "mixed_text_rebind_group")] {
        // Reference execution and all full-document comparisons are untimed.
        let (_, expected) = multi_edit_sample(&source, mixed, false);
        for (batched, mode) in [(false, "sequential"), (true, "batch")] {
            ops[format!("{name}_{mode}")] = measure(samples, || {
                let (ms, actual) = multi_edit_sample(&source, mixed, batched);
                assert_eq!(actual, expected);
                black_box(&actual);
                ms
            });
        }
    }
    check_batch_rollback(&source);
    if print {
        println!("{report}");
    }
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|arg| arg == "--test") {
        for shared in [false, true] {
            run(100, shared, 1, false);
        }
        return;
    }
    let mut sizes = vec![100, 1000, 5000];
    let mut samples = 3;
    for arg in args {
        if let Some(value) = arg.strip_prefix("--sizes=") {
            sizes = value
                .split(',')
                .map(|n| n.parse::<usize>().expect("integer size"))
                .collect();
        } else if let Some(value) = arg.strip_prefix("--samples=") {
            samples = value.parse::<usize>().expect("integer samples");
        } else {
            assert_eq!(arg, "--bench", "unknown argument");
        }
    }
    assert!((1..=20).contains(&samples));
    assert!(
        !sizes.is_empty()
            && sizes
                .iter()
                .all(|s| (100..=5000).contains(s) && s.is_multiple_of(COMPONENT_SIZE))
    );
    for shared in [false, true] {
        for &size in &sizes {
            run(size, shared, samples, true);
        }
    }
}
