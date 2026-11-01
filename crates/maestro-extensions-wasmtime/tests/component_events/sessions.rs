//! Session-before groups: last result, cancellation, selected results and in-hook aborts.
use super::Harness;
use super::cases::{answer, cancellation, encoded, fail, labels, observe, outcome, trace, typed};
use maestro_extensions::{Cancellation, ExtensionRunner, Local};
use maestro_extensions::{
    SessionBeforeCompactEvent, SessionBeforeCompactResult, SessionBeforeForkEvent,
    SessionBeforeForkResult, SessionBeforeSwitchEvent, SessionBeforeSwitchResult,
    SessionBeforeTreeEvent, SessionBeforeTreeResult, runner::RunnerEmitEvent,
};
use serde::Serialize;
use serde_json::{Value, json};

/// A typed session-before event of the named kind, carrying `signal` where its type declares one.
enum Before {
    /// A switch, which declares no preparation or signal.
    Switch(SessionBeforeSwitchEvent),
    /// A fork, which declares no preparation or signal.
    Fork(SessionBeforeForkEvent),
    /// A compaction with its preparation and signal.
    Compact(Box<SessionBeforeCompactEvent>),
    /// A tree navigation with its preparation and signal.
    Tree(SessionBeforeTreeEvent),
}

impl Before {
    /// The named event with recorded-equivalent values.
    fn new(name: &str, signal: Cancellation) -> Self {
        match name {
            "session_before_switch" => Self::Switch(typed(
                json!({"reason":"resume","targetSessionFile":"target"}),
            )),
            "session_before_fork" => {
                Self::Fork(typed(json!({"entryId":"entry","position":"before"})))
            }
            "session_before_compact" => Self::Compact(Box::new(SessionBeforeCompactEvent {
                data: typed(
                    json!({"preparation":{"firstKeptEntryId":"kept","messagesToSummarize":[],"turnPrefixMessages":[],"isSplitTurn":false,"tokensBefore":1.5,"previousSummary":"retained","fileOps":{"read":[],"written":[],"edited":[]},"settings":{"enabled":true,"reserveTokens":2.5,"keepRecentTokens":3.5}},"branchEntries":[],"customInstructions":"instructions"}),
                ),
                signal,
            })),
            _ => Self::Tree(SessionBeforeTreeEvent {
                preparation: typed(
                    json!({"targetId":"target","oldLeafId":"old","commonAncestorId":"common","entriesToSummarize":[],"userWantsSummary":true,"customInstructions":"retained","replaceInstructions":false,"label":"label"}),
                ),
                signal,
            }),
        }
    }

    /// Whether the type declares a preparation and signal.
    fn prepared(&self) -> bool {
        matches!(self, Self::Compact(_) | Self::Tree(_))
    }

    /// The declared preparation, if any.
    fn preparation(&self) -> Value {
        match self {
            Self::Compact(event) => encoded(&event.preparation),
            Self::Tree(event) => encoded(&event.preparation),
            _ => Value::Null,
        }
    }

    /// Emits the event, returning its selected result.
    async fn emit(&mut self, runner: &ExtensionRunner<Local>) -> Option<Value> {
        match self {
            Self::Switch(event) => selected(runner, event).await,
            Self::Fork(event) => selected(runner, event).await,
            Self::Compact(event) => selected(runner, event.as_mut()).await,
            Self::Tree(event) => selected(runner, event).await,
        }
    }

    /// A recorded `{cancel, tag}` result mapped onto the family's declared fields: the tag
    /// travels in the family's own distinguishing field, and a switch declares none.
    fn result(&self, cancel: bool, tag: &str) -> Value {
        match self {
            Self::Switch(_) => encoded(&typed::<SessionBeforeSwitchResult>(
                json!({ "cancel": cancel }),
            )),
            Self::Fork(_) => encoded(&typed::<SessionBeforeForkResult>(
                json!({ "cancel": cancel, "skipConversationRestore": tag != "first" }),
            )),
            Self::Compact(_) => encoded(&typed::<SessionBeforeCompactResult>(
                json!({ "cancel": cancel, "compaction": { "summary": tag, "firstKeptEntryId": "kept", "tokensBefore": 1.5 } }),
            )),
            Self::Tree(_) => encoded(&typed::<SessionBeforeTreeResult>(
                json!({ "cancel": cancel, "label": tag }),
            )),
        }
    }
}

/// Emits a typed session-before event and encodes its selected result.
async fn selected<V: RunnerEmitEvent>(
    runner: &ExtensionRunner<Local>,
    event: &mut V,
) -> Option<Value>
where
    V::Result: Serialize,
{
    runner
        .emit(event)
        .await
        .unwrap()
        .map(|result| encoded(&result))
}

/// The last present result wins and an explicit cancellation stops later handlers.
pub(super) async fn veto(harness: &Harness, input: &Value, mut recorded: Value) -> (Value, Value) {
    let name = input["event"].as_str().unwrap();
    let signal = cancellation(harness);
    let mut event = Before::new(name, signal.clone());
    // An aborted signal shows the handler received the event's own signal.
    signal.abort();
    let cancel = input["outcome"] == "cancel";
    let mut first = vec![trace("a"), answer(event.result(false, "first"))];
    if event.prepared() {
        first.insert(0, observe("aborted"));
    }
    let last = if cancel {
        event.result(true, "stop")
    } else {
        event.result(false, "last")
    };
    let handlers = vec![
        Value::Array(first),
        json!([trace("b"), fail("veto fault")]),
        json!([trace("c"), answer(json!({}))]),
        json!([trace("d"), answer(last)]),
    ];
    let run = harness
        .prepare(name, &[handlers, vec![json!([trace("e")])]])
        .await;
    let before = event.preparation();
    let result = event.emit(&run.runner).await;
    let reported = labels(harness);
    let mut observed = json!({
        "result": result,
        "trace": reported.iter().filter(|value| value.is_string()).collect::<Vec<_>>(),
    });
    let output = recorded["output"].as_object_mut().unwrap();
    let tag = output["result"]["tag"].as_str().unwrap().to_owned();
    let recorded_cancel = output["result"]["cancel"] == true;
    output.insert("result".into(), event.result(recorded_cancel, &tag));
    if event.prepared() {
        observed["signalIdentity"] = json!(reported.contains(&json!(true)));
        observed["preparation"] = json!(event.preparation() == before);
        output.insert("preparation".into(), json!(true));
    } else {
        // A switch or fork declares neither a preparation nor a signal.
        output.shift_remove("signalIdentity");
        output.shift_remove("preparation");
    }
    (outcome(observed, &run), recorded)
}

/// An absent result stays absent; an empty or uncancelled result is still selected.
pub(super) async fn selected_result(
    harness: &Harness,
    input: &Value,
    recorded: Value,
) -> (Value, Value) {
    let name = input["event"].as_str().unwrap();
    let steps = match input["variant"].as_str().unwrap() {
        "absent" => json!([]),
        "empty" => json!([answer(json!({}))]),
        _ => json!([answer(json!({ "cancel": false }))]),
    };
    let run = harness.prepare(name, &[vec![steps]]).await;
    let result = Before::new(name, Cancellation::new())
        .emit(&run.runner)
        .await;
    let observed = json!({ "present": result.is_some(), "result": result });
    (observed, recorded)
}

/// An abort raised inside one handler is seen by the next through the event's signal.
pub(super) async fn abort(harness: &Harness, input: &Value, recorded: Value) -> (Value, Value) {
    let name = input["event"].as_str().unwrap();
    let mut event = Before::new(name, cancellation(harness));
    let handlers = vec![
        json!(["abort", answer(json!({ "cancel": false }))]),
        json!([observe("aborted")]),
    ];
    let run = harness.prepare(name, &[handlers]).await;
    let result = event.emit(&run.runner).await;
    let observed = json!({ "seen": labels(harness), "result": result });
    (observed, recorded)
}
