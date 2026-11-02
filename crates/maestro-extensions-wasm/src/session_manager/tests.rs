//! Opaque entry payloads retain their semantic JSON at the wire boundary.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::SessionEntry;
use maestro_request::presence::Presence;
use serde_json::{Value, json};

#[test]
fn session_entry_payloads_keep_exact_json_text_and_wire_roundtrip() {
    for (kind, field) in [
        ("custom", "data"),
        ("compaction", "details"),
        ("branch_summary", "details"),
        ("custom_message", "details"),
    ] {
        for payload in [
            Some(json!({"z": {"b": [false, null, "Ω"]}, "a": 1})),
            Some(json!([null, true, {"z": 1, "a": 2}])),
            Some(json!(2.5)),
            Some(json!(9_007_199_254_740_993_u64)),
            Some(json!("literal \"text\"")),
            Some(Value::Null),
            None,
        ] {
            let mut input = json!({"type": kind, "id": "root", "parentId": null,
                "timestamp": "literal", "customType": "state", "summary": "summary",
                "firstKeptEntryId": "keep", "tokensBefore": 2.5, "fromId": "root",
                "content": "content", "display": true});
            if let Some(payload) = &payload {
                input[field] = payload.clone();
            }
            let decoded: SessionEntry = serde_json::from_value(input.clone()).unwrap();
            let text = match &decoded {
                SessionEntry::Custom(entry) => &entry.data,
                SessionEntry::Compaction(entry) => &entry.details,
                SessionEntry::BranchSummary(entry) => &entry.details,
                SessionEntry::CustomMessage(entry) => &entry.details,
                _ => panic!("wrong entry kind"),
            };
            let expected = match &payload {
                None => Presence::Missing,
                Some(Value::Null) => Presence::Null,
                Some(payload) => Presence::Present(serde_json::to_string(payload).unwrap().into()),
            };
            assert_eq!(text, &expected, "{kind}: {payload:?}");
            let wire = serde_json::to_value(&decoded).unwrap();
            assert_eq!(wire.get(field), payload.as_ref());
            let reread: SessionEntry = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(reread).unwrap(), wire);
        }
    }
}

#[test]
fn opaque_entry_decoding_keeps_duplicate_field_rejection() {
    let text = r#"{"type":"custom","customType":"first","customType":"second","id":"root","parentId":null,"timestamp":"literal","data":{"n":2.5}}"#;
    let error = serde_json::from_str::<SessionEntry>(text).unwrap_err();
    assert!(error.to_string().contains("duplicate field"));
}

#[test]
fn json_text_preserves_text_access_and_checks_authored_json() {
    let authored = String::from("{ \"z\": 1, \"a\": 2 }");
    let text = super::JsonText::from(authored.clone());
    let borrowed: &str = &text;
    assert_eq!(borrowed, authored);
    assert_eq!(text.as_ref(), authored);
    assert_eq!(text.to_string(), authored);
    assert_eq!(String::from(text.clone()), authored);
    assert_eq!(serde_json::to_string(&text).unwrap(), r#"{"z":1,"a":2}"#);
    assert!(serde_json::to_string(&super::JsonText::from("not JSON".to_owned())).is_err());
}
