//! Session totals and cost buckets over assistant, tool and summary usage.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, Scripted, TS, answer, assistant, harness, on_task_set, prompt_to_end,
    script_responses, sdk_definition, start_prompt, tool_definition, usage, user,
};
use maestro_app::agent_session::PromptOptions;
use maestro_app::{BranchSummaryOptions, CompactionOptions, Presence};
use maestro_extensions::{
    CompactionResult, ExtensionAPI, ExtensionFactory, SessionBeforeCompactEvent,
    SessionBeforeCompactResult, SessionBeforeTreeEvent, SessionBeforeTreeResult,
    SessionBeforeTreeSummary,
};
use maestro_models::Local;
use maestro_session::session_manager::{NavigateTreeOptions, ReadonlySessionManager};
use serde_json::{Value, json};
use std::rc::Rc;

/// A usage record reporting `input` and `output` tokens at `cost`, as stored.
fn reported(input: u64, output: u64, cost: f64) -> Value {
    let mut value = usage([input, output, 0, 0], cost, Some(input + output));
    value["cost"]["output"] = json!(cost);
    value
}

/// A tool result carrying the usage of the model calls it made.
fn tool_result(usage: &Value) -> Value {
    json!({"role": "toolResult", "toolCallId": "t", "toolName": "nested", "isError": false,
        "content": [{"type": "text", "text": "done"}], "usage": usage, "timestamp": TS + 3})
}

/// The stored usage as the typed record the session manager takes.
fn typed(usage: &Value) -> Presence<maestro_app::Usage> {
    Presence::Present(serde_json::from_value(usage.clone()).unwrap())
}

#[test]
fn tool_and_summary_usage_has_its_own_bucket_that_reconciles_with_the_total() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let (answer, tool, compaction, branch) = (
            reported(100, 50, 0.5),
            reported(10, 5, 0.1),
            reported(20, 10, 0.2),
            reported(30, 15, 0.3),
        );
        {
            let mut manager = h.session.session_manager().borrow_mut();
            let root = manager.append_message(user(&json!("question"), 1)).unwrap();
            manager
                .append_message(assistant("reply", 2, &json!({"usage": answer})))
                .unwrap();
            manager.append_message(tool_result(&tool)).unwrap();
            manager
                .append_compaction(
                    "summary",
                    &root,
                    900.0,
                    CompactionOptions {
                        usage: typed(&compaction),
                        ..CompactionOptions::default()
                    },
                )
                .unwrap();
            manager
                .branch_with_summary(
                    Some(&root),
                    "abandoned",
                    BranchSummaryOptions {
                        usage: typed(&branch),
                        ..BranchSummaryOptions::default()
                    },
                )
                .unwrap();
        }
        let stats = h.session.get_session_stats().unwrap();
        assert_eq!(stats.tokens.input.to_bits(), 160.0_f64.to_bits());
        assert_eq!(stats.tokens.output.to_bits(), 80.0_f64.to_bits());
        assert_eq!(stats.tokens.total.to_bits(), 240.0_f64.to_bits());
        assert!((stats.cost - 1.1).abs() < 1e-9, "{}", stats.cost);
        let buckets = h.session.get_usage_cost_breakdown().unwrap();
        let keys: Vec<_> = buckets.iter().map(|bucket| bucket.key.as_str()).collect();
        assert_eq!(keys, ["Tools/summaries", "faux/plain"]);
        assert!((buckets[0].cost - 0.6).abs() < 1e-9, "{}", buckets[0].cost);
        assert_eq!(buckets[0].tokens.to_bits(), 90.0_f64.to_bits());
        assert_eq!(buckets[1].tokens.to_bits(), 150.0_f64.to_bits());
        let cost: f64 = buckets.iter().map(|bucket| bucket.cost).sum();
        let tokens: f64 = buckets.iter().map(|bucket| bucket.tokens).sum();
        assert!((cost - stats.cost).abs() < 1e-9);
        assert_eq!(tokens.to_bits(), stats.tokens.total.to_bits());
    });
}

/// A session after two completed prompt turns, whose cut keeps almost nothing, with the
/// summary request of the next compaction or navigation answered by the scripted provider.
async fn conversation() -> Harness {
    let h = harness(HarnessOptions {
        settings: json!({"compaction": {"keepRecentTokens": 1}}),
        responses: Some(vec![Scripted::text("one"), Scripted::text("two")]),
        ..HarnessOptions::default()
    })
    .await;
    for text in ["First message", "Second message"] {
        prompt_to_end(&h, text, PromptOptions::default()).await;
    }
    let summaries = vec![answer("Summary", None, None), answer("Prefix", None, None)];
    script_responses(&h, summaries);
    h
}

/// The tokens a stored usage record counts.
fn counted(usage: &Value) -> f64 {
    ["input", "output", "cacheRead", "cacheWrite"]
        .iter()
        .map(|field| usage[field].as_f64().unwrap())
        .sum()
}

/// The stored entry of `kind` that was added last.
fn last_entry(h: &Harness, kind: &str) -> Value {
    let manager = h.session.session_manager().borrow();
    let entries = manager.get_entries();
    (*entries
        .iter()
        .rev()
        .find(|entry| entry["type"] == kind)
        .unwrap())
    .clone()
}

/// The bucket of tool and summary usage.
fn summary_bucket(h: &Harness) -> maestro_app::UsageCostBreakdownEntry {
    let buckets = h.session.get_usage_cost_breakdown().unwrap();
    buckets
        .into_iter()
        .find(|bucket| bucket.key == "Tools/summaries")
        .unwrap()
}

#[test]
fn manual_compaction_records_the_usage_of_its_summary_request() {
    on_task_set(|_| async {
        let h = conversation().await;
        let result = h.session.compact(None).await.unwrap();
        let Presence::Present(reported) = result.usage else {
            panic!("the compaction reports no usage");
        };
        let stored = last_entry(&h, "compaction");
        assert_eq!(stored["usage"], serde_json::to_value(&reported).unwrap());
        assert!(stored["usage"].is_object(), "{stored}");
        let tokens = counted(&stored["usage"]);
        assert!(tokens > 0.0);
        assert_eq!(summary_bucket(&h).tokens.to_bits(), tokens.to_bits());
    });
}

#[test]
fn branch_summary_records_the_usage_of_its_summary_request() {
    on_task_set(|_| async {
        let h = conversation().await;
        let first = {
            let manager = h.session.session_manager().borrow();
            let entries = manager.get_entries();
            entries
                .iter()
                .find(|entry| entry["type"] == "message")
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let options = NavigateTreeOptions {
            summarize: Some(true),
            ..NavigateTreeOptions::default()
        };
        h.session.navigate_tree(&first, options).await.unwrap();
        let stored = last_entry(&h, "branch_summary");
        assert!(stored["usage"].is_object(), "{stored}");
        let tokens = counted(&stored["usage"]);
        assert!(tokens > 0.0);
        assert_eq!(summary_bucket(&h).tokens.to_bits(), tokens.to_bits());
    });
}

#[test]
fn tool_usage_reaches_the_stored_result_and_the_summary_bucket() {
    on_task_set(|_| async {
        let mut nested = tool_definition("echo", None, None);
        nested.definition.parameters =
            json!({"type": "object", "properties": {"text": {"type": "string"}}}).into();
        nested.execute = std::rc::Rc::new(|_, _, _, _, _| {
            Box::pin(async {
                Ok(maestro_agent::AgentToolResult {
                    content: vec![],
                    details: None,
                    terminate: None,
                    usage: serde_json::from_value(reported(10, 5, 0.1)).unwrap(),
                })
            })
        });
        let h = harness(HarnessOptions {
            custom_tools: vec![sdk_definition(nested)],
            responses: Some(vec![Scripted::echo_call("x"), Scripted::text("done")]),
            ..HarnessOptions::default()
        })
        .await;
        start_prompt(&h, "go").await.unwrap().unwrap();
        h.session.agent().wait_for_idle().await;
        let stored = last_entry(&h, "message");
        let results: Vec<Value> = h
            .session
            .session_manager()
            .borrow()
            .get_entries()
            .iter()
            .filter(|entry| entry["message"]["role"] == "toolResult")
            .map(|entry| entry["message"]["usage"].clone())
            .collect();
        assert_eq!(results.len(), 1, "{stored}");
        assert_eq!(results[0]["input"], json!(10.0));
        let bucket = summary_bucket(&h);
        assert_eq!(bucket.tokens.to_bits(), 15.0_f64.to_bits());
        assert!((bucket.cost - 0.1).abs() < 1e-9);
    });
}

/// An extension that supplies every compaction and tree summary, reporting `usage` for each.
fn supplying_extension(usage: &Value) -> ExtensionFactory<Local> {
    let usage: maestro_app::Usage = serde_json::from_value(usage.clone()).unwrap();
    Rc::new(move |api: ExtensionAPI<Local>| {
        let compaction_usage = usage.clone();
        let compaction = api.on::<SessionBeforeCompactEvent, SessionBeforeCompactResult>(
            "session_before_compact".into(),
            Rc::new(move |event, _| {
                let preparation = &event.data.preparation;
                Box::pin(std::future::ready(Ok(Some(SessionBeforeCompactResult {
                    cancel: Presence::Missing,
                    compaction: Presence::Present(CompactionResult {
                        summary: "supplied".into(),
                        first_kept_entry_id: preparation.first_kept_entry_id.clone(),
                        tokens_before: preparation.tokens_before,
                        details: Presence::Present(json!({})),
                        usage: Presence::Present(compaction_usage.clone()),
                    }),
                }))))
            }),
        );
        let tree_usage = usage.clone();
        let tree = api.on::<SessionBeforeTreeEvent, SessionBeforeTreeResult>(
            "session_before_tree".into(),
            Rc::new(move |_, _| {
                Box::pin(std::future::ready(Ok(Some(SessionBeforeTreeResult {
                    summary: Presence::Present(SessionBeforeTreeSummary {
                        summary: "supplied".into(),
                        details: Presence::Missing,
                        usage: Presence::Present(tree_usage.clone()),
                    }),
                    cancel: Presence::Missing,
                    custom_instructions: Presence::Missing,
                    replace_instructions: Presence::Missing,
                    label: Presence::Missing,
                }))))
            }),
        );
        drop((compaction, tree));
        Box::pin(std::future::ready(Ok(())))
    })
}

#[test]
fn supplied_compaction_and_tree_summaries_keep_their_reported_usage() {
    on_task_set(|_| async {
        let reported = reported(25, 5, 0.4);
        let h = harness(HarnessOptions {
            settings: json!({"compaction": {"keepRecentTokens": 1}}),
            responses: Some(vec![Scripted::text("one"), Scripted::text("two")]),
            factories: vec![supplying_extension(&reported)],
            ..HarnessOptions::default()
        })
        .await;
        for text in ["First message", "Second message"] {
            prompt_to_end(&h, text, PromptOptions::default()).await;
        }
        h.session.compact(None).await.unwrap();
        let first = {
            let manager = h.session.session_manager().borrow();
            let entries = manager.get_entries();
            entries
                .iter()
                .find(|entry| entry["type"] == "message")
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let options = NavigateTreeOptions {
            summarize: Some(true),
            ..NavigateTreeOptions::default()
        };
        h.session.navigate_tree(&first, options).await.unwrap();
        let expected =
            serde_json::to_value(serde_json::from_value::<maestro_app::Usage>(reported).unwrap())
                .unwrap();
        assert_eq!(last_entry(&h, "compaction")["usage"], expected);
        assert_eq!(last_entry(&h, "branch_summary")["usage"], expected);
    });
}
