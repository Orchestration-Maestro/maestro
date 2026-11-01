//! Session records carried through both author adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::documents::{Answer, ask};
use super::scenario::Driver;
use serde_json::{Value, json};

/// Prepared lists, settings and file categories remain independent.
async fn preparations(driver: &mut impl Driver) -> Result<(), String> {
    for populated in [false, true] {
        for (split, enabled) in [(false, false), (false, true), (true, false), (true, true)] {
            let messages = if populated {
                json!([{"role":"user","content":"summary","timestamp":1.5}])
            } else {
                json!([])
            };
            let prefix = if populated {
                json!([{"role":"user","content":"prefix","timestamp":2.5}])
            } else {
                json!([])
            };
            let event = json!({"type":"session_before_compact","branchEntries":[],"preparation":{
                    "firstKeptEntryId":"cut","tokensBefore":3.5,"isSplitTurn":split,
                    "messagesToSummarize":messages,"turnPrefixMessages":prefix,
                    "fileOps":{"read":if populated {vec!["read"]} else {vec![]},"written":[],"edited":[]},
                    "settings":{"enabled":enabled,"reserveTokens":4.5,"keepRecentTokens":5.5}}});
            assert_eq!(
                ask(driver, &event, &json!({})).await?,
                Answer::returned(&event, None)
            );
        }
    }
    let mut event = super::documents::document("session_before_compact");
    let messages = super::session_corpus::messages();
    assert_eq!(messages.as_array().unwrap().len(), 8);
    event["preparation"]["messagesToSummarize"] = messages.clone();
    let mut prefix = messages.as_array().unwrap().clone();
    prefix.reverse();
    event["preparation"]["turnPrefixMessages"] = json!(prefix);
    assert_eq!(
        ask(driver, &event, &json!({})).await?,
        Answer::returned(&event, None)
    );
    Ok(())
}
#[test]
fn maestro_compaction_preparation_keeps_messages_settings_and_files() -> Result<(), String> {
    on_both_adapters!(preparations)
}

/// Every entry variant is carried in both entry lists; the nested message families are
/// covered in compaction entries before compaction, and tree entries carry user messages.
async fn entries(driver: &mut impl Driver) -> Result<(), String> {
    super::session_corpus::run(
        driver,
        &[
            "entry-variants",
            "nested-messages",
            "custom-content",
            "parent-ids",
            "tree-entry-order",
        ],
    )
    .await
}
#[test]
fn maestro_session_entries_keep_every_variant() -> Result<(), String> {
    on_both_adapters!(entries)
}

/// File categories keep literal identity and insertion order across handler edits.
async fn file_operations(driver: &mut impl Driver) -> Result<(), String> {
    super::session_corpus::run(driver, &["file-sets"]).await?;
    let mut event = super::documents::document("session_before_compact");
    for paths in [
        json!([]),
        json!([
            "z", "a", "z", "A", "é", "e\u{301}", " ../x ", "\u{feff}", "\u{85}"
        ]),
    ] {
        event["preparation"]["fileOps"] =
            json!({"read":paths,"written":["z","a","z"],"edited":["z"]});
        let mut expected = event.clone();
        expected["preparation"]["fileOps"]["read"] = if paths.as_array().unwrap().is_empty() {
            json!([])
        } else {
            json!([
                "a", "A", "é", "e\u{301}", " ../x ", "\u{feff}", "\u{85}", "z"
            ])
        };
        expected["preparation"]["fileOps"]["written"] = json!(["z", "a"]);
        assert_eq!(
            ask(driver, &event, &json!({"sessionAction":"file_edit"})).await?,
            Answer::returned(&expected, None)
        );
    }
    // Differently escaped equal authored strings are the same set member.
    let text = event
        .to_string()
        .replace("\"read\":[", "\"read\":[\"a\",\"\\u0061\",");
    let event: serde_json::Value = serde_json::from_str(&text).unwrap();
    let answer = ask(driver, &event, &json!({})).await?;
    let read = &answer.event.unwrap()["preparation"]["fileOps"]["read"];
    assert_eq!(
        read.as_array()
            .unwrap()
            .iter()
            .filter(|value| **value == json!("a"))
            .count(),
        1
    );
    Ok(())
}
#[test]
fn maestro_file_operations_keep_set_membership_and_insertion_order() -> Result<(), String> {
    on_both_adapters!(file_operations)
}

/// Unshared optional entry, preparation and compaction-result fields retain their states.
async fn optionals(driver: &mut impl Driver) -> Result<(), String> {
    super::session_corpus::run(
        driver,
        &[
            "optional-fields",
            "entry-optionals",
            "compaction-result-fields",
        ],
    )
    .await
}
#[test]
fn maestro_session_optional_fields_keep_missing_null_and_values() -> Result<(), String> {
    on_both_adapters!(optionals)
}

/// Selected nested decoders keep raw message input and ordinary Serde record domains.
async fn ingress(driver: &mut impl Driver) -> Result<(), String> {
    let canonical = super::documents::document("session_before_compact");
    let mut event = canonical.clone();
    event["preparation"]["settings"] = json!([true, 1.0, 2.0]);
    event["preparation"]["fileOps"] = json!([[], [], []]);
    assert_eq!(
        ask(driver, &event, &json!({})).await?,
        Answer::returned(&canonical, None)
    );
    let entry = json!({"type":"thinking_level_change","id":"z","parentId":null,"timestamp":"literal","thinkingLevel":"open"});
    event = canonical.clone();
    event["branchEntries"] = json!([entry]);
    let mut extra = event.clone();
    extra["branchEntries"][0]["unread"] = json!({"anything":[false]});
    assert_eq!(
        ask(driver, &extra, &json!({})).await?,
        Answer::returned(&event, None)
    );
    let mut absent_parent = event.clone();
    absent_parent["branchEntries"][0]
        .as_object_mut()
        .unwrap()
        .shift_remove("parentId");
    assert_eq!(
        ask(driver, &absent_parent, &json!({})).await?,
        Answer::returned(&event, None)
    );
    refused_entries(driver, &canonical, &event).await?;
    // A concrete compaction entry's tag is serialization metadata, not a second selector.
    let mut compact = super::session_corpus::base("session_compact");
    let expected = compact.clone();
    compact["compactionEntry"]["type"] = json!({"ignored":true});
    assert_eq!(
        ask(driver, &compact, &json!({})).await?,
        Answer::returned(&expected, None)
    );
    let tree = super::session_corpus::base("session_before_tree");
    let mut array_tree = tree.clone();
    let prep = &tree["preparation"];
    array_tree["preparation"] = json!([
        prep["targetId"],
        prep["oldLeafId"],
        prep["commonAncestorId"],
        prep["entriesToSummarize"],
        prep["userWantsSummary"],
        prep["customInstructions"],
        prep["replaceInstructions"],
        prep["label"]
    ]);
    assert_eq!(
        ask(driver, &array_tree, &json!({})).await?,
        Answer::returned(&tree, None)
    );
    Ok(())
}
#[test]
fn maestro_session_ingress_uses_selected_record_decoders() -> Result<(), String> {
    on_both_adapters!(ingress)
}

/// Refuse malformed selected entries before the probe handler can run.
async fn refused_entries(
    driver: &mut impl Driver,
    canonical: &serde_json::Value,
    event: &serde_json::Value,
) -> Result<(), String> {
    let handler = driver.identity("event probe")?;
    for entry in [
        json!({"type":"unknown"}),
        json!({"type":{"message":null}}),
        json!(["thinking_level_change", "id"]),
        json!({"type":"message","id":"z","parentId":null,"timestamp":"t","message":{"role":"user","content":"missing timestamp"}}),
    ] {
        let mut malformed = canonical.clone();
        malformed["branchEntries"] = json!([entry]);
        let signal = driver.lend_signal(false)?;
        assert!(
            driver
                .deliver(handler, &malformed.to_string(), "{}", Some(signal))
                .await
                .is_err()
        );
    }
    let duplicate = event.to_string().replace(
        "\"thinkingLevel\":\"open\"",
        "\"thinkingLevel\":\"first\",\"thinkingLevel\":\"open\"",
    );
    let signal = driver.lend_signal(false)?;
    assert!(
        driver
            .deliver(handler, &duplicate, "{}", Some(signal))
            .await
            .unwrap_err()
            .contains("duplicate field")
    );
    let escaped = event
        .to_string()
        .replace("thinking_level_change", "thinking_level_\\u0063hange");
    let signal = driver.lend_signal(false)?;
    let delivered = driver
        .deliver(handler, &escaped, "{}", Some(signal))
        .await?;
    assert_eq!(Answer::of(delivered)?, Answer::returned(event, None));
    Ok(())
}

/// Re-encoding selects the stored member order rather than the incoming order.
async fn shared_record_order(driver: &mut impl Driver) -> Result<(), String> {
    let mut event = super::documents::document("session_before_compact");
    event["branchEntries"] = json!([
        {"type":"custom","id":"state","parentId":null,"timestamp":"literal",
         "customType":"state","data":"opaque"},
        {"type":"custom_message","id":"display","parentId":"state","timestamp":"literal",
         "customType":"display","content":"hello","display":true,"details":"opaque"}
    ]);
    event["preparation"]["previousSummary"] = json!("earlier");
    let reversed = event["preparation"]
        .as_object()
        .unwrap()
        .iter()
        .rev()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    event["preparation"] = serde_json::Value::Object(reversed);
    let answer = ask(driver, &event, &json!({})).await?;
    let encoded = answer.event.as_ref().unwrap();
    assert_shared_member_order(encoded);
    assert_eq!(answer, Answer::returned(&event, None));
    Ok(())
}
#[test]
fn maestro_shared_records_reencode_in_session_file_member_order() -> Result<(), String> {
    on_both_adapters!(shared_record_order)
}

/// Checks the concrete entry and preparation declaration sequences.
fn assert_shared_member_order(encoded: &serde_json::Value) {
    for (index, expected) in [
        vec!["type", "customType", "data", "id", "parentId", "timestamp"],
        vec![
            "type",
            "customType",
            "content",
            "display",
            "details",
            "id",
            "parentId",
            "timestamp",
        ],
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(
            encoded["branchEntries"][index]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            *expected
        );
    }
    assert_eq!(
        encoded["preparation"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "firstKeptEntryId",
            "messagesToSummarize",
            "turnPrefixMessages",
            "isSplitTurn",
            "tokensBefore",
            "previousSummary",
            "fileOps",
            "settings"
        ]
    );
}

/// All opaque entry fields roundtrip arbitrary semantic JSON inside real event delivery.
async fn opaque_entry_payloads(driver: &mut impl Driver) -> Result<(), String> {
    for payload in [
        Some(json!({"z": {"b": [false, null, "Ω"]}, "a": 1})),
        Some(json!([null, true, {"z": 1, "a": 2}])),
        Some(json!(2.5)),
        Some(json!(9_007_199_254_740_993_u64)),
        Some(json!("literal \"text\"")),
        Some(Value::Null),
        None,
    ] {
        let mut event = super::documents::document("session_before_compact");
        let entries = opaque_entries(payload.as_ref());
        event["branchEntries"] = json!(entries);
        let compact =
            json!({"type":"session_compact", "compactionEntry": entries[1], "fromExtension": true});
        let tree = json!({"type":"session_tree", "newLeafId": null, "oldLeafId": null,
            "summaryEntry": entries[2], "fromExtension": false});
        for event in [event, compact, tree] {
            assert_eq!(
                ask(driver, &event, &json!({})).await?,
                Answer::returned(&event, None)
            );
        }
    }
    Ok(())
}

#[test]
fn maestro_session_entry_payloads_roundtrip_arbitrary_json() -> Result<(), String> {
    on_both_adapters!(opaque_entry_payloads)
}

/// Builds all four source entry shapes with the same independently selected payload.
pub(super) fn opaque_entries(payload: Option<&Value>) -> Vec<Value> {
    let mut entries = Vec::new();
    for (kind, field) in [
        ("custom", "data"),
        ("compaction", "details"),
        ("branch_summary", "details"),
        ("custom_message", "details"),
    ] {
        let mut entry = json!({"type":kind,"id":"root","parentId":null,"timestamp":"literal"});
        match kind {
            "custom" => entry["customType"] = json!("state"),
            "compaction" => {
                entry["summary"] = json!("summary");
                entry["firstKeptEntryId"] = json!("keep");
                entry["tokensBefore"] = json!(2.5);
            }
            "branch_summary" => {
                entry["fromId"] = json!("root");
                entry["summary"] = json!("summary");
            }
            "custom_message" => {
                entry["customType"] = json!("state");
                entry["content"] = json!("content");
                entry["display"] = json!(true);
            }
            _ => unreachable!(),
        }
        if let Some(payload) = payload {
            entry[field] = payload.clone();
        }
        entries.push(entry);
    }
    entries
}
