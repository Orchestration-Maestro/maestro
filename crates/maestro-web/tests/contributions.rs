//! Contribution registration, lookup, admission and descriptor records.
#![cfg(test)]
use maestro_web::{
    contributions::{
        BrowserContribution, ContributionDescriptor, ContributionFailure, ContributionView, admit,
        register_contribution, route_contribution, slot_contributions, slot_input,
    },
    dom::{DomContent, DomView},
    tools::types::ToolUiError,
};
use serde_json::{Value, json};
use std::rc::Rc;

/// A view whose identity, not its content, is observed by registry lookups.
struct Marker;
impl ContributionView for Marker {
    fn content(&self, _: &Value) -> Box<dyn DomContent> {
        Box::new(Unmounted)
    }
}
/// Content that registry lookups never mount.
struct Unmounted;
impl DomContent for Unmounted {
    fn mount(&self, _: &DomView) -> Result<web_sys::Node, ToolUiError> {
        Err(ToolUiError::Presentation(
            "registry lookups do not mount".into(),
        ))
    }
    fn update(&self, _: &web_sys::Node, _: &DomView) -> Result<(), ToolUiError> {
        Err(ToolUiError::Presentation(
            "registry lookups do not update".into(),
        ))
    }
}
/// A contribution from `package` with the descriptor's JSON fields.
fn contribution(package: &str, descriptor: Value) -> BrowserContribution {
    BrowserContribution {
        package: package.into(),
        descriptor: serde_json::from_value(descriptor).unwrap(),
    }
}
/// A card attached to the `home` slot.
fn card(package: &str, id: &str) -> BrowserContribution {
    contribution(package, json!({"id": id, "kind": "card", "target": "home"}))
}
/// The identifiers and views attached to `slot`, in order.
fn attached(slot: &str) -> Vec<(String, Rc<dyn ContributionView>)> {
    slot_contributions(slot)
        .into_iter()
        .map(|(contribution, view)| (contribution.descriptor.id, view))
        .collect()
}

/// The same package and identifier replace the entry in place; other packages stay distinct.
#[test]
fn registration_replaces_a_contribution_with_the_same_package_and_identifier() {
    let first: Rc<dyn ContributionView> = Rc::new(Marker);
    let replacement: Rc<dyn ContributionView> = Rc::new(Marker);
    let _a = register_contribution(card("one", "a"), Rc::clone(&first));
    let _b = register_contribution(card("one", "b"), Rc::new(Marker));
    let _other = register_contribution(card("two", "a"), Rc::new(Marker));
    let _a2 = register_contribution(card("one", "a"), Rc::clone(&replacement));
    let home = attached("home");
    let ids: Vec<&str> = home.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "a"]);
    assert!(Rc::ptr_eq(&home[0].1, &replacement));
    assert!(!Rc::ptr_eq(&home[2].1, &first));
}

/// An older registration cannot remove the replacement that superseded it.
#[test]
fn removing_a_registration_never_erases_a_later_replacement() {
    let older = register_contribution(card("one", "a"), Rc::new(Marker));
    let replacement: Rc<dyn ContributionView> = Rc::new(Marker);
    let newer = register_contribution(card("one", "a"), Rc::clone(&replacement));
    assert!(!older.unregister());
    let home = attached("home");
    assert_eq!(home.len(), 1);
    assert!(Rc::ptr_eq(&home[0].1, &replacement));
    assert!(newer.unregister());
    assert!(attached("home").is_empty());
}

/// Slots list their contributions in registration order, never renderer or API kinds.
#[test]
fn slot_lookup_keeps_registration_order_and_excludes_renderer_and_api_kinds() {
    let mut kept = Vec::new();
    for (id, kind) in [
        ("z", "card"),
        ("tool", "tool-renderer"),
        ("a", "tab"),
        ("message", "message-renderer"),
        ("api", "api"),
        ("m", "widget"),
    ] {
        let declared = contribution("pkg", json!({"id": id, "kind": kind, "target": "home"}));
        kept.push(register_contribution(declared, Rc::new(Marker)));
    }
    let elsewhere = register_contribution(
        contribution("pkg", json!({"id": "x", "kind": "card", "target": "other"})),
        Rc::new(Marker),
    );
    let home = attached("home");
    let ids: Vec<&str> = home.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["z", "a", "m"]);
    assert_eq!(attached("other").len(), 1);
    assert!(attached("missing").is_empty());
    drop((kept, elsewhere));
}

/// A route resolves to the page registered for it and nothing else.
#[test]
fn route_lookup_returns_the_page_registered_for_it() {
    let _plain = register_contribution(card("pkg", "card"), Rc::new(Marker));
    let _page = register_contribution(
        contribution(
            "pkg",
            json!({"id": "page", "kind": "page", "target": "pages", "route": "/fixture"}),
        ),
        Rc::new(Marker),
    );
    let page = route_contribution("/fixture").unwrap();
    assert_eq!(
        (page.package.as_str(), page.descriptor.id.as_str()),
        ("pkg", "page")
    );
    assert!(route_contribution("/other").is_none());
    assert!(route_contribution("").is_none());
}

/// A slot reports the input contract its supplier declares.
#[test]
fn slot_input_reports_the_declared_contract() {
    let _page = register_contribution(
        contribution(
            "pkg",
            json!({
                "id": "page", "kind": "page", "target": "pages",
                "slots": [{"name": "page.tabs", "input": "fixture.tab"}, {"name": "page.free"}]
            }),
        ),
        Rc::new(Marker),
    );
    assert_eq!(slot_input("page.tabs").as_deref(), Some("fixture.tab"));
    assert_eq!(slot_input("page.free"), None);
    assert_eq!(slot_input("missing"), None);
}

/// Mismatched input and an unprovided required API each have their own failure.
#[test]
fn admission_rejects_incompatible_input_and_missing_api() {
    let declared: ContributionDescriptor = serde_json::from_value(
        json!({"id": "tab", "kind": "tab", "target": "page.tabs", "input": "fixture.tab"}),
    )
    .unwrap();
    assert_eq!(
        admit(&declared, Some("other.tab")),
        Err(ContributionFailure::IncompatibleInput {
            id: "tab".into(),
            expected: "fixture.tab".into(),
            supplied: Some("other.tab".into()),
        })
    );
    assert_eq!(
        admit(&declared, None),
        Err(ContributionFailure::IncompatibleInput {
            id: "tab".into(),
            expected: "fixture.tab".into(),
            supplied: None,
        })
    );
    let requiring: ContributionDescriptor = serde_json::from_value(
        json!({"id": "card", "kind": "card", "target": "home", "requires": ["fixture.greeting"]}),
    )
    .unwrap();
    assert_eq!(
        admit(&requiring, None),
        Err(ContributionFailure::MissingApi {
            id: "card".into(),
            api: "fixture.greeting".into(),
        })
    );
}

/// Each failure sentence names the identifier and the problem.
#[test]
fn failure_messages_name_the_identifier_and_the_problem() {
    let cases = [
        (
            ContributionFailure::MissingApi {
                id: "card".into(),
                api: "fixture.greeting".into(),
            },
            "card: missing host API fixture.greeting",
        ),
        (
            ContributionFailure::IncompatibleInput {
                id: "tab".into(),
                expected: "fixture.tab".into(),
                supplied: Some("other.tab".into()),
            },
            "tab: expects fixture.tab, slot supplies other.tab",
        ),
        (
            ContributionFailure::IncompatibleInput {
                id: "tab".into(),
                expected: "fixture.tab".into(),
                supplied: None,
            },
            "tab: expects fixture.tab, slot supplies no input",
        ),
        (
            ContributionFailure::Undeclared { id: "stray".into() },
            "stray: not declared by its package",
        ),
        (
            ContributionFailure::Initialization {
                entry: "modules/a/module.js".into(),
                cause: "HTTP 404".into(),
            },
            "modules/a/module.js: HTTP 404",
        ),
    ];
    for (failure, sentence) in cases {
        assert_eq!(failure.to_string(), sentence);
        let error: &dyn std::error::Error = &failure;
        assert!(error.source().is_none());
    }
}

/// Descriptors keep camelCase keys, omit absent optionals and reject wrong field types.
#[test]
fn descriptor_round_trips_optional_fields_and_defaults() {
    let full = json!({
        "id": "page", "kind": "page", "target": "pages", "route": "/fixture",
        "input": "fixture.page", "slots": [{"name": "page.tabs", "input": "fixture.tab"}],
        "requires": ["fixture.greeting"],
        "module": {"entry": "modules/a.js", "wasm": "modules/a.wasm", "register": "register"},
        "document": "frames/page.html"
    });
    let parsed: ContributionDescriptor = serde_json::from_value(full.clone()).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), full);
    let minimal = json!({"id": "card", "kind": "card", "target": "home"});
    for authored in [
        minimal.clone(),
        json!({"id": "card", "kind": "card", "target": "home", "route": null, "input": null,
               "document": null, "module": null, "slots": [], "requires": []}),
    ] {
        let parsed: ContributionDescriptor = serde_json::from_value(authored).unwrap();
        assert_eq!(serde_json::to_value(&parsed).unwrap(), minimal);
    }
    for invalid in [
        json!({"kind": "card", "target": "home"}),
        json!({"id": 1, "kind": "card", "target": "home"}),
        json!({"id": "card", "kind": "card", "target": "home", "route": 7}),
        json!({"id": "card", "kind": "card", "target": "home", "requires": [1]}),
        json!({"id": "card", "kind": "card", "target": "home", "slots": ["tabs"]}),
        json!({"id": "card", "kind": "card", "target": "home", "module": {"entry": "a.js"}}),
    ] {
        assert!(serde_json::from_value::<ContributionDescriptor>(invalid).is_err());
    }
}
