#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn unsigned_text_content_preserves_owned_text_and_omits_signature() {
        use maestro_request::types::TextContent;

        for text in [
            "",
            "first\n雪",
            " \t\0\r\n\u{feff}\u{0085} ",
            "\u{1b}[31mraw\u{1b}[0m",
        ] {
            let content = TextContent::new(text.to_owned());
            assert_eq!(content.text, text);
            assert_eq!(content.text_signature, None);
        }
    }

    #[test]
    fn maestro_diagnostics_preserve_name_without_message() {
        use maestro_request::diagnostics::{
            DiagnosticCode, DiagnosticErrorInfo, DiagnosticInput, extract_diagnostic_error,
            format_thrown_value,
        };
        let mut error = DiagnosticErrorInfo {
            name: Some("Named".into()),
            message: String::new(),
            stack: Some("supplied stack".into()),
            code: Some(DiagnosticCode::Number(42.0)),
        };
        assert_eq!(format_thrown_value(DiagnosticInput::Error(&error)), "Named");
        let extracted = extract_diagnostic_error(DiagnosticInput::Error(&error));
        assert_eq!(extracted.message, "Named");
        assert_eq!(extracted.name.as_deref(), Some("Named"));
        assert_eq!(extracted.stack.as_deref(), Some("supplied stack"));
        assert_eq!(extracted.code, Some(DiagnosticCode::Number(42.0)));
        error.message = "actual message".into();
        error.name = Some(String::new());
        error.code = Some(DiagnosticCode::Text("E_CALLER".into()));
        let extracted = extract_diagnostic_error(DiagnosticInput::Error(&error));
        assert_eq!(extracted.message, "actual message");
        assert_eq!(extracted.name, None);
        assert_eq!(
            extracted.code,
            Some(DiagnosticCode::Text("E_CALLER".into()))
        );
        let text = extract_diagnostic_error(DiagnosticInput::Text("\u{feff} caller text "));
        assert_eq!(text.name.as_deref(), Some("ThrownValue"));
        assert_eq!(text.message, "\u{feff} caller text ");
        assert_eq!(text.to_string(), "\u{feff} caller text ");
        error.message.clear();
        error.name = None;
        assert_eq!(format_thrown_value(DiagnosticInput::Error(&error)), "");
    }

    #[test]
    fn maestro_whole_messages_round_trip_by_role() {
        use maestro_request::types::{AssistantMessage, Message, ToolResultMessage, UserMessage};
        let fixtures = [
            json!({"role":"user","content":"hello","timestamp":12.0}),
            json!({"role":"user","content":[{"type":"text","text":"hello","textSignature":"opaque"},{"type":"image","data":"YQ==","mimeType":"image/png"}],"timestamp":12.0}),
            json!({"role":"assistant","content":[{"type":"thinking","thinking":"reason","thinkingSignature":"sig","redacted":true},{"type":"toolCall","id":"call","name":"tool","arguments":{"x":null},"thoughtSignature":"thought"}],"api":"custom","provider":"custom","model":"m","responseModel":"actual","responseId":"response","diagnostics":[],"usage":{"input":1.0,"output":2.0,"cacheRead":3.0,"cacheWrite":4.0,"totalTokens":10.0,"cost":{"input":0.1,"output":0.2,"cacheRead":0.3,"cacheWrite":0.4,"total":1.0}},"stopReason":"toolUse","errorMessage":"retained","timestamp":13.0}),
            json!({"role":"toolResult","toolCallId":"call","toolName":"tool","content":[{"type":"text","text":"done"}],"details":null,"isError":false,"timestamp":14.0}),
            json!({"role":"toolResult","toolCallId":"call","toolName":"tool","content":[],"isError":true,"timestamp":15.0}),
        ];
        for fixture in &fixtures {
            let message: Message = serde_json::from_value(fixture.clone()).unwrap();
            assert_eq!(serde_json::to_value(message).unwrap(), *fixture);
        }
        let user: UserMessage = serde_json::from_value(fixtures[0].clone()).unwrap();
        assert_eq!(serde_json::to_value(user).unwrap(), fixtures[0]);
        let assistant: AssistantMessage = serde_json::from_value(fixtures[2].clone()).unwrap();
        assert_eq!(serde_json::to_value(assistant).unwrap(), fixtures[2]);
        let tool: ToolResultMessage = serde_json::from_value(fixtures[3].clone()).unwrap();
        assert_eq!(tool.details, Some(serde_json::Value::Null));
        assert_eq!(serde_json::to_value(tool).unwrap(), fixtures[3]);
        let tool: ToolResultMessage = serde_json::from_value(fixtures[4].clone()).unwrap();
        assert_eq!(tool.details, None);
        assert_eq!(serde_json::to_value(tool).unwrap(), fixtures[4]);
    }

    fn model_fixture(api: &str, compat: &serde_json::Value) -> serde_json::Value {
        json!({"id":"custom-model","name":"Custom","api":api,"provider":"custom","baseUrl":"https://fixture.invalid","reasoning":true,"thinkingLevelMap":{"off":null,"minimal":"tiny","low":"low","medium":"medium","high":"high","xhigh":"max"},"input":["text","image"],"cost":{"input":1.0,"output":2.0,"cacheRead":3.0,"cacheWrite":4.0},"contextWindow":1000.0,"maxTokens":50.0,"headers":{"x":"y"},"compat":compat})
    }

    #[test]
    fn custom_protocol_models_round_trip_each_compatibility_family() {
        use maestro_request::types::Model;
        for api in [
            "openai-completions",
            "openai-responses",
            "anthropic-messages",
        ] {
            for compat in [
                match api {
                    "openai-completions" => {
                        json!({"supportsStore":false,"openRouterRouting":{"order":["b","a","b"]}})
                    }
                    "openai-responses" => json!({"sendSessionIdHeader":false}),
                    _ => json!({"supportsEagerToolInputStreaming":true}),
                },
                json!({}),
                json!({"supportsLongCacheRetention":false}),
            ] {
                let mut model: Model = serde_json::from_value(model_fixture(api, &compat)).unwrap();
                model.api = "custom".into();
                let encoded = serde_json::to_value(&model).unwrap();
                let decoded: Model = serde_json::from_value(encoded.clone()).unwrap();
                assert_eq!(serde_json::to_value(&decoded).unwrap(), encoded);
            }
        }
    }

    fn compatibility_contracts() {
        use maestro_request::types::Model;
        let completion = json!({"supportsStore":false,"supportsDeveloperRole":true,"supportsReasoningEffort":false,"supportsUsageInStreaming":true,"maxTokensField":"max_completion_tokens","requiresToolResultName":true,"requiresAssistantAfterToolResult":true,"requiresThinkingAsText":false,"requiresReasoningContentOnAssistantMessages":true,"thinkingFormat":"qwen-chat-template","openRouterRouting":{"allow_fallbacks":false,"require_parameters":true,"data_collection":"deny","zdr":true,"enforce_distillable_text":true,"order":["b","a"],"only":["b"],"ignore":["c"],"quantizations":["fp16"],"sort":{"by":"price","partition":null},"max_price":{"prompt":1.0,"completion":"2","image":3.0,"audio":"4","request":5.0},"preferred_min_throughput":{"p50":1.0,"p75":2.0,"p90":3.0,"p99":4.0},"preferred_max_latency":5.0},"vercelGatewayRouting":{"only":["b"],"order":["b","a"]},"zaiToolStream":false,"supportsStrictMode":false,"cacheControlFormat":"anthropic","sendSessionAffinityHeaders":true,"supportsLongCacheRetention":false});
        for (api, compat) in [
            ("openai-completions", completion),
            (
                "openai-responses",
                json!({"sendSessionIdHeader":false,"supportsLongCacheRetention":true}),
            ),
            (
                "anthropic-messages",
                json!({"supportsEagerToolInputStreaming":false,"supportsLongCacheRetention":true}),
            ),
        ] {
            let fixture = model_fixture(api, &compat);
            let model: Model = serde_json::from_value(fixture.clone()).unwrap();
            assert_eq!(
                model.compat.as_ref().map(|value| &value.0),
                compat.as_object()
            );
            assert_eq!(serde_json::to_value(model).unwrap(), fixture);
            let empty = model_fixture(api, &json!({}));
            let model: Model = serde_json::from_value(empty.clone()).unwrap();
            assert_eq!(serde_json::to_value(model).unwrap(), empty);
        }
    }

    fn event_contracts() {
        use maestro_request::types::{AssistantMessage, AssistantMessageEvent};
        let message = json!({"role":"assistant","content":[],"api":"custom","provider":"custom","model":"m","usage":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"totalTokens":0.0,"cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"total":0.0}},"stopReason":"stop","timestamp":1.0});
        let _: AssistantMessage = serde_json::from_value(message.clone()).unwrap();
        for tag in [
            "start",
            "text_start",
            "text_delta",
            "text_end",
            "thinking_start",
            "thinking_delta",
            "thinking_end",
            "toolcall_start",
            "toolcall_delta",
            "toolcall_end",
            "done",
            "error",
        ] {
            let mut fixture = json!({"type":tag});
            if tag == "done" {
                fixture["reason"] = json!("stop");
                fixture["message"] = message.clone();
            } else if tag == "error" {
                fixture["reason"] = json!("error");
                fixture["error"] = message.clone();
            } else {
                fixture["partial"] = message.clone();
            }
            if tag.contains('_') {
                fixture["contentIndex"] = json!(2);
            }
            if tag.ends_with("delta") {
                fixture["delta"] = json!("delta");
            }
            if tag == "text_end" || tag == "thinking_end" {
                fixture["content"] = json!("content");
            }
            if tag == "toolcall_end" {
                fixture["toolCall"] = json!({"type":"toolCall","id":"c","name":"t","arguments":{}});
            }
            let event: AssistantMessageEvent = serde_json::from_value(fixture.clone()).unwrap();
            assert_eq!(serde_json::to_value(event).unwrap(), fixture);
        }
    }

    fn assert_literals<T: serde::de::DeserializeOwned + serde::Serialize>(literals: &[&str]) {
        for literal in literals {
            let record: T = serde_json::from_value(json!(literal)).unwrap();
            assert_eq!(serde_json::to_value(record).unwrap(), json!(literal));
        }
    }

    fn option_and_reason_contracts() {
        use maestro_request::types::*;
        assert_literals::<ThinkingLevel>(&["minimal", "low", "medium", "high", "xhigh"]);
        assert_literals::<ModelThinkingLevel>(&[
            "off", "minimal", "low", "medium", "high", "xhigh",
        ]);
        assert_literals::<StopReason>(&["stop", "length", "toolUse", "error", "aborted"]);
        assert_literals::<DoneReason>(&["stop", "length", "toolUse"]);
        assert_literals::<ErrorReason>(&["aborted", "error"]);
        assert_literals::<MaxTokensField>(&["max_completion_tokens", "max_tokens"]);
        assert_literals::<TextPhase>(&["commentary", "final_answer"]);
        let mut absent = model_fixture("custom", &json!({}));
        absent.as_object_mut().unwrap().shift_remove("compat");
        absent
            .as_object_mut()
            .unwrap()
            .shift_remove("thinkingLevelMap");
        absent.as_object_mut().unwrap().shift_remove("headers");
        let model: Model = serde_json::from_value(absent.clone()).unwrap();
        assert_eq!(model.compat, None);
        assert_eq!(serde_json::to_value(model).unwrap(), absent);
        assert!(
            serde_json::from_value::<Model>(model_fixture(
                "custom",
                &json!({"supportsStore":true,"sendSessionIdHeader":false})
            ))
            .is_ok()
        );
        let standalone = TextContent {
            text: "standalone".into(),
            text_signature: None,
        };
        assert_eq!(
            serde_json::to_value(standalone).unwrap(),
            json!({"type":"text","text":"standalone"})
        );
    }

    #[test]
    fn maestro_records_keep_wire_contracts() {
        use maestro_request::types::*;
        option_and_reason_contracts();
        compatibility_contracts();
        event_contracts();
        let signature = json!({"v":1,"id":"opaque","phase":"final_answer"});
        let record: TextSignatureV1 = serde_json::from_value(signature.clone()).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), signature);
        assert!(serde_json::from_value::<TextSignatureV1>(json!({"v":2,"id":"bad"})).is_err());
        assert!(
            serde_json::to_value(TextSignatureV1 {
                v: 2,
                id: "bad".into(),
                phase: None
            })
            .is_err()
        );
        for (literal, value) in [
            ("openai", ThinkingFormat::Openai),
            ("openrouter", ThinkingFormat::Openrouter),
            ("deepseek", ThinkingFormat::Deepseek),
            ("zai", ThinkingFormat::Zai),
            ("qwen", ThinkingFormat::Qwen),
            ("qwen-chat-template", ThinkingFormat::QwenChatTemplate),
        ] {
            assert_eq!(serde_json::to_value(value).unwrap(), json!(literal));
        }
        let routing = json!({"sort":"price","data_collection":"allow","preferred_min_throughput":3.0,"preferred_max_latency":{"p50":1.0,"p75":2.0,"p90":3.0,"p99":4.0}});
        let record: OpenRouterRouting = serde_json::from_value(routing.clone()).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), routing);
        for partition in [
            json!({}),
            json!({"partition":null}),
            json!({"partition":"model"}),
        ] {
            let sort: RoutingSort = serde_json::from_value(partition.clone()).unwrap();
            assert_eq!(serde_json::to_value(sort).unwrap(), partition);
        }
        let context = json!({"systemPrompt":"system","messages":[],"tools":[{"name":"t","description":"tool","parameters":{"type":"object"}}]});
        let record: Context = serde_json::from_value(context.clone()).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), context);
        assert!(
            serde_json::from_value::<UserBlock>(json!({"type":"image","text":"wrong"})).is_err()
        );
    }
}

/// A saved assistant message whose usage reports the categories 7/3/2/1 and cost 1.
fn saved_assistant_without_total() -> serde_json::Value {
    serde_json::json!({
        "role": "assistant",
        "content": [{"type": "text", "text": "answer"}],
        "api": "chat",
        "provider": "faux",
        "model": "test",
        "usage": {
            "input": 7.0,
            "output": 3.0,
            "cacheRead": 2.0,
            "cacheWrite": 1.0,
            "cost": {"input": 0.0, "output": 0.0, "cacheRead": 0.0, "cacheWrite": 0.0, "total": 1.0}
        },
        "stopReason": "stop",
        "timestamp": 0.0
    })
}

#[test]
fn saved_usage_preserves_total_presence() {
    use maestro_request::presence::Presence;
    use maestro_request::types::{Message, Usage};

    let saved = saved_assistant_without_total();
    let message: Message = serde_json::from_value(saved.clone()).unwrap();
    let Message::Assistant(assistant) = message else {
        panic!("a saved assistant record decoded as another role");
    };
    assert_eq!(assistant.usage.total_tokens, Presence::Missing);
    let encoded = serde_json::to_value(&assistant.usage).unwrap();
    assert!(!encoded.as_object().unwrap().contains_key("totalTokens"));
    for (key, value) in [
        ("input", 7.0),
        ("output", 3.0),
        ("cacheRead", 2.0),
        ("cacheWrite", 1.0),
    ] {
        assert_eq!(encoded[key], serde_json::json!(value));
    }
    assert_eq!(encoded["cost"]["total"], serde_json::json!(1.0));

    for (supplied, expected) in [
        (serde_json::json!(null), Presence::Null),
        (serde_json::json!(0.0), Presence::Present(0.0)),
        (serde_json::json!(97.0), Presence::Present(97.0)),
        (serde_json::json!(1.5), Presence::Present(1.5)),
        (serde_json::json!(-2.0), Presence::Present(-2.0)),
    ] {
        let mut raw = saved["usage"].clone();
        raw["totalTokens"] = supplied.clone();
        let usage: Usage = serde_json::from_value(raw).unwrap();
        assert_eq!(usage.total_tokens, expected);
        let encoded = serde_json::to_value(&usage).unwrap();
        assert_eq!(encoded["totalTokens"], supplied);
    }

    let signed: Usage = serde_json::from_str(
        "{\"input\":7,\"output\":3,\"cacheRead\":2,\"cacheWrite\":1,\"totalTokens\":-0.0,\"cost\":{\"input\":0,\"output\":0,\"cacheRead\":0,\"cacheWrite\":0,\"total\":1}}",
    )
    .unwrap();
    assert_eq!(
        signed.total_tokens.present().unwrap().to_bits(),
        (-0.0_f64).to_bits()
    );
    assert!(
        serde_json::to_string(&signed)
            .unwrap()
            .contains("\"totalTokens\":-0.0")
    );

    for supplied in [
        serde_json::json!("9"),
        serde_json::json!(true),
        serde_json::json!({}),
        serde_json::json!([]),
    ] {
        let mut raw = saved["usage"].clone();
        raw["totalTokens"] = supplied.clone();
        assert!(
            serde_json::from_value::<Usage>(raw).is_err(),
            "accepted a non-numeric total {supplied}"
        );
    }
}

#[test]
fn usage_defaults_preserve_zero_fields() {
    use maestro_request::presence::Presence;
    use maestro_request::types::Usage;
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/assistant-records.json")).unwrap();
    let usage = Usage::default();
    assert_eq!(rows[0]["kind"], "zero_usage");
    assert_eq!(rows[0]["input"], serde_json::json!({}));
    let expected: Usage = serde_json::from_value(rows[0]["expected"].clone()).unwrap();
    assert_eq!(usage, expected);
    for value in [
        usage.input,
        usage.output,
        usage.cache_read,
        usage.cache_write,
        usage.cost.input,
        usage.cost.output,
        usage.cost.cache_read,
        usage.cost.cache_write,
        usage.cost.total,
    ] {
        assert_eq!(value.to_bits(), 0_f64.to_bits());
    }
    let Presence::Present(total) = usage.total_tokens else {
        panic!("the default usage left its total unset");
    };
    assert_eq!(total.to_bits(), 0_f64.to_bits());
    assert_eq!(
        serde_json::to_value(&usage).unwrap()["totalTokens"],
        serde_json::json!(0.0)
    );
}

#[test]
fn message_diagnostic_constructors_preserve_fields() {
    use maestro_request::diagnostics::{
        DiagnosticErrorInfo, DiagnosticInput, extract_diagnostic_error,
    };
    for name in [
        None,
        Some(""),
        Some("Error"),
        Some("ThrownValue"),
        Some("Named"),
    ] {
        for text in ["", "supplied"] {
            let error = DiagnosticErrorInfo::new(name.map(str::to_owned), text);
            assert_eq!(error.name.as_deref(), name);
            assert_eq!(error.message, text);
            assert_eq!(error.stack, None);
            assert_eq!(error.code, None);
        }
    }
    assert_eq!(
        DiagnosticErrorInfo::error(""),
        DiagnosticErrorInfo::new(Some("Error".into()), "")
    );
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/assistant-records.json")).unwrap();
    for row in rows.as_array().unwrap().iter().skip(1) {
        assert_eq!(row["kind"], "diagnostic_text");
        let actual = extract_diagnostic_error(DiagnosticInput::Text(
            row["input"]["text"].as_str().unwrap(),
        ));
        assert_eq!(serde_json::to_value(actual).unwrap(), row["expected"]);
    }
}

#[test]
fn user_block_text_output_keeps_empty_repeated_and_literal_text() {
    use maestro_request::types::{ImageContent, TextContent, UserBlock};
    let text = |value: &str| UserBlock::Text(TextContent::new(value.into()));
    let image = || {
        UserBlock::Image(ImageContent {
            data: "AA==".into(),
            mime_type: "image/png".into(),
        })
    };
    for (blocks, expected) in [
        (vec![], ""),
        (vec![image()], ""),
        (vec![text(""), text("")], "\n"),
        (
            vec![text("one"), image(), text(""), text("雪"), text("one")],
            "one\n\n雪\none",
        ),
        (
            vec![text("\u{1b}[31mraw\u{1b}[0m"), text(" \t\0 ")],
            "\u{1b}[31mraw\u{1b}[0m\n \t\0 ",
        ),
    ] {
        assert_eq!(UserBlock::text_output(&blocks), expected);
    }
}
