//! The base-layer record and the payload that publishes it.
use crate::{
    areas::Composition,
    layer::{payload, record, recorded_at},
};
use chrono::DateTime;
use serde_json::{Value, json};

fn composition() -> Composition {
    let Value::Object(settings) = json!({"defaultModel": "m", "images": {"blockImages": true}})
    else {
        unreachable!()
    };
    Composition {
        settings,
        locked: ["manifest".to_owned(), "defaultModel".to_owned()].into(),
    }
}

#[test]
fn the_record_carries_the_settings_locks_source_and_time() {
    assert_eq!(
        record(
            &composition(),
            "git:github.com/acme/m@v1",
            "2026-10-29T12:00:00Z"
        ),
        json!({
            "settings": {"defaultModel": "m", "images": {"blockImages": true}},
            "locked": ["defaultModel", "manifest"],
            "source": "git:github.com/acme/m@v1",
            "recorded_at": "2026-10-29T12:00:00Z"
        })
    );
}

#[test]
fn the_payload_publishes_a_layer_per_scope_and_removes_with_null() {
    let layer = json!({"settings": {}, "locked": [], "source": "s", "recorded_at": "t"});
    assert_eq!(
        payload(&[("user", Some(layer.clone())), ("project", None)]),
        json!({"baseLayers": [
            {"scope": "user", "layer": layer},
            {"scope": "project", "layer": null}
        ]})
    );
}

#[test]
fn the_acceptance_time_is_a_utc_instant() {
    let text = recorded_at();
    assert!(DateTime::parse_from_rfc3339(&text).is_ok(), "{text:?}");
    assert!(text.ends_with('Z'), "{text:?}");
}
