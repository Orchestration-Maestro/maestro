#[cfg(test)]
mod tests {
    use serde_json::json;

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

    fn compatibility_contracts() {
        use maestro_request::types::{Model, ModelCompat};
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
            assert!(matches!(
                (&model.api[..], &model.compat),
                (
                    "openai-completions",
                    Some(ModelCompat::OpenAICompletions(_))
                ) | ("openai-responses", Some(ModelCompat::OpenAIResponses(_)))
                    | (
                        "anthropic-messages",
                        Some(ModelCompat::AnthropicMessages(_))
                    )
            ));
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
        absent.as_object_mut().unwrap().remove("compat");
        absent.as_object_mut().unwrap().remove("thinkingLevelMap");
        absent.as_object_mut().unwrap().remove("headers");
        let model: Model = serde_json::from_value(absent.clone()).unwrap();
        assert_eq!(model.compat, None);
        assert_eq!(serde_json::to_value(model).unwrap(), absent);
        assert!(serde_json::from_value::<Model>(model_fixture("custom", &json!({}))).is_err());
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
