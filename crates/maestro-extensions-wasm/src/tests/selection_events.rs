//! Model and reasoning selections through both author adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::documents::{Answer, ask};
use super::scenario::Driver;
use serde_json::{Value, json};

/// A noncatalog descriptor with ordered headers and supplied prices.
pub(super) fn model() -> Value {
    json!({"id":"custom","name":"Custom Ω","api":"openai-completions","provider":"custom","baseUrl":"not a URL",
        "reasoning":true,"thinkingLevelMap":{"off":null,"minimal":"tiny","xhigh":"max"},"input":["image","text","image"],
        "cost":{"input":-0.0,"output":2.5,"cacheRead":3.5,"cacheWrite":4.5},"contextWindow":123.5,"maxTokens":234.5,
        "headers":{"z":"first","a":"second"},"compat":{"supportsStore":false,"supportsStrictMode":true,
        "thinkingFormat":"qwen-chat-template","maxTokensField":"max_tokens","cacheControlFormat":"anthropic",
        "openRouterRouting":{"order":["z","a","z"],"max_price":{"prompt":"007","completion":-0.0},
        "sort":{"by":"cost","partition":null},"preferred_min_throughput":1.5,"preferred_max_latency":{"p50":2.5,"p99":3.5}},
        "vercelGatewayRouting":{"only":["b","a","b"]}}})
}

/// Selection source and previous state do not infer capabilities or defaults.
async fn selections(driver: &mut impl Driver) -> Result<(), String> {
    for source in ["set", "cycle", "restore"] {
        for previous in [None, Some(Value::Null), Some(model())] {
            let mut event = json!({"type":"model_select","model":model(),"source":source});
            if let Some(previous) = previous {
                event["previousModel"] = previous;
            }
            let answer = ask(driver, &event, &json!({})).await?;
            let headers = answer.event.as_ref().ok_or("missing event")?["model"]["headers"]
                .as_object()
                .ok_or("missing headers")?;
            assert_eq!(
                headers.keys().map(String::as_str).collect::<Vec<_>>(),
                ["z", "a"]
            );
            assert_eq!(answer, Answer::returned(&event, None));
        }
    }
    for level in ["off", "minimal", "low", "medium", "high", "xhigh"] {
        for previous in ["off", "minimal", "low", "medium", "high", "xhigh"] {
            let event =
                json!({"type":"thinking_level_select","level":level,"previousLevel":previous});
            assert_eq!(
                ask(driver, &event, &json!({})).await?,
                Answer::returned(&event, None)
            );
        }
    }
    Ok(())
}

/// Every protocol-specific option uses its owning shared descriptor.
async fn compatibility(driver: &mut impl Driver) -> Result<(), String> {
    for (api, fields) in [
        (
            "openai-completions",
            &[
                "supportsStore",
                "supportsDeveloperRole",
                "supportsReasoningEffort",
                "supportsUsageInStreaming",
                "requiresToolResultName",
                "requiresAssistantAfterToolResult",
                "requiresThinkingAsText",
                "requiresReasoningContentOnAssistantMessages",
                "zaiToolStream",
                "supportsStrictMode",
                "sendSessionAffinityHeaders",
                "supportsLongCacheRetention",
            ][..],
        ),
        (
            "openai-responses",
            &["sendSessionIdHeader", "supportsLongCacheRetention"][..],
        ),
        (
            "anthropic-messages",
            &[
                "supportsEagerToolInputStreaming",
                "supportsLongCacheRetention",
            ][..],
        ),
    ] {
        for &field in fields {
            for flag in [false, true] {
                let mut descriptor = model();
                descriptor["api"] = json!(api);
                descriptor["compat"] = json!({field:flag});
                let event = json!({"type":"model_select","model":descriptor,"source":"cycle"});
                assert_eq!(
                    ask(driver, &event, &json!({})).await?,
                    Answer::returned(&event, None)
                );
            }
        }
    }
    compatibility_literals(driver).await
}

/// Literal compatibility choices use the shared descriptor decoder.
async fn compatibility_literals(driver: &mut impl Driver) -> Result<(), String> {
    for (field, values) in [
        (
            "maxTokensField",
            &["max_completion_tokens", "max_tokens"][..],
        ),
        (
            "thinkingFormat",
            &[
                "openai",
                "openrouter",
                "deepseek",
                "zai",
                "qwen",
                "qwen-chat-template",
            ][..],
        ),
        ("cacheControlFormat", &["anthropic"][..]),
    ] {
        for value in values {
            let mut descriptor = model();
            descriptor["compat"] = json!({field:value});
            let event = json!({"type":"model_select","model":descriptor,"source":"restore"});
            assert_eq!(
                ask(driver, &event, &json!({})).await?,
                Answer::returned(&event, None)
            );
        }
    }
    Ok(())
}

/// Routing union alternatives are retained without numeric or list normalization.
async fn routing(driver: &mut impl Driver) -> Result<(), String> {
    for (field, values) in [
        (
            "sort",
            vec![
                json!(""),
                json!("cost"),
                json!({}),
                json!({"by":"latency"}),
                json!({"partition":null}),
                json!({"partition":""}),
            ],
        ),
        (
            "preferred_min_throughput",
            vec![
                json!(-0.0),
                json!({}),
                json!({"p50":1.5}),
                json!({"p75":2.5}),
                json!({"p90":3.5}),
                json!({"p99":4.5}),
                json!({"p50":1.5,"p75":2.5,"p90":3.5,"p99":4.5}),
            ],
        ),
        (
            "preferred_max_latency",
            vec![
                json!(2.5),
                json!({}),
                json!({"p50":4.5,"p75":3.5,"p90":2.5,"p99":1.5}),
            ],
        ),
        ("data_collection", vec![json!("deny"), json!("allow")]),
    ] {
        for value in values {
            let mut descriptor = model();
            descriptor["compat"] = json!({"openRouterRouting":{field:value}});
            let event = json!({"type":"model_select","model":descriptor,"source":"set"});
            assert_eq!(
                ask(driver, &event, &json!({})).await?,
                Answer::returned(&event, None)
            );
        }
    }
    Ok(())
}

/// Numeric-looking price and diagnostic strings remain distinct from numeric branches.
async fn number_or_string(driver: &mut impl Driver) -> Result<(), String> {
    for field in ["prompt", "completion", "image", "audio", "request"] {
        for value in [json!("0"), json!("007"), json!(-0.0), json!(1.5)] {
            let mut descriptor = model();
            descriptor["compat"] = json!({"openRouterRouting":{"max_price":{field:value}}});
            let event = json!({"type":"model_select","model":descriptor,"source":"restore"});
            let answer = ask(driver, &event, &json!({})).await?;
            let returned = &answer.event.as_ref().ok_or("missing event")?["model"]["compat"]["openRouterRouting"]
                ["max_price"][field];
            if let Some(number) = value.as_f64() {
                assert_eq!(returned.as_f64().unwrap().to_bits(), number.to_bits());
            }
            assert_eq!(answer, Answer::returned(&event, None));
        }
    }
    for code in [json!("007"), json!(-0.0), json!(42.5)] {
        let mut message = super::stream_events::assistant();
        message["diagnostics"][0]["error"]["code"] = code.clone();
        let event = json!({"type":"message_start","message":message});
        let answer = ask(driver, &event, &json!({})).await?;
        if let Some(number) = code.as_f64() {
            assert_eq!(
                answer.event.as_ref().unwrap()["message"]["diagnostics"][0]["error"]["code"]
                    .as_f64()
                    .unwrap()
                    .to_bits(),
                number.to_bits()
            );
        }
        assert_eq!(answer, Answer::returned(&event, None));
    }
    Ok(())
}

#[test]
fn maestro_model_compat_keeps_each_declared_option() -> Result<(), String> {
    on_both_adapters!(compatibility)
}
#[test]
fn maestro_routing_unions_keep_shape_and_presence() -> Result<(), String> {
    on_both_adapters!(routing)
}
#[test]
fn maestro_number_or_string_fields_keep_ambiguous_text() -> Result<(), String> {
    on_both_adapters!(number_or_string)
}

#[test]
fn maestro_selection_events_keep_model_and_thinking_metadata() -> Result<(), String> {
    on_both_adapters!(selections)
}
