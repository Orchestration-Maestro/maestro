//! Cloud conversation reduction through controlled native event-stream frames.
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/bedrock.rs"]
mod bedrock;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/chat.rs"]
mod chat;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/transport.rs"]
mod transport;

#[path = "support/child_process.rs"]
mod child_process;
use chat::{TestResult, block_on};

use bedrock::corpus;
#[test]
fn thinking_text_and_signatures_follow_separate_updates() -> TestResult {
    block_on(
        false,
        corpus(include_str!("fixtures/cloud_conversation/thinking.json")),
    )
}

#[test]
fn tool_arguments_repair_each_fragment_and_finalize() -> TestResult {
    block_on(false, async {
        corpus(include_str!("fixtures/cloud_conversation/tools.json")).await?;
        canonical_argument_keys()?;
        partial_arguments().await
    })
}

#[test]
fn interleaved_blocks_keep_content_indices_and_shared_result() -> TestResult {
    block_on(
        false,
        corpus(include_str!("fixtures/cloud_conversation/interleaved.json")),
    )
}

#[test]
fn text_blocks_keep_arrival_order_and_reused_numbers() -> TestResult {
    block_on(
        false,
        corpus(include_str!("fixtures/cloud_conversation/text.json")),
    )
}

#[test]
fn unmatched_deltas_and_stops_do_not_create_tool_calls() -> TestResult {
    block_on(
        false,
        corpus(include_str!("fixtures/cloud_conversation/unmatched.json")),
    )
}

#[test]
fn usage_replaces_reports_without_inventing_cache_totals() -> TestResult {
    block_on(
        false,
        corpus(include_str!("fixtures/cloud_conversation/usage.json")),
    )
}

#[test]
fn stop_reasons_preserve_the_terminal_outcome() -> TestResult {
    block_on(
        false,
        corpus(include_str!("fixtures/cloud_conversation/stops.json")),
    )
}

#[test]
fn failures_keep_thinking_and_tool_arguments_without_scratch() -> TestResult {
    block_on(
        false,
        corpus(include_str!("fixtures/cloud_conversation/failures.json")),
    )
}

#[test]
fn empty_and_unknown_streams_conclude_without_synthetic_blocks() -> TestResult {
    block_on(
        false,
        corpus(include_str!("fixtures/cloud_conversation/empty.json")),
    )
}

#[test]
fn stream_fields_keep_sdk_selection_and_failure_phases() -> TestResult {
    block_on(
        false,
        corpus(include_str!("fixtures/cloud_conversation/fields.json")),
    )
}

/// Frames opening a thinking block and a tool with an incomplete argument.
fn partial_frames() -> TestResult<Vec<Vec<u8>>> {
    Ok(vec![
        bedrock::event("messageStart", &serde_json::json!({"role":"assistant"}))?,
        bedrock::event(
            "contentBlockDelta",
            &serde_json::json!({"contentBlockIndex":9,"delta":{"reasoningContent":{"text":"partial"}}}),
        )?,
        bedrock::event(
            "contentBlockDelta",
            &serde_json::json!({"contentBlockIndex":9,"delta":{"reasoningContent":{"signature":"s"}}}),
        )?,
        bedrock::event(
            "contentBlockStart",
            &serde_json::json!({"contentBlockIndex":4,"start":{"toolUse":{"toolUseId":"call","name":"inspect"}}}),
        )?,
        bedrock::event(
            "contentBlockDelta",
            &serde_json::json!({"contentBlockIndex":4,"delta":{"toolUse":{"input":"{\"z\":1,\"a\":\"open"}}}),
        )?,
    ])
}

/// Observe repaired arguments before the gated producer can supply their completion.
async fn partial_arguments() -> TestResult {
    let mut pending = bedrock::held(
        partial_frames()?,
        vec![
            bedrock::event(
                "contentBlockDelta",
                &serde_json::json!({"contentBlockIndex":4,"delta":{"toolUse":{"input":" closed\"}"}}}),
            )?,
            bedrock::event(
                "contentBlockStop",
                &serde_json::json!({"contentBlockIndex":4}),
            )?,
        ],
    );
    let stream = maestro_models::providers::cloud::amazon_bedrock::stream_bedrock(
        bedrock::model("https://bedrock-runtime.us-east-1.amazonaws.com")?,
        bedrock::context("Hello")?,
        Some(bedrock::options(&pending.scripted)),
    );
    pending.received.recv().await.ok_or("no producer")?;
    while let Some(event) = stream.next().await {
        if let maestro_models::AssistantMessageEvent::ToolcallDelta { partial, .. } = event {
            let value = serde_json::to_value(&*partial.read().map_err(|e| e.to_string())?)?;
            assert_eq!(
                value["content"][1]["arguments"],
                serde_json::json!({"z":1,"a":"open"})
            );
            assert_eq!(
                value["content"][1]["arguments"]
                    .as_object()
                    .ok_or("arguments")?
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                ["z", "a"]
            );
            break;
        }
    }
    pending.release.send(()).ok();
    pending.ended.await?;
    let (_, message) = transport::drain(&stream).await?;
    assert_eq!(
        serde_json::to_value(message)?["content"][1]["arguments"],
        serde_json::json!({"z":1,"a":"open closed"})
    );
    Ok(())
}

#[test]
fn cancellation_keeps_partial_blocks_and_allows_a_new_call() -> TestResult {
    block_on(
        false,
        corpus(include_str!(
            "fixtures/cloud_conversation/cancellation.json"
        )),
    )
}

#[test]
fn thinking_tool_results_replay_through_generic_dispatch() -> TestResult {
    if child_process::child_case().is_none() {
        return child_process::rerun(
            "thinking_tool_results_replay_through_generic_dispatch",
            "replay",
            &[("AWS_BEARER_TOKEN_BEDROCK", "fixture-token")],
        );
    }
    block_on(
        false,
        bedrock::replay_corpus(include_str!("fixtures/cloud_conversation/replay.json")),
    )
}

/// Complete, unfinished and nested objects retain canonical numeric keys without sorting text keys.
fn canonical_argument_keys() -> TestResult {
    for input in [
        r#"{"9":9,"z":1,"2":2,"a":2}"#,
        r#"{"9":9,"z":1,"2":2,"a":2"#,
    ] {
        let arguments = maestro_models::parsed_arguments(input);
        assert_eq!(
            arguments.keys().map(String::as_str).collect::<Vec<_>>(),
            ["2", "9", "z", "a"]
        );
    }
    for input in [
        r#"{"outer":{"9":9,"z":1,"2":2,"a":2}}"#,
        r#"{"outer":{"9":9,"z":1,"2":2,"a":2"#,
    ] {
        let arguments = maestro_models::parsed_arguments(input);
        let nested = arguments["outer"].as_object().ok_or("nested object")?;
        assert_eq!(
            nested.keys().map(String::as_str).collect::<Vec<_>>(),
            ["2", "9", "z", "a"]
        );
    }
    Ok(())
}
