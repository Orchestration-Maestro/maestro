//! How the answers of the shared handlers read in a transcript, and the conversion of each
//! binding family's outcome record.

use serde_json::Value;

use super::scenario::{Decision, Delivery, Encoded};

/// Defines the conversion of the outcome record of the `guest` module in scope at the call
/// site, which each binding family generates separately.
macro_rules! define_fixtures {
    () => {
        /// The outcome of an event delivery, independent of the bindings.
        pub fn delivery(outcome: guest::EventOutcome) -> super::scenario::Delivery {
            use super::scenario::{Decision, Delivery};
            Delivery {
                event: outcome.event,
                decision: match outcome.decision {
                    guest::Decision::Returned(result) => Decision::Returned(result),
                    guest::Decision::Failed(message) => Decision::Failed(message),
                },
            }
        }
    };
}

/// The document an encoded answer holds, when it holds one.
fn document(encoded: &Encoded) -> Option<Value> {
    let text = encoded.as_ref().ok()?.as_deref()?;
    serde_json::from_str(text).ok()
}

/// The text of a JSON string, or an empty one.
fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// How a handler's answer to an input reads in the transcript.
pub fn input_decision(delivery: &Delivery) -> String {
    match &delivery.decision {
        Decision::Returned(result) => match document(result) {
            Some(result) if result["action"] == "transform" => {
                format!("transform:{}", text(&result["text"]))
            }
            _ => "other".to_owned(),
        },
        Decision::Failed(message) => format!("error:{message}"),
    }
}

/// How a handler's answer to a compaction reads in the transcript.
pub fn compact_decision(delivery: &Delivery) -> String {
    match &delivery.decision {
        Decision::Returned(result) => match document(result) {
            Some(result) => format!(
                "cancel={} summary={}",
                result["cancel"].as_bool().unwrap_or_default(),
                text(&result["compaction"]["summary"])
            ),
            None => "other".to_owned(),
        },
        Decision::Failed(message) => format!("error:{message}"),
    }
}

/// How a handler that edits its event answered.
fn edit_decision(decision: &Decision) -> String {
    match decision {
        Decision::Returned(Ok(None)) => "ok".to_owned(),
        Decision::Returned(_) => "answered".to_owned(),
        Decision::Failed(message) => format!("error:{message}"),
    }
}

/// What the trimming handler decided and the input as it left it.
pub fn trimmed(delivery: &Delivery) -> String {
    let document = document(&delivery.event);
    let edited = document.as_ref().map(|event| text(&event["text"]));
    format!("{} edited={edited:?}", edit_decision(&delivery.decision))
}

/// What the noting handler decided and the preparation as it left it.
pub fn noted(delivery: &Delivery) -> String {
    let document = document(&delivery.event);
    let edited = document
        .as_ref()
        .map(|event| event["preparation"]["previousSummary"].as_str());
    format!("{} edited={edited:?}", edit_decision(&delivery.decision))
}
