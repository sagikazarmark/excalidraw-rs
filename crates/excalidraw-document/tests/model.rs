#![recursion_limit = "512"]
use excalidraw_document::*;
use serde_json::{Value, json};

fn check<O: Clone + std::fmt::Debug + PartialEq, T: WireValue>(record: &Record<O>, key: Key<O, T>) {
    let original = record.as_object()[key.name].clone();
    let mut edited = record.clone();
    match record.get(key).unwrap() {
        Field::Value(value) => edited.set(key, value).unwrap(),
        Field::Null => edited.set_null(key),
        Field::Missing => panic!("fixture missing {}", key.name),
    }
    assert_eq!(edited.as_object()[key.name], original, "{}", key.name);
    assert_eq!(
        &edited, record,
        "typed access must preserve neighboring fields"
    );
}

#[test]
fn every_common_and_variant_field_has_typed_access_without_losing_extensions() {
    // Independent field values transcribed from the pinned source inventory.
    // This deliberately mixed record tests the wire vocabulary, not authored validity.
    let value = json!({
        "type":"arrow","id":"001","x":-1.25,"y":2.75,"width":123.5,"height":44.25,"angle":0.75,
        "strokeColor":"rgba(1,2,3,.5)","backgroundColor":"transparent","fillStyle":"zigzag","strokeWidth":1.5,"strokeStyle":"dotted",
        "roundness":{"type":3,"value":17,"future":true},"roughness":0.7,"opacity":12.5,"seed":7,"version":13,"versionNonce":23,
        "index":"a1V","isDeleted":true,"groupIds":["inner","outer"],"frameId":"frame","boundElements":[{"id":"label","type":"text","future":42}],
        "updated":1234,"created":1200,"link":"https://example.test/?element=label","locked":true,"customData":{"future":{"nested":[1,null]}},
        "fontSize":21.5,"fontFamily":1000,"baseFontSize":24,"text":"a\nβ","originalText":"a β","textAlign":"right","verticalAlign":"bottom",
        "containerId":"box","autoResize":false,"lineHeight":1.15,"labelPosition":0.3,
        "points":[[0,0],[-1.25,20],[50,20]],"startBinding":{"elementId":"box","focus":0.2,"gap":8,"fixedPoint":[-1,2],"mode":"skip","future":true},
        "endBinding":{"elementId":"other","fixedPoint":[1,0],"mode":"orbit"},"startArrowhead":"bar","endArrowhead":"cardinality_zero_or_many",
        "lastCommittedPoint":[1,2],"polygon":true,"elbowed":true,"fixedSegments":[{"start":[0,0],"end":[2,0],"index":1,"future":true}],
        "startIsSpecial":true,"endIsSpecial":false,"pressures":[0.1,0.8,0.2],"simulatePressure":false,"strokeOptions":{"variability":"constant","streamline":0.7,"future":true},
        "fileId":"asset","status":"saved","scale":[-1,1],"crop":{"x":1,"y":2,"width":20,"height":30,"naturalWidth":40,"naturalHeight":60,"future":true},
        "name":"Frame name","baseHeight":120,"futureElement":{"a":42}
    });
    let record = Element::from_object(value.as_object().unwrap().clone());
    macro_rules! fields { ($($name:ident),*) => { $(check(&record,element::$name);)* }; }
    fields!(
        TYPE,
        ID,
        X,
        Y,
        WIDTH,
        HEIGHT,
        ANGLE,
        STROKE_COLOR,
        BACKGROUND_COLOR,
        FILL_STYLE,
        STROKE_WIDTH,
        STROKE_STYLE,
        ROUNDNESS,
        ROUGHNESS,
        OPACITY,
        SEED,
        VERSION,
        VERSION_NONCE,
        INDEX,
        IS_DELETED,
        GROUP_IDS,
        FRAME_ID,
        BOUND_ELEMENTS,
        UPDATED,
        CREATED,
        LINK,
        LOCKED,
        CUSTOM_DATA,
        FONT_SIZE,
        FONT_FAMILY,
        BASE_FONT_SIZE,
        TEXT,
        ORIGINAL_TEXT,
        TEXT_ALIGN,
        VERTICAL_ALIGN,
        CONTAINER_ID,
        AUTO_RESIZE,
        LINE_HEIGHT,
        LABEL_POSITION,
        POINTS,
        START_BINDING,
        END_BINDING,
        START_ARROWHEAD,
        END_ARROWHEAD,
        LAST_COMMITTED_POINT,
        POLYGON,
        ELBOWED,
        FIXED_SEGMENTS,
        START_IS_SPECIAL,
        END_IS_SPECIAL,
        PRESSURES,
        SIMULATE_PRESSURE,
        STROKE_OPTIONS,
        FILE_ID,
        STATUS,
        SCALE,
        CROP,
        NAME,
        BASE_HEIGHT
    );
    assert_eq!(element::FIELDS.len(), value.as_object().unwrap().len() - 1);
}

#[test]
fn nested_objects_and_all_file_appstate_library_fields_have_typed_access() {
    macro_rules! record {
        ($ty:ty,$value:expr,$module:ident,[$($field:ident),*]) => {{
            let value = $value;
            let record = <$ty>::from_object(value.as_object().unwrap().clone());
            $(check(&record,$module::$field);)*
            assert_eq!($module::FIELDS.len(),value.as_object().unwrap().len()-1);
        }}
    }
    record!(
        Binding,
        json!({"elementId":"e","focus":0.25,"gap":3,"fixedPoint":[0.1,0.2],"mode":"inside","extra":true}),
        binding,
        [ELEMENT_ID, FOCUS, GAP, FIXED_POINT, MODE]
    );
    record!(
        BoundElement,
        json!({"id":"e","type":"arrow","extra":true}),
        bound_element,
        [ID, TYPE]
    );
    record!(
        Roundness,
        json!({"type":2,"value":0.4,"extra":true}),
        roundness,
        [TYPE, VALUE]
    );
    record!(
        FixedSegment,
        json!({"start":[1,2],"end":[3,4],"index":2,"extra":true}),
        fixed_segment,
        [START, END, INDEX]
    );
    record!(
        StrokeOptions,
        json!({"variability":"variable","streamline":0.2,"extra":true}),
        stroke_options,
        [VARIABILITY, STREAMLINE]
    );
    record!(
        ImageCrop,
        json!({"x":1,"y":2,"width":3,"height":4,"naturalWidth":5,"naturalHeight":6,"extra":true}),
        image_crop,
        [X, Y, WIDTH, HEIGHT, NATURAL_WIDTH, NATURAL_HEIGHT]
    );
    record!(
        GenerationData,
        json!({"status":"error","html":"<p>text</p>","code":"ERR_GENERATION_INTERRUPTED","message":"test","extra":true}),
        generation_data,
        [STATUS, HTML, CODE, MESSAGE]
    );
    record!(
        BinaryFile,
        json!({"id":"asset","mimeType":"image/avif","dataURL":"data:image/avif;base64,AQ==","created":1,"lastRetrieved":2,"version":3,"extra":true}),
        binary_file,
        [ID, MIME_TYPE, DATA_URL, CREATED, LAST_RETRIEVED, VERSION]
    );
    record!(
        AppState,
        json!({"gridSize":17,"gridStep":3,"gridModeEnabled":true,"viewBackgroundColor":"#123456","lockedMultiSelections":{"g":true},"extra":true}),
        app_state,
        [
            GRID_SIZE,
            GRID_STEP,
            GRID_MODE_ENABLED,
            VIEW_BACKGROUND_COLOR,
            LOCKED_MULTI_SELECTIONS
        ]
    );
    record!(
        LibraryItem,
        json!({"id":"item","status":"published","created":1,"elements":[{"type":"rectangle","future":true}],"name":"item name","error":"diagnostic","extra":true}),
        library_item,
        [ID, STATUS, CREATED, ELEMENTS, NAME, ERROR]
    );
}

#[test]
fn every_persisted_kind_can_be_authored_in_its_profile() {
    let kinds = [
        "rectangle",
        "diamond",
        "ellipse",
        "text",
        "line",
        "arrow",
        "freedraw",
        "image",
        "frame",
        "magicframe",
        "iframe",
        "embeddable",
        "stickynote",
    ];
    for profile in [Profile::V0_18_1, Profile::SnapshotAfa3a653] {
        for kind in kinds {
            let result = Element::new(
                ElementKind::from_wire(kind),
                profile,
                ElementId::from("test"),
                Number::from(1_u64),
            );
            if profile == Profile::V0_18_1 && kind == "stickynote" {
                assert!(result.is_err());
                continue;
            }
            let element = result.unwrap();
            assert_eq!(element.as_object()["type"], kind);
            let mut document = Document::new("test");
            document.set_elements(vec![element]);
            let report = document.validate(profile, Purpose::Author);
            assert!(report.is_valid(), "{kind}: {report:?}");
        }
    }
}

#[test]
fn open_enums_retain_every_wire_spelling_and_unknown_values() {
    macro_rules! values {
        ($ty:ty,[$($value:literal),*]) => { $(assert_eq!(serde_json::to_value(serde_json::from_value::<$ty>(json!($value)).unwrap()).unwrap(),json!($value));)* };
    }
    values!(
        FillStyle,
        ["solid", "hachure", "cross-hatch", "zigzag", "future"]
    );
    values!(
        Arrowhead,
        [
            "arrow",
            "bar",
            "circle",
            "circle_outline",
            "triangle",
            "triangle_outline",
            "diamond",
            "diamond_outline",
            "dot",
            "crowfoot_one",
            "crowfoot_many",
            "crowfoot_one_or_many",
            "cardinality_one",
            "cardinality_many",
            "cardinality_one_or_many",
            "cardinality_exactly_one",
            "cardinality_zero_or_one",
            "cardinality_zero_or_many",
            "future"
        ]
    );
    values!(
        MimeType,
        [
            "image/svg+xml",
            "image/png",
            "image/jpeg",
            "image/gif",
            "image/webp",
            "image/bmp",
            "image/x-icon",
            "image/avif",
            "image/jfif",
            "application/octet-stream",
            "future"
        ]
    );
    values!(StrokeStyle, ["solid", "dashed", "dotted", "future"]);
    values!(BindMode, ["inside", "orbit", "skip", "future"]);
    values!(GenerationStatus, ["pending", "done", "error", "future"]);
    let mut element = Element::default();
    element.set_null(element::END_ARROWHEAD);
    assert_eq!(
        serde_json::to_value(&element).unwrap()["endArrowhead"],
        Value::Null
    );
}
