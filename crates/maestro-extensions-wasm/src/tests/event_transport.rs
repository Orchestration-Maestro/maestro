//! Transport of event data through the shared adapter: optional properties, failure phases,
//! resource lifetimes and suspension.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde::ser::{Error as _, Serializer};
use serde_json::{Value, json};

use super::documents::{
    Answer, Ended, KINDS, ask, ask_with, document, fails, marking, replacing, returns,
};
use super::guest_family;
use super::scenario::{Decision, Delivery, Driver};
use crate::component_adapter::outcome;
use crate::types::EventData;
use crate::{
    CompactionResult, Presence, ResourcesDiscoverResult, SessionBeforeCompactResult,
    SessionBeforeSwitchResult, SessionStartEvent, SessionStartReason,
};

/// Checks that `value` serializes to `expected` and that parsing that text back yields a value
/// serializing to the same text, so no state collapses into another.
fn assert_stable<T: Serialize + DeserializeOwned>(value: &T, expected: &str) -> Result<(), String> {
    let text = serde_json::to_string(value).map_err(|error| error.to_string())?;
    assert_eq!(text, expected);
    let parsed: T = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let again = serde_json::to_string(&parsed).map_err(|error| error.to_string())?;
    assert_eq!(
        again, expected,
        "parsing the text back keeps the property state"
    );
    Ok(())
}

/// The three states of one optional property, built from a present value.
fn states<T>(present: T) -> [Presence<T>; 3] {
    [
        Presence::Missing,
        Presence::Null,
        Presence::Present(present),
    ]
}

#[test]
fn maestro_missing_properties_do_not_become_null() -> Result<(), String> {
    let flags = ["{}", r#"{"cancel":null}"#, r#"{"cancel":false}"#];
    for (cancel, expected) in states(false).into_iter().zip(flags) {
        assert_stable(&SessionBeforeSwitchResult { cancel }, expected)?;
    }
    let lists = ["{}", r#"{"skillPaths":null}"#, r#"{"skillPaths":[]}"#];
    for (skill_paths, expected) in states(Vec::<String>::new()).into_iter().zip(lists) {
        let result = ResourcesDiscoverResult {
            skill_paths,
            prompt_paths: Presence::Missing,
            theme_paths: Presence::Missing,
        };
        assert_stable(&result, expected)?;
    }
    let texts = [
        r#"{"reason":"new"}"#,
        r#"{"reason":"new","previousSessionFile":null}"#,
        r#"{"reason":"new","previousSessionFile":""}"#,
    ];
    for (previous_session_file, expected) in states(String::new()).into_iter().zip(texts) {
        let reason = SessionStartReason::New;
        assert_stable(
            &SessionStartEvent {
                reason,
                previous_session_file,
            },
            expected,
        )?;
    }
    let empty = CompactionResult {
        summary: String::new(),
        first_kept_entry_id: String::new(),
        tokens_before: 0.0,
        details: Presence::Missing,
    };
    let records = [
        "{}",
        r#"{"compaction":null}"#,
        r#"{"compaction":{"summary":"","firstKeptEntryId":"","tokensBefore":0.0}}"#,
    ];
    for (compaction, expected) in states(empty).into_iter().zip(records) {
        let result = SessionBeforeCompactResult {
            cancel: Presence::Missing,
            compaction,
        };
        assert_stable(&result, expected)?;
    }
    let missing = serde_json::to_string(&Presence::<String>::Missing).map(|_| ());
    assert_eq!(
        missing.map_err(|error| error.to_string()),
        Err("missing value outside a property".to_owned())
    );
    let required = serde_json::from_str::<SessionStartEvent>("{}");
    assert!(
        required.is_err(),
        "a required field has no fabricated default"
    );
    Ok(())
}

/// The message the probe handler fails with.
const AUTHORED_FAILURE: &str = "authored failure Ω";

/// The field of each kind that the probe handler edits.
const MARKED: [(&str, &str); 9] = [
    ("resources_discover", "cwd"),
    ("session_start", "previousSessionFile"),
    ("session_before_switch", "targetSessionFile"),
    ("session_before_fork", "entryId"),
    ("session_before_compact", "customInstructions"),
    ("session_shutdown", "targetSessionFile"),
    ("before_provider_request", "payload"),
    ("after_provider_response", "status"),
    ("input", "text"),
];

/// The document with its marked field edited the way the probe handler edits it.
fn edited(document: &Value, field: &str) -> Value {
    let mut edited = document.clone();
    edited[field] = if field == "status" {
        json!(201.0)
    } else {
        json!("changed")
    };
    edited
}

/// A result of the contract of the kind, for the kinds that have one.
fn own_result(kind: &str) -> Option<(&'static str, Value)> {
    Some(match kind {
        "resources_discover" => ("resources_discover", json!({ "skillPaths": ["a"] })),
        "session_before_switch" => ("session_before_switch", json!({ "cancel": true })),
        "session_before_fork" => (
            "session_before_fork",
            json!({ "cancel": false, "skipConversationRestore": true }),
        ),
        "session_before_compact" => ("session_before_compact", json!({ "cancel": true })),
        "before_provider_request" => ("before_provider_request", json!("replacement")),
        "input" => ("input", json!({ "action": "handled" })),
        _ => return None,
    })
}

/// A result of the contract of another kind than `kind`.
fn foreign_result(kind: &str) -> (&'static str, Value) {
    if kind == "input" {
        ("session_before_switch", json!({ "cancel": true }))
    } else {
        ("input", json!({ "action": "handled" }))
    }
}

/// An edit made before an error, a result or none survives in the event; a handler that
/// replaces its event with one of any other kind leaves no event to return.
async fn edits_and_replacements(driver: &mut impl Driver) -> Result<(), String> {
    for (kind, field) in MARKED {
        let event = document(kind);
        let edit = edited(&event, field);
        let (family, value) = foreign_result(kind);
        let mut endings = vec![
            (json!({}), Ended::Returned(None)),
            (
                fails(AUTHORED_FAILURE),
                Ended::Failed(AUTHORED_FAILURE.to_owned()),
            ),
            (returns(family, &value), Ended::Returned(Some(value))),
        ];
        if let Some((family, value)) = own_result(kind) {
            endings.push((returns(family, &value), Ended::Returned(Some(value))));
        }
        for (directive, ended) in endings {
            let answer = ask(driver, &event, &marking(directive.clone())).await?;
            let expected = Answer {
                event: Some(edit.clone()),
                ended,
            };
            assert_eq!(answer, expected, "{kind} edited, directive {directive}");
        }
    }
    for kind in KINDS {
        for target in KINDS.into_iter().filter(|target| *target != kind) {
            for (directive, ended) in [
                (json!({}), Ended::Returned(None)),
                (
                    fails(AUTHORED_FAILURE),
                    Ended::Failed(AUTHORED_FAILURE.to_owned()),
                ),
            ] {
                let directive = replacing(directive, document(target));
                let answer = ask(driver, &document(kind), &directive).await?;
                let expected = Answer { event: None, ended };
                assert_eq!(
                    answer, expected,
                    "{kind} replaced by {target}, directive {directive}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn maestro_event_edits_survive_errors_and_kind_changes() -> Result<(), String> {
    on_both_adapters!(edits_and_replacements)
}

/// The properties each kind cannot do without.
const REQUIRED: [(&str, &[&str]); 9] = [
    ("resources_discover", &["cwd", "reason"]),
    ("session_start", &["reason"]),
    ("session_before_switch", &["reason"]),
    ("session_before_fork", &["entryId", "position"]),
    (
        "session_before_compact",
        &[
            "preparation",
            "preparation.firstKeptEntryId",
            "preparation.isSplitTurn",
            "preparation.tokensBefore",
        ],
    ),
    ("session_shutdown", &["reason"]),
    ("before_provider_request", &["payload"]),
    ("after_provider_response", &["status", "headers"]),
    ("input", &["text", "source"]),
];

/// The optional properties that hold text or a list.
const OPTIONAL: [(&str, &str); 6] = [
    ("session_start", "previousSessionFile"),
    ("session_before_switch", "targetSessionFile"),
    ("session_before_compact", "customInstructions"),
    ("session_before_compact", "preparation.previousSummary"),
    ("session_shutdown", "targetSessionFile"),
    ("input", "images"),
];

/// The properties that hold one of a fixed set of words.
const WORDS: [(&str, &str); 6] = [
    ("resources_discover", "reason"),
    ("session_start", "reason"),
    ("session_before_switch", "reason"),
    ("session_before_fork", "position"),
    ("session_shutdown", "reason"),
    ("input", "source"),
];

/// The document with the property at the dotted `path` replaced by `value` or removed.
fn at(document: &Value, path: &str, value: Option<Value>) -> Value {
    let mut changed = document.clone();
    let (parent, key) = path.rsplit_once('.').unwrap_or(("", path));
    let target = if parent.is_empty() {
        Some(&mut changed)
    } else {
        changed.pointer_mut(&format!("/{}", parent.replace('.', "/")))
    };
    if let Some(object) = target.and_then(Value::as_object_mut) {
        match value {
            Some(value) => object.insert(key.to_owned(), value),
            None => object.shift_remove(key),
        };
    }
    changed
}

/// A document the decoder must refuse, as text, with what is wrong with it.
type Refusal = (String, String);

/// The documents that lack a tag or a required property, or mistype or misword a property.
fn wrong_properties() -> Vec<Refusal> {
    let mut found = Vec::new();
    let mut add = |label: String, document: Value| found.push((label, document.to_string()));
    for (kind, paths) in REQUIRED {
        let valid = document(kind);
        add(format!("{kind} without a tag"), at(&valid, "type", None));
        for path in paths {
            let wrong = if *path == "preparation" {
                json!("not a record")
            } else {
                json!({})
            };
            add(format!("{kind} without {path}"), at(&valid, path, None));
            add(
                format!("{kind} with {path} of the wrong type"),
                at(&valid, path, Some(wrong)),
            );
        }
    }
    for (kind, path) in OPTIONAL {
        add(
            format!("{kind} with {path} of the wrong type"),
            at(&document(kind), path, Some(json!({}))),
        );
    }
    add(
        "an unknown kind".to_owned(),
        at(&document("input"), "type", Some(json!("session_unknown"))),
    );
    for (kind, word) in WORDS {
        add(
            format!("{kind} with an unknown {word}"),
            at(&document(kind), word, Some(json!("unknown"))),
        );
    }
    add(
        "an object-valued kind".to_owned(),
        json!({ "type": { "input": null }, "text": "x", "source": "rpc" }),
    );
    found
}

/// The documents with an image, a header pair or a nested record that is malformed, and the
/// ones that give a record as a positional array.
fn wrong_records() -> Vec<Refusal> {
    let mut found = Vec::new();
    let mut add = |label: String, document: Value| found.push((label, document.to_string()));
    for (what, image) in [
        ("without data", json!({ "mimeType": "image/png" })),
        ("without mimeType", json!({ "data": "AA==" })),
        (
            "with data of the wrong type",
            json!({ "data": {}, "mimeType": "image/png" }),
        ),
        (
            "with mimeType of the wrong type",
            json!({ "data": "AA==", "mimeType": {} }),
        ),
    ] {
        let input = at(&document("input"), "images", Some(json!([image])));
        add(format!("an image {what}"), input);
    }
    let image = at(
        &document("input"),
        "images",
        Some(json!([["AA==", "image/png"]])),
    );
    let preparation = json!(["e2", false, 1234.0, null]);
    let compaction = at(
        &document("session_before_compact"),
        "preparation",
        Some(preparation),
    );
    for (what, positional) in [
        ("an event", json!(["input", "hello", null, "rpc"])),
        ("an image", image),
        ("a preparation", compaction),
    ] {
        add(format!("{what} as a positional array"), positional);
    }
    for pair in [
        json!(["a"]),
        json!(["a", "b", "c"]),
        json!([1, "b"]),
        json!(["a", 1]),
        json!("ab"),
    ] {
        let headers = at(
            &document("after_provider_response"),
            "headers",
            Some(json!([pair])),
        );
        add(format!("a header pair {pair}"), headers);
    }
    found
}

/// Every document the decoder must refuse, labelled.
fn undecodable() -> Vec<Refusal> {
    ["", "{", "[]", "null", "42", r#"{"type":"input""#]
        .into_iter()
        .map(|text| (format!("text {text:?}"), text.to_owned()))
        .chain(wrong_properties())
        .chain(wrong_records())
        .collect()
}

/// The message the pinned decoder gives for `text`.
fn decoder_error(text: &str) -> Result<String, String> {
    match EventData::decode(text) {
        Ok(_) => Err(format!("the decoder accepts {text}")),
        Err(error) => Ok(error.to_string()),
    }
}

/// How many times the probe handler has been entered.
fn entered(driver: &impl Driver) -> usize {
    driver
        .transcript()
        .iter()
        .filter(|line| *line == "entry entered none")
        .count()
}

/// Documents that cannot be decoded are refused with the decoder's message, and a compaction
/// without its signal with the attachment's message, before the handler is entered; a callback
/// of another kind is reported.
async fn decode_phases(driver: &mut impl Driver) -> Result<(), String> {
    let probe = driver.identity("event probe")?;
    for (label, text) in undecodable() {
        let signal = Some(driver.lend_signal(false)?);
        let delivery = driver.deliver(probe, &text, "{}", signal).await;
        assert_eq!(delivery, Err(decoder_error(&text)?), "{label}");
    }
    let compact = document("session_before_compact").to_string();
    let delivery = driver.deliver(probe, &compact, "{}", None).await;
    assert_eq!(delivery, Err("missing compaction signal".to_owned()));
    let command = driver.identity("command capture")?;
    let signal = Some(driver.lend_signal(false)?);
    let delivery = driver.deliver(command, &compact, "{}", signal).await?;
    let Decision::Failed(message) = delivery.decision else {
        return Err("a command handler answered an event".to_owned());
    };
    let identity = message.strip_prefix("no callback registered for identity ");
    assert!(
        identity.is_some_and(|digits| digits.parse::<u32>().is_ok()),
        "{message}"
    );
    assert_eq!(
        entered(driver),
        0,
        "no handler was entered by a refused delivery"
    );
    Ok(())
}

/// A value nested in more arrays than the JSON reader's recursion limit of 128 allows.
fn nested() -> Value {
    (0..130).fold(json!(1), |inner, _| json!([inner]))
}

/// The valid document of `kind` and the same document with the unknown property `top` on the
/// event and `inner` on the nested record of the kinds that have one.
fn with_unknown(kind: &str, top: &Value, inner: &Value) -> (Value, Value) {
    let image = json!([{ "data": "AA==", "mimeType": "image/png" }]);
    let event = match kind {
        "input" => at(&document(kind), "images", Some(image)),
        _ => document(kind),
    };
    let mut extended = at(&event, "extra", Some(top.clone()));
    let record = match kind {
        "session_before_compact" => Some("preparation.extra"),
        "input" => Some("images.0.extra"),
        _ => None,
    };
    if let Some(path) = record {
        extended = at(&extended, path, Some(inner.clone()));
    }
    (event, extended)
}

/// Unknown properties on an event, on the preparation of a compaction and on the images of an
/// input are ignored, also when they are nested deeper than the reader's recursion limit.
async fn unknown_properties(driver: &mut impl Driver) -> Result<(), String> {
    for (kind, _) in REQUIRED {
        for (top, inner) in [(json!(true), json!([1])), (nested(), nested())] {
            let (event, extended) = with_unknown(kind, &top, &inner);
            let answer = ask(driver, &extended, &json!({})).await?;
            assert_eq!(
                answer,
                Answer::returned(&event, None),
                "{kind} with unknown properties {top}"
            );
        }
    }
    assert_eq!(
        entered(driver),
        2 * KINDS.len(),
        "each of them entered the handler once"
    );
    Ok(())
}

/// A value that encodes, or fails to with a message.
#[derive(Clone)]
enum Document {
    /// Encodes as a small object.
    Fine,
    /// Fails to encode with this message.
    Broken(&'static str),
}

impl Document {
    /// The encoding it gives, or the message it fails with.
    fn encoded(&self) -> Result<String, String> {
        match self {
            Self::Fine => Ok(r#"{"ok":true}"#.to_owned()),
            Self::Broken(message) => Err((*message).to_owned()),
        }
    }
}

impl Serialize for Document {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Fine => serializer.collect_map([("ok", true)]),
            Self::Broken(message) => Err(S::Error::custom(message)),
        }
    }
}

/// Whichever of an edited event and a result fails to encode, the other part and the handler's
/// own failure are reported unchanged.
fn encoding_failures_stay_apart() {
    let events = [
        None,
        Some(Document::Fine),
        Some(Document::Broken("event encoding failed")),
    ];
    let decisions = [
        Ok(None),
        Ok(Some(Document::Fine)),
        Ok(Some(Document::Broken("result encoding failed"))),
        Err(AUTHORED_FAILURE.to_owned()),
    ];
    for event in &events {
        for decision in &decisions {
            let outcome = outcome(event.clone(), decision.clone());
            let expected = Delivery {
                event: event.as_ref().map(Document::encoded).transpose(),
                decision: match decision {
                    Ok(result) => {
                        Decision::Returned(result.as_ref().map(Document::encoded).transpose())
                    }
                    Err(message) => Decision::Failed(message.clone()),
                },
            };
            assert_eq!(guest_family::delivery(outcome), expected);
        }
    }
}

/// Refused deliveries, then deliveries with unknown properties.
async fn decode_and_unknown(driver: &mut impl Driver) -> Result<(), String> {
    decode_phases(driver).await?;
    unknown_properties(driver).await
}

#[test]
fn maestro_event_failures_keep_decode_callback_and_encoding_phases() -> Result<(), String> {
    encoding_failures_stay_apart();
    on_both_adapters!(decode_and_unknown)
}

/// One way a delivery can end, and the delivery that takes it.
struct Exit {
    /// What the delivery does.
    label: String,
    /// The handler it is addressed to.
    handler: u32,
    /// The event document text.
    event: String,
    /// Whether a signal is lent with it.
    signal: bool,
    /// The directive text, as the working directory of the context.
    directive: String,
    /// Whether the extension refuses the delivery before the handler is entered.
    refused: bool,
}

/// The deliveries that end every way a delivery can, each lent a context and, when it says so,
/// a signal; every event kind but the compaction is lent a signal it must not retain.
fn exits(probe: u32, unregistered: u32, command: u32) -> Vec<Exit> {
    let compact = document("session_before_compact").to_string();
    let input = document("input");
    let exit = |label: &str, handler, event: &str, directive: Value, refused| Exit {
        label: label.to_owned(),
        handler,
        event: event.to_owned(),
        signal: true,
        directive: directive.to_string(),
        refused,
    };
    vec![
        exit("undecodable", probe, "{", json!({}), true),
        Exit {
            signal: false,
            ..exit(
                "compaction without signal",
                probe,
                &compact,
                json!({}),
                true,
            )
        },
        exit(
            "unregistered callback",
            unregistered,
            &compact,
            json!({}),
            false,
        ),
        exit(
            "callback of another kind",
            command,
            &compact,
            json!({}),
            false,
        ),
        exit("success with a signal", probe, &compact, json!({}), false),
        exit("failure", probe, &compact, fails(AUTHORED_FAILURE), false),
        exit(
            "replacement by another kind",
            probe,
            &compact,
            replacing(json!({}), input),
            false,
        ),
    ]
    .into_iter()
    .chain(
        KINDS
            .into_iter()
            .filter(|kind| *kind != "session_before_compact")
            .map(|kind| {
                let label = format!("success ignoring a signal on {kind}");
                exit(&label, probe, &document(kind).to_string(), json!({}), false)
            }),
    )
    .collect()
}

/// Every owner the host lent for a delivery is dropped when the delivery has ended, unless the
/// handler kept it, and dropped when the handler that kept it is released; releasing every
/// handler leaves no owner behind.
async fn lifetimes(driver: &mut impl Driver) -> Result<(), String> {
    let probe = driver.identity("event probe")?;
    let unregistered = driver.unregistered()?;
    let command = driver.identity("command capture")?;
    let baseline = driver.live();
    for exit in exits(probe, unregistered, command) {
        let signal = if exit.signal {
            Some(driver.lend_signal(false)?)
        } else {
            None
        };
        let delivery = driver
            .deliver(exit.handler, &exit.event, &exit.directive, signal)
            .await;
        assert_eq!(delivery.is_err(), exit.refused, "{}", exit.label);
        assert_eq!(driver.live(), baseline, "after {}", exit.label);
    }
    let compact = document("session_before_compact");
    let mut kept = 0;
    for (context, signal) in [(true, false), (false, true), (true, true)] {
        let directive = json!({ "retain": { "context": context, "signal": signal } });
        ask(driver, &compact, &directive).await?;
        kept += usize::from(context) + usize::from(signal);
        assert_eq!(
            driver.live(),
            baseline + kept,
            "after keeping context {context}, signal {signal}"
        );
    }
    assert!(driver.release("event probe").await);
    assert_eq!(
        driver.live(),
        baseline - 1,
        "only the identity of the released handler is gone"
    );
    driver.release_all().await;
    assert_eq!(
        driver.live(),
        0,
        "releasing every handler left no resource behind"
    );
    Ok(())
}

#[test]
fn maestro_event_resources_release_on_every_exit() -> Result<(), String> {
    on_both_adapters!(lifetimes)
}

/// The report the probe handler writes about what it kept, as the host transcript shows it.
fn kept_report(kept: &[(&str, bool)]) -> String {
    let entries: Vec<Value> = kept
        .iter()
        .map(|(cwd, aborted)| json!({ "cwd": cwd, "aborted": aborted }))
        .collect();
    format!("entry kept {}", Value::Array(entries))
}

/// A handler that kept the context and signal of its first invocation reads those after a
/// second invocation with other resources: the first signal, cancelled after both invocations,
/// reads as cancelled and the first context still reports its own directory. Releasing every
/// handler then leaves no resource behind.
async fn retained_capabilities(driver: &mut impl Driver) -> Result<(), String> {
    let baseline = driver.live();
    let compact = document("session_before_compact");
    let retain = json!({ "retain": { "context": true, "signal": true } });
    let first = with_failure(&retain, "first");
    let second = with_failure(&retain, "second");
    let signals = [driver.lend_signal(false)?, driver.lend_signal(false)?];
    for (directive, signal) in [(&first, signals[0]), (&second, signals[1])] {
        let answer = ask_with(driver, &compact, directive, Some(signal)).await?;
        assert_eq!(
            answer.ended,
            Ended::Failed(
                directive["ending"]["fails"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            )
        );
    }
    assert_eq!(
        driver.live(),
        baseline + 4,
        "both invocations kept both owners"
    );
    driver.abort_signal(signals[0])?;
    ask(driver, &document("input"), &json!({ "report": true })).await?;
    let expected = kept_report(&[(&first.to_string(), true), (&second.to_string(), false)]);
    assert_eq!(driver.transcript().last(), Some(&expected));
    assert!(driver.release("event probe").await);
    assert_eq!(
        driver.live(),
        baseline - 1,
        "releasing the handler dropped what it kept"
    );
    driver.release_all().await;
    assert_eq!(
        driver.live(),
        0,
        "releasing every handler left no resource behind"
    );
    Ok(())
}

/// The directive with the handler failing with `message`.
fn with_failure(directive: &Value, message: &str) -> Value {
    let mut failing = directive.clone();
    failing["ending"] = fails(message)["ending"].clone();
    failing
}

#[test]
fn maestro_retained_event_capabilities_keep_original_identity() -> Result<(), String> {
    on_both_adapters!(retained_capabilities)
}

/// A handler that waits for idle on a captured command context stays suspended until the host
/// lets the wait end, and finishes with its edit afterwards.
async fn suspension(driver: &mut impl Driver) -> Result<(), String> {
    driver.run_plain_command("capture", "").await?;
    let probe = driver.identity("event probe")?;
    let event = document("input");
    let directive = marking(json!({ "wait": true }));
    let delivery = driver
        .deliver_held(probe, &event.to_string(), &directive.to_string())
        .await?;
    let transcript = driver.transcript();
    let position = |line: &str| transcript.iter().position(|seen| seen == line);
    let waiting = position("wait-for-idle /work");
    assert!(
        position("entry entered none") < waiting,
        "the handler was entered before it waited"
    );
    assert!(
        waiting < position("command suspended after wait-for-idle /work"),
        "the host saw the wait pending before it released it: {transcript:?}"
    );
    let answer = Answer::of(delivery)?;
    assert_eq!(
        answer,
        Answer {
            event: Some(edited(&event, "text")),
            ended: Ended::Returned(None)
        }
    );
    Ok(())
}

#[test]
fn maestro_event_future_suspends_until_host_release() -> Result<(), String> {
    on_both_adapters!(suspension)
}
