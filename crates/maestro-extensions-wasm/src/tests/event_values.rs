//! Values carried by events and results: every class of every property must come back as it
//! went in, through the built component and through the controlled adapter.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::{Value, json};

use super::documents::{
    Answer, Ended, LITERAL, ask, document, flags, images, lists, product, returns, strings, with,
};
use super::scenario::Driver;

/// Delivers `event` and checks that the handler saw it unchanged and returned `result`, a
/// document of the contract of `family`, or nothing.
async fn carries(
    driver: &mut impl Driver,
    event: &Value,
    family: &str,
    result: Option<Value>,
) -> Result<(), String> {
    let directive = result
        .as_ref()
        .map_or_else(|| json!({}), |value| returns(family, value));
    let answer = ask(driver, event, &directive).await?;
    assert_eq!(
        answer,
        Answer::returned(event, result),
        "event {event} with result directive {directive}"
    );
    Ok(())
}

/// Resource discovery, both reasons: every combination of the three optional path lists is
/// returned as authored, and a missing result stays missing.
async fn resources(driver: &mut impl Driver) -> Result<(), String> {
    for reason in ["startup", "reload"] {
        let event = json!({ "type": "resources_discover", "cwd": LITERAL, "reason": reason });
        for [skill, prompt, theme] in product::<3>(&lists()) {
            let mut result = with(json!({}), "skillPaths", &skill);
            result = with(result, "promptPaths", &prompt);
            result = with(result, "themePaths", &theme);
            carries(driver, &event, "resources_discover", Some(result)).await?;
        }
        let bare = json!({ "type": "resources_discover", "cwd": "", "reason": reason });
        carries(driver, &bare, "resources_discover", None).await?;
    }
    Ok(())
}

#[test]
fn maestro_resource_events_keep_paths_and_reasons() -> Result<(), String> {
    on_both_adapters!(resources)
}

/// Session lifecycle events: reasons with every class of their optional file.
async fn lifecycle(driver: &mut impl Driver) -> Result<(), String> {
    let reasons = [
        (
            "session_start",
            "previousSessionFile",
            vec!["startup", "reload", "new", "resume", "fork"],
        ),
        (
            "session_shutdown",
            "targetSessionFile",
            vec!["quit", "reload", "new", "resume", "fork"],
        ),
        (
            "session_before_switch",
            "targetSessionFile",
            vec!["new", "resume"],
        ),
    ];
    for (kind, property, reasons) in reasons {
        for reason in reasons {
            for file in strings() {
                let event = with(json!({ "type": kind, "reason": reason }), property, &file);
                carries(driver, &event, kind, None).await?;
            }
        }
    }
    Ok(())
}

/// Switch and fork decisions: every combination of the independent flags is returned as
/// authored.
async fn decisions(driver: &mut impl Driver) -> Result<(), String> {
    for reason in ["new", "resume"] {
        let event =
            json!({ "type": "session_before_switch", "reason": reason, "targetSessionFile": "" });
        for cancel in flags() {
            let result = with(json!({}), "cancel", &cancel);
            carries(driver, &event, "session_before_switch", Some(result)).await?;
        }
    }
    for position in ["before", "at"] {
        for entry in ["", LITERAL] {
            let event =
                json!({ "type": "session_before_fork", "entryId": entry, "position": position });
            for [cancel, skip] in product::<2>(&flags()) {
                let result = with(json!({}), "cancel", &cancel);
                let result = with(result, "skipConversationRestore", &skip);
                carries(driver, &event, "session_before_fork", Some(result)).await?;
            }
        }
    }
    Ok(())
}

/// Session events and their decisions.
async fn sessions(driver: &mut impl Driver) -> Result<(), String> {
    lifecycle(driver).await?;
    decisions(driver).await
}

#[test]
fn maestro_session_events_keep_reasons_and_optional_paths() -> Result<(), String> {
    on_both_adapters!(sessions)
}

/// The results an input handler can return for an input with text `text`: none, continue,
/// handled, and a transform of the text with every class of replacement images.
fn input_results(text: &str) -> Vec<Option<Value>> {
    let mut results = vec![
        None,
        Some(json!({ "action": "continue" })),
        Some(json!({ "action": "handled" })),
    ];
    for replacement in images() {
        let transform = json!({ "action": "transform", "text": format!("changed:{text}") });
        results.push(Some(with(transform, "images", &replacement)));
    }
    results
}

/// The inputs of one source and text with every class of images, each with every result.
fn input_cases(source: &str, text: &str) -> Vec<(Value, Option<Value>)> {
    let mut cases = Vec::new();
    for attached in images() {
        let event = with(
            json!({ "type": "input", "text": text, "source": source }),
            "images",
            &attached,
        );
        cases.extend(
            input_results(text)
                .into_iter()
                .map(|result| (event.clone(), result)),
        );
    }
    cases
}

/// Inputs from every source with every class of images, answered by every kind of result.
async fn inputs(driver: &mut impl Driver) -> Result<(), String> {
    for source in ["interactive", "rpc", "extension"] {
        for text in ["", LITERAL] {
            for (event, result) in input_cases(source, text) {
                carries(driver, &event, "input", result).await?;
            }
        }
    }
    Ok(())
}

#[test]
fn maestro_input_transforms_keep_source_and_image_presence() -> Result<(), String> {
    on_both_adapters!(inputs)
}

/// The opaque JSON texts a provider payload or replacement can be.
const PAYLOADS: [&str; 7] = [
    "null",
    "false",
    "0",
    "\"\"",
    "[]",
    "{}",
    r#"{"z":1,"a":[false,null,"Ω"]}"#,
];

/// Every payload text answered by no replacement and by every payload text as a replacement.
async fn provider_requests(driver: &mut impl Driver) -> Result<(), String> {
    for payload in PAYLOADS {
        let event = json!({ "type": "before_provider_request", "payload": payload });
        carries(driver, &event, "before_provider_request", None).await?;
        for replacement in PAYLOADS {
            let result = Some(json!(replacement));
            carries(driver, &event, "before_provider_request", result).await?;
        }
    }
    Ok(())
}

#[test]
fn maestro_provider_replacements_keep_absent_and_falsy_json() -> Result<(), String> {
    on_both_adapters!(provider_requests)
}

/// Header lists in the order the host enumerates them: integer-looking names first, case
/// distinct names, an empty name, an empty value and a repeated name.
fn header_lists() -> [Value; 3] {
    [
        json!([]),
        json!([
            ["2", "two"],
            ["10", "ten"],
            ["X-Z", " z "],
            ["x-a", ""],
            ["", "Ω"],
            ["__proto__", "literal"],
        ]),
        json!([["set-cookie", "a=1"], ["set-cookie", "b=2"]]),
    ]
}

/// Responses with the repeated-name header list at every status, which the number test does
/// not cross; an edit of status and headers over each header list.
async fn responses(driver: &mut impl Driver) -> Result<(), String> {
    let [empty, mixed, repeated] = header_lists();
    for status in [0.0, 200.0, 599.0] {
        let event =
            json!({ "type": "after_provider_response", "status": status, "headers": repeated });
        carries(driver, &event, "", None).await?;
    }
    let event = document("after_provider_response");
    for headers in [empty, mixed, repeated] {
        let reversed: Vec<Value> = headers
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .cloned()
            .collect();
        for status in [json!(-3.0), json!(0.25), json!(-0.0)] {
            let edit =
                json!({ "type": "after_provider_response", "status": status, "headers": reversed });
            let directive = json!({ "replaceWith": edit });
            let answer = ask(driver, &event, &directive).await?;
            assert_eq!(answer, Answer::returned(&edit, None), "edit {edit}");
        }
    }
    Ok(())
}

#[test]
fn maestro_response_events_keep_status_and_header_order() -> Result<(), String> {
    on_both_adapters!(responses)
}

/// The authored JSON texts that must pass through byte for byte: duplicate keys, whitespace,
/// exponent, signed zero, escapes, a token-like string, scalars and a very deep array.
fn opaque_texts() -> Vec<String> {
    vec![
        r#"{ "z":1, "a":2, "z":3, "n":1e+02, "neg":-0.0, "esc":"\u03a9", "glyph":"😀", "token":"f64:7ff0000000000000" }"#.to_owned(),
        "null".to_owned(),
        "false".to_owned(),
        "0".to_owned(),
        "\"\"".to_owned(),
        "[]".to_owned(),
        "{}".to_owned(),
        format!("{}0{}", "[".repeat(256), "]".repeat(256)),
    ]
}

/// Opaque texts as a provider payload and as its replacement, except those [`PAYLOADS`]
/// already carries.
async fn opaque_provider_texts(driver: &mut impl Driver) -> Result<(), String> {
    for text in opaque_texts()
        .into_iter()
        .filter(|text| !PAYLOADS.contains(&text.as_str()))
    {
        let event = json!({ "type": "before_provider_request", "payload": text });
        carries(driver, &event, "before_provider_request", Some(json!(text))).await?;
    }
    Ok(())
}

/// The compaction event with every class of its optional properties and a result whose
/// details take every class of text.
async fn compaction_properties(driver: &mut impl Driver) -> Result<(), String> {
    for (index, [instructions, summary, details]) in
        product::<3>(&strings()).into_iter().enumerate()
    {
        let preparation = with(
            json!({ "firstKeptEntryId": "keep", "isSplitTurn": index % 2 == 0, "tokensBefore": 1.5 }),
            "previousSummary",
            &summary,
        );
        let event = with(
            json!({ "type": "session_before_compact", "preparation": preparation }),
            "customInstructions",
            &instructions,
        );
        let compaction =
            json!({ "summary": "changed", "firstKeptEntryId": "keep", "tokensBefore": -1.5 });
        let result = json!({ "compaction": with(compaction, "details", &details) });
        carries(driver, &event, "session_before_compact", Some(result)).await?;
    }
    Ok(())
}

/// Opaque texts as the details of a replacement compaction.
async fn opaque_details(driver: &mut impl Driver) -> Result<(), String> {
    let event = document("session_before_compact");
    for text in opaque_texts() {
        let compaction = json!({
            "summary": "changed", "firstKeptEntryId": "keep", "tokensBefore": 0.0, "details": text,
        });
        let result = json!({ "compaction": compaction });
        carries(driver, &event, "session_before_compact", Some(result)).await?;
    }
    Ok(())
}

/// A cancellation flag with a replacement compaction: missing, null, empty and populated.
async fn compaction_results(driver: &mut impl Driver) -> Result<(), String> {
    let event = json!({
        "type": "session_before_compact",
        "preparation": { "firstKeptEntryId": "keep", "isSplitTurn": false, "tokensBefore": 0.0 },
    });
    let empty = json!({ "summary": "", "firstKeptEntryId": "", "tokensBefore": 0.0 });
    let populated = json!({
        "summary": LITERAL, "firstKeptEntryId": LITERAL, "tokensBefore": -1.5, "details": "false",
    });
    let compactions = [None, Some(Value::Null), Some(empty), Some(populated)];
    for cancel in flags() {
        for compaction in &compactions {
            let result = with(with(json!({}), "cancel", &cancel), "compaction", compaction);
            carries(driver, &event, "session_before_compact", Some(result)).await?;
        }
    }
    Ok(())
}

/// Opaque texts and compaction records.
async fn opaque_and_compaction(driver: &mut impl Driver) -> Result<(), String> {
    opaque_provider_texts(driver).await?;
    compaction_properties(driver).await?;
    opaque_details(driver).await?;
    compaction_results(driver).await
}

#[test]
fn maestro_opaque_event_json_keeps_authored_bytes() -> Result<(), String> {
    on_both_adapters!(opaque_and_compaction)
}

/// Finite number patterns: both zeros, finite extremes and the integer precision boundary.
const PATTERNS: [u64; 13] = [
    0x4069_0000_0000_0000,
    0x4082_b800_0000_0000,
    0x0000_0000_0000_0000,
    0x8000_0000_0000_0000,
    0x0000_0000_0000_0001,
    0x8000_0000_0000_0001,
    0x0010_0000_0000_0000,
    0x7fef_ffff_ffff_ffff,
    0xffef_ffff_ffff_ffff,
    0x3fd5_5555_5555_5555,
    0xbff8_0000_0000_0000,
    0x4340_0000_0000_0000,
    0x4340_0000_0000_0001,
];

/// The JSON document of a finite number.
fn number_of(bits: u64) -> Value {
    json!(f64::from_bits(bits))
}

/// The bits of a JSON number document.
fn bits_of(document: Option<&Value>) -> Result<u64, String> {
    document
        .and_then(Value::as_f64)
        .map(f64::to_bits)
        .ok_or_else(|| format!("{document:?} is not a number document"))
}

/// The bits of the status that came back after the number `bits` went in beside `headers`,
/// which come back unchanged, order included.
async fn status_back(driver: &mut impl Driver, bits: u64, headers: &Value) -> Result<u64, String> {
    let response =
        json!({ "type": "after_provider_response", "status": number_of(bits), "headers": headers });
    let answer = ask(driver, &response, &json!({})).await?;
    let event = answer.event.as_ref();
    assert_eq!(
        event.map(|event| &event["headers"]),
        Some(headers),
        "incoming headers beside status bits {bits:016x}"
    );
    bits_of(event.map(|event| &event["status"]))
}

/// The bits of the preparation tokens and of the result tokens that came back after the number
/// `bits` went in each.
async fn tokens_back(driver: &mut impl Driver, bits: u64) -> Result<[u64; 2], String> {
    let number = number_of(bits);
    let preparation = json!({
        "firstKeptEntryId": LITERAL, "isSplitTurn": true, "tokensBefore": number, "previousSummary": "",
    });
    let event = json!({
        "type": "session_before_compact", "preparation": preparation, "customInstructions": LITERAL,
    });
    let compaction = json!({
        "summary": LITERAL, "firstKeptEntryId": "keep", "tokensBefore": number, "details": null,
    });
    let directive = returns(
        "session_before_compact",
        &json!({ "cancel": false, "compaction": compaction }),
    );
    let answer = ask(driver, &event, &directive).await?;
    let Ended::Returned(Some(result)) = answer.ended else {
        return Err("the handler returned no result".to_owned());
    };
    Ok([
        bits_of(
            answer
                .event
                .as_ref()
                .map(|event| &event["preparation"]["tokensBefore"]),
        )?,
        bits_of(Some(&result["compaction"]["tokensBefore"]))?,
    ])
}

/// Every pattern keeps its bits in the status beside an empty and a populated header list, which
/// stay as they came, in the preparation tokens and in the result tokens.
async fn numbers(driver: &mut impl Driver) -> Result<(), String> {
    let [empty, mixed, _] = header_lists();
    for bits in PATTERNS {
        for headers in [&empty, &mixed] {
            let status = status_back(driver, bits, headers).await?;
            assert_eq!(status, bits, "status bits {bits:016x} beside {headers}");
        }
        assert_eq!(
            tokens_back(driver, bits).await?,
            [bits; 2],
            "bits {bits:016x} in the preparation tokens and the result tokens"
        );
    }
    Ok(())
}

#[test]
fn maestro_event_numbers_keep_bits_across_adapters() -> Result<(), String> {
    on_both_adapters!(numbers)
}
