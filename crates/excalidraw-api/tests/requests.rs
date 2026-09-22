//! Golden request shapes for every documented operation, plus path escaping.
//! None of these tests performs I/O.
use excalidraw_api::{
    CollectionId, Error, InviteId, LogOperation, LogQuery, Method, Operation, PageRequest, SceneId,
    UserId, model, op, plus,
    scene_content::{self, ElementIds, PatchFields},
};
use excalidraw_document::Document;
use serde_json::{Value, json};

fn scene() -> SceneId {
    SceneId::new("scene-1").unwrap()
}
fn collection() -> CollectionId {
    CollectionId::new("col-1").unwrap()
}

fn check<O: Operation>(op: O, method: Method, url: &str) -> Option<Value> {
    let request = op.request().expect("request builds");
    assert_eq!(request.method, method, "method for {url}");
    assert_eq!(request.relative_url(), url);
    request
        .body
        .map(|body| serde_json::from_slice(&body).expect("body is JSON"))
}

// ---------------------------------------------------------------- scene content

#[test]
fn scene_content_requests() {
    check(
        op::GetSceneContent { scene: scene() },
        Method::Get,
        "/scenes/scene-1/content",
    );

    let document = Document::from_value(json!({
        "type": "excalidraw", "version": 2, "source": "excaliplot",
        "elements": [], "appState": { "viewBackgroundColor": "#ffffff" }, "files": {}
    }))
    .unwrap();
    let (body, _) = scene_content::replacement_from_document(
        &document,
        &scene_content::PlusProjection::strict(),
    )
    .unwrap();
    let sent = check(
        op::ReplaceSceneContent {
            scene: scene(),
            body,
        },
        Method::Put,
        "/scenes/scene-1/content",
    )
    .expect("PUT has a body");
    assert_eq!(sent["type"], "excalidraw");

    let patch = scene_content::patch_from(
        &document,
        PatchFields::elements(),
        ElementIds::AllowProvisional,
    )
    .unwrap();
    let sent = check(
        op::PatchSceneContent {
            scene: scene(),
            body: patch,
        },
        Method::Patch,
        "/scenes/scene-1/content",
    )
    .expect("PATCH has a body");
    assert_eq!(sent, json!({ "elements": [] }));
}

// ----------------------------------------------------------------------- scenes

#[test]
fn scene_requests() {
    check(op::ListScenes::default(), Method::Get, "/scenes");

    check(
        op::ListScenes {
            page: PageRequest::new().limit(50).offset(100),
            collection: Some(collection()),
        },
        Method::Get,
        "/scenes?limit=50&offset=100&collectionId=col-1",
    );

    let sent = check(
        op::CreateScene {
            scene: model::NewScene::new("Q3 plan", collection()).pinned(true),
        },
        Method::Post,
        "/scenes",
    )
    .unwrap();
    assert_eq!(
        sent,
        json!({ "name": "Q3 plan", "pinned": true, "collectionId": "col-1" }),
        "pinned is required by the schema and is always sent"
    );

    check(
        op::GetScene { scene: scene() },
        Method::Get,
        "/scenes/scene-1",
    );

    let sent = check(
        op::UpdateScene {
            scene: scene(),
            patch: model::ScenePatch::new().name("Renamed"),
        },
        Method::Patch,
        "/scenes/scene-1",
    )
    .unwrap();
    assert_eq!(
        sent,
        json!({ "name": "Renamed" }),
        "omitted fields stay out"
    );

    check(
        op::DeleteScene { scene: scene() },
        Method::Delete,
        "/scenes/scene-1",
    );
}

#[test]
fn creating_a_scene_without_a_collection_is_refused() {
    let refused = op::CreateScene {
        scene: model::NewScene::in_path("no collection"),
    }
    .request();
    assert!(matches!(refused, Err(Error::Invalid { what, .. }) if what == "scene collection"));
}

#[test]
fn an_empty_scene_patch_is_refused() {
    let refused = op::UpdateScene {
        scene: scene(),
        patch: model::ScenePatch::new(),
    }
    .request();
    assert!(matches!(refused, Err(Error::Invalid { what, .. }) if what == "scene patch"));
}

// ------------------------------------------------------------------ collections

#[test]
fn collection_requests() {
    check(op::ListCollections::default(), Method::Get, "/collections");

    let sent = check(
        op::CreateCollection {
            collection: model::NewCollection::new("Design"),
        },
        Method::Post,
        "/collections",
    )
    .unwrap();
    assert_eq!(sent, json!({ "name": "Design" }));

    check(
        op::GetCollection {
            collection: collection(),
        },
        Method::Get,
        "/collections/col-1",
    );

    let sent = check(
        op::UpdateCollection {
            collection: collection(),
            patch: model::CollectionPatch::name("Renamed"),
        },
        Method::Patch,
        "/collections/col-1",
    )
    .unwrap();
    assert_eq!(sent, json!({ "name": "Renamed" }));

    check(
        op::DeleteCollection {
            collection: collection(),
        },
        Method::Delete,
        "/collections/col-1",
    );

    check(
        op::ListCollectionScenes {
            collection: collection(),
            page: PageRequest::new().limit(10),
        },
        Method::Get,
        "/collections/col-1/scenes?limit=10",
    );

    let sent = check(
        op::CreateSceneInCollection {
            collection: collection(),
            scene: model::NewScene::in_path("Nested"),
        },
        Method::Post,
        "/collections/col-1/scenes",
    )
    .unwrap();
    assert_eq!(
        sent,
        json!({ "name": "Nested", "pinned": false }),
        "the collection comes from the path, not the body"
    );
}

#[test]
fn the_private_collection_is_addressable_but_not_mutable() {
    check(
        op::GetCollection {
            collection: CollectionId::private(),
        },
        Method::Get,
        "/collections/private",
    );
    check(
        op::ListScenes {
            page: PageRequest::default(),
            collection: Some(CollectionId::private()),
        },
        Method::Get,
        "/scenes?collectionId=private",
    );

    for refused in [
        op::UpdateCollection {
            collection: CollectionId::private(),
            patch: model::CollectionPatch::name("nope"),
        }
        .request()
        .err(),
        op::DeleteCollection {
            collection: CollectionId::private(),
        }
        .request()
        .err(),
    ] {
        assert!(matches!(refused, Some(Error::Invalid { what, .. }) if what == "collection"));
    }
}

// --------------------------------------------------------- workspace and users

#[test]
fn workspace_and_user_requests() {
    check(op::GetWorkspace, Method::Get, "/workspaces");

    let sent = check(
        op::UpdateWorkspace {
            patch: model::WorkspacePatch::new().name("Studio").clear_picture(),
        },
        Method::Patch,
        "/workspaces",
    )
    .unwrap();
    assert_eq!(
        sent,
        json!({ "name": "Studio", "picture": null }),
        "clearing sends an explicit null; omitting sends nothing"
    );

    check(op::ListUsers::default(), Method::Get, "/workspaces/users");
    check(
        op::GetUser {
            user: UserId::new("u-1").unwrap(),
        },
        Method::Get,
        "/workspaces/users/u-1",
    );

    let sent = check(
        op::UpdateUser {
            user: UserId::new("u-1").unwrap(),
            patch: model::UserPatch::new().role(model::Role::Admin),
        },
        Method::Patch,
        "/workspaces/users/u-1",
    )
    .unwrap();
    assert_eq!(sent, json!({ "role": "admin" }));

    check(
        op::DeleteUser {
            user: UserId::new("u-1").unwrap(),
        },
        Method::Delete,
        "/workspaces/users/u-1",
    );
}

#[test]
fn a_role_read_as_unknown_cannot_be_written_back() {
    let refused = op::UpdateUser {
        user: UserId::new("u-1").unwrap(),
        patch: model::UserPatch::new().role(model::Role::Unknown("owner".into())),
    }
    .request();
    assert!(matches!(refused, Err(Error::Invalid { what, .. }) if what == "user role"));
}

#[test]
fn preferences_read_as_unknown_cannot_be_written_back() {
    let unset = model::UserPreferences::default;
    for (preferences, field) in [
        (
            model::UserPreferences {
                scene_order: Some(model::SceneOrder::Unknown("random".into())),
                ..unset()
            },
            "user preference sceneOrder",
        ),
        (
            model::UserPreferences {
                initial_redirect: Some(model::InitialRedirect::Unknown("home".into())),
                ..unset()
            },
            "user preference initialRedirect",
        ),
        (
            model::UserPreferences {
                theme: Some(model::Theme::Unknown("bogus".into())),
                ..unset()
            },
            "user preference theme",
        ),
        (
            model::UserPreferences {
                locale: Some(model::Locale::Unknown("xx".into())),
                ..unset()
            },
            "user preference locale",
        ),
    ] {
        let refused = op::UpdateUser {
            user: UserId::new("u-1").unwrap(),
            patch: model::UserPatch::new().preferences(preferences),
        }
        .request();
        assert!(
            matches!(refused, Err(Error::Invalid { what, .. }) if what == field),
            "{field}: {refused:?}"
        );
    }

    let listed = model::UserPreferences {
        theme: Some(model::Theme::Dark),
        locale: Some(model::Locale::De),
        ..unset()
    };
    assert!(
        op::UpdateUser {
            user: UserId::new("u-1").unwrap(),
            patch: model::UserPatch::new().preferences(listed),
        }
        .request()
        .is_ok()
    );
}

// ------------------------------------------------------------- invites and logs

#[test]
fn invite_requests() {
    check(
        op::ListInvites::default(),
        Method::Get,
        "/workspaces/invites",
    );

    let sent = check(
        op::CreateInvite {
            invite: model::NewInvite::email(model::Role::Member, "a@example.com"),
        },
        Method::Post,
        "/workspaces/invites",
    )
    .unwrap();
    assert_eq!(sent, json!({ "role": "member", "email": "a@example.com" }));

    let sent = check(
        op::CreateInvite {
            invite: model::NewInvite::link(model::Role::Admin)
                .max_uses(model::MaxUses::Unlimited)
                .restricted_domains(vec!["example.com".into()])
                .into(),
        },
        Method::Post,
        "/workspaces/invites",
    )
    .unwrap();
    assert_eq!(
        sent,
        json!({
            "role": "admin",
            "maxUses": "unlimited",
            "restrictedDomains": ["example.com"]
        }),
        "the link arm never carries an email key"
    );

    for uses in [0, 9_007_199_254_740_992] {
        let refused = op::CreateInvite {
            invite: model::NewInvite::link(model::Role::Member)
                .max_uses(model::MaxUses::Limited(uses))
                .into(),
        }
        .request();
        assert!(
            matches!(refused, Err(Error::Invalid { what, .. }) if what == "invite max uses"),
            "create sent maxUses {uses}"
        );
        let refused = op::UpdateInvite {
            invite: InviteId::new("i-1").unwrap(),
            patch: model::InvitePatch::new().max_uses(model::MaxUses::Limited(uses)),
        }
        .request();
        assert!(
            matches!(refused, Err(Error::Invalid { what, .. }) if what == "invite max uses"),
            "update sent maxUses {uses}"
        );
    }

    let sent = check(
        op::UpdateInvite {
            invite: InviteId::new("i-1").unwrap(),
            patch: model::InvitePatch::new().max_uses(model::MaxUses::Limited(5)),
        },
        Method::Patch,
        "/workspaces/invites/i-1",
    )
    .unwrap();
    assert_eq!(sent, json!({ "maxUses": 5 }));

    check(
        op::GetInvite {
            invite: InviteId::new("i-1").unwrap(),
        },
        Method::Get,
        "/workspaces/invites/i-1",
    );
    check(
        op::DeleteInvite {
            invite: InviteId::new("i-1").unwrap(),
        },
        Method::Delete,
        "/workspaces/invites/i-1",
    );
}

#[test]
fn log_requests_use_their_own_query_shape() {
    check(op::GetLogs::default(), Method::Get, "/logs");

    check(
        op::GetLogs {
            query: LogQuery {
                limit: Some(25),
                cursor: Some("1234567890".into()),
                user: Some("user123".into()),
                action: Some("workspace".into()),
                operation: Some(LogOperation::Create),
                date_from: Some("2024-01-01".into()),
                date_to: Some("2024-12-31".into()),
                ..LogQuery::new()
            },
        },
        Method::Get,
        "/logs?limit=25&cursor=1234567890&user=user123&action=workspace\
         &operation=create&dateFrom=2024-01-01&dateTo=2024-12-31",
    );
}

#[test]
fn an_undocumented_log_operation_filter_is_refused() {
    let refused = op::GetLogs {
        query: LogQuery {
            operation: Some(LogOperation::Unknown("purge".into())),
            ..LogQuery::new()
        },
    }
    .request();
    assert!(matches!(refused, Err(Error::Invalid { what, .. }) if what == "log operation filter"));
}

// ------------------------------------------------------------------- validation

#[test]
fn opaque_ids_cannot_escape_their_path_segment() {
    let hostile = SceneId::new("../../workspaces/users").unwrap();
    let request = op::GetSceneContent { scene: hostile }.request().unwrap();
    assert_eq!(
        request.path, "/scenes/..%2F..%2Fworkspaces%2Fusers/content",
        "traversal is encoded, not resolved"
    );
    assert!(!request.path.contains("/workspaces/"));

    for raw in ["a b", "a?b", "a#b", "a%b", "a/b"] {
        let request = op::GetScene {
            scene: SceneId::new(raw).unwrap(),
        }
        .request()
        .unwrap();
        let encoded = request.path.strip_prefix("/scenes/").unwrap();
        assert!(
            !encoded.contains(['/', '?', '#', ' ']),
            "{raw} encoded to {encoded}"
        );
    }
}

#[test]
fn empty_identifiers_are_refused_before_a_request_exists() {
    assert!(SceneId::new("").is_err());
    assert!(CollectionId::new("").is_err());
    assert!(UserId::new("").is_err());
    assert!(InviteId::new("").is_err());
}

#[test]
fn dot_segment_identifiers_are_refused_because_url_parsing_resolves_them() {
    // `/workspaces/users/..` resolves to `/workspaces/`, and percent-encoding
    // cannot help: URL parsing treats `%2E%2E` as `..` too.
    for dots in [".", ".."] {
        assert!(SceneId::new(dots).is_err(), "{dots}");
        assert!(CollectionId::new(dots).is_err(), "{dots}");
        assert!(UserId::new(dots).is_err(), "{dots}");
        assert!(InviteId::new(dots).is_err(), "{dots}");
        assert!(
            serde_json::from_value::<UserId>(json!(dots)).is_err(),
            "{dots}"
        );
    }
    // Only a whole segment of dots resolves; anything longer is an ordinary id.
    for id in ["...", ".a", "a.."] {
        assert!(SceneId::new(id).is_ok(), "{id}");
    }
    assert_eq!(
        serde_json::from_value::<SceneId>(json!("scene-1")).unwrap(),
        scene()
    );
    assert_eq!(serde_json::to_value(scene()).unwrap(), json!("scene-1"));
}

#[test]
fn pagination_bounds_are_checked_locally() {
    for bad in [PageRequest::new().limit(0), PageRequest::new().limit(101)] {
        let refused = op::ListScenes {
            page: bad,
            collection: None,
        }
        .request();
        assert!(matches!(refused, Err(Error::Invalid { what, .. }) if what == "pagination limit"));
    }

    let refused = op::ListScenes {
        page: PageRequest::new().offset(9_007_199_254_740_992),
        collection: None,
    }
    .request();
    assert!(matches!(refused, Err(Error::Invalid { what, .. }) if what == "pagination offset"));
}

#[test]
fn every_body_carrying_request_is_json() {
    // This checked three operations under a name that claims all of them, so a
    // new op sending a non-JSON or unexpected body would not have been noticed.
    // It now sweeps the shared list.
    //
    // It does not prove each body was validated: `validate()` is called by hand
    // in each op with no type-level link to "has a body". That gap is real and
    // a test cannot close it from out here.
    for built in all_ops::all() {
        match &built.body {
            Some(body) => {
                assert!(
                    matches!(built.method, Method::Post | Method::Put | Method::Patch),
                    "{} carries a body on {}",
                    built.name,
                    built.method.as_str()
                );
                serde_json::from_slice::<Value>(body)
                    .unwrap_or_else(|e| panic!("{}'s body is not JSON: {e}", built.name));
            }
            None => assert!(
                matches!(built.method, Method::Get | Method::Delete),
                "{} sends {} with no body",
                built.name,
                built.method.as_str()
            ),
        }
    }
}

#[test]
fn plus_bodies_report_support_unconfirmed_element_paths() {
    // The published element schema is `{}`; this reports documentation gaps, not
    // server rejections.
    let document = Document::from_value(json!({
        "type": "excalidraw", "version": 2, "source": "x",
        "elements": [{ "type": "stickynote", "id": "s1" }],
        "appState": { "viewBackgroundColor": "#fff" }, "files": {}
    }))
    .unwrap();
    let body = plus::ReplaceSceneContent::new(document).unwrap();
    assert_eq!(body.unconfirmed_element_paths(), vec!["/elements/0/type"]);
}

#[test]
fn combining_log_cursor_and_page_is_refused_locally() {
    // Confirmed 2026-09-21: the service returns 500 for both parameters, so the
    // crate refuses before sending rather than surfacing a server error.
    let refused = op::GetLogs {
        query: LogQuery {
            cursor: Some("abc".into()),
            page: Some("1".into()),
            ..LogQuery::new()
        },
    }
    .request();
    assert!(matches!(refused, Err(Error::Invalid { what, .. }) if what == "log pagination"));

    // Either one alone is fine.
    for query in [
        LogQuery {
            cursor: Some("abc".into()),
            ..LogQuery::new()
        },
        LogQuery {
            page: Some("1".into()),
            ..LogQuery::new()
        },
    ] {
        assert!(op::GetLogs { query }.request().is_ok());
    }
}

mod all_ops;
