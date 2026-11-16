//! User-request summaries assembled by an invoked author handler.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::documents::{Ended, ask};
use super::scenario::Driver;
use serde_json::{Value, json};

/// Summaries filter users and truncate text at Unicode scalar boundaries.
async fn summaries(driver: &mut impl Driver) -> Result<(), String> {
    let rows: Vec<Value> = serde_json::from_str(include_str!("session_summary.json")).unwrap();
    for row in rows {
        let mut event = super::documents::document("session_before_compact");
        event["preparation"]["firstKeptEntryId"] = json!("keep");
        event["preparation"]["tokensBefore"] = json!(-0.0);
        event["preparation"]["messagesToSummarize"] =
            super::session_corpus::materialize(&row["messages"]);
        for message in event["preparation"]["messagesToSummarize"]
            .as_array_mut()
            .unwrap()
        {
            if message.get("timestamp").is_none() {
                message["timestamp"] = json!(0.0);
            }
            if message["role"] == "assistant" {
                let content = message["content"].clone();
                *message = super::stream_events::assistant();
                message["content"] = content;
            }
        }
        let answer = ask(driver, &event, &json!({"sessionAction":"summary"})).await?;
        super::session_corpus::same(&answer.event.unwrap(), &event);
        match answer.ended {
            Ended::Returned(Some(result)) => super::session_corpus::same(
                &result,
                &super::session_corpus::materialize(&row["expected"]),
            ),
            other => panic!("summary handler returned {other:?}"),
        }
    }
    Ok(())
}
#[test]
fn maestro_compaction_summary_keeps_unicode_boundaries() -> Result<(), String> {
    on_both_adapters!(summaries)
}
