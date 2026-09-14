use excalidraw_document::{Document, Profile, Purpose};
use serde_json::{Value, json};

fn document(elements: Vec<Value>) -> Document {
    Document::from_value(json!({"type":"excalidraw","elements":elements})).unwrap()
}

#[test]
fn frame_cycles_include_incoming_paths_but_not_disconnected_or_ambiguous_paths() {
    let scene = document(vec![
        json!({"type":"frame","id":"a","frameId":"b"}),
        json!({"type":"frame","id":"b","frameId":"a"}),
        json!({"type":"rectangle","id":"tail","frameId":"a"}),
        json!({"type":"frame","id":"independent"}),
        json!({"type":"rectangle","id":"ok","frameId":"independent"}),
        json!({"type":"frame","id":"self","frameId":"self"}),
        json!({"type":"frame","id":"duplicate","frameId":"a"}),
        json!({"type":"frame","id":"duplicate"}),
        json!({"type":"rectangle","id":"ambiguous","frameId":"duplicate"}),
    ]);
    let before = scene.clone();
    let report = scene.validate(Profile::V0_18_1, Purpose::Inspect);
    let paths: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "frame-cycle")
        .map(|d| d.path.as_str())
        .collect();
    assert_eq!(
        paths,
        vec![
            "/elements/0/frameId",
            "/elements/1/frameId",
            "/elements/2/frameId",
            "/elements/5/frameId",
            "/elements/6/frameId"
        ]
    );
    assert_eq!(scene, before);
}

#[test]
fn long_frame_chains_validate_without_recursive_stack_growth() {
    let elements=(0..2000).map(|i|json!({"type":"frame","id":format!("f{i}"),"frameId":if i==0 {Value::Null} else {json!(format!("f{}",i-1))}})).collect();
    let scene = document(elements);
    assert!(
        scene
            .validate(Profile::V0_18_1, Purpose::Inspect)
            .diagnostics
            .is_empty()
    );
}
