#[cfg(test)]
mod tests {
    use maestro_request::types::Model;
    use serde::Deserialize;
    use serde_json::Value;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Case {
        input: Value,
        expected: Value,
    }

    #[test]
    fn mixed_option_objects_round_trip_on_every_protocol() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("fixtures/model_compatibility.json")).unwrap();
        assert_eq!(cases.len(), 222);
        for (index, case) in cases.into_iter().enumerate() {
            let model: Model = serde_json::from_value(case.input)
                .unwrap_or_else(|_| panic!("decode case {index}"));
            let wire = serde_json::to_value(&model).unwrap();
            for (key, expected) in case.expected.as_object().unwrap() {
                assert_eq!(&wire[key], expected, "case {index} field {key}");
            }
            let decoded: Model = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(decoded).unwrap(), wire, "case {index}");
        }
    }

    fn descriptor(compat: Value) -> Model {
        let mut input: Value =
            serde_json::from_str(include_str!("fixtures/model_compatibility.json")).unwrap();
        let mut input = input[0]["input"].take();
        input["compat"] = compat;
        serde_json::from_value(input).unwrap()
    }

    #[test]
    fn compatibility_field_order_and_duplicate_values_survive() {
        let compat: Value = serde_json::from_str(
            r#"{"z":0,"nested":{"b":["two","one","two"],"a":1},"a":2,"\u007a":3}"#,
        )
        .unwrap();
        let model = descriptor(compat);
        let wire = serde_json::to_value(model).unwrap();
        assert_eq!(
            wire["compat"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            ["z", "nested", "a"]
        );
        assert_eq!(
            wire["compat"]["nested"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            ["b", "a"]
        );
        assert_eq!(wire["compat"]["z"], 3);
        assert_eq!(
            wire["compat"]["nested"]["b"],
            serde_json::json!(["two", "one", "two"])
        );
    }

    #[test]
    fn absent_and_empty_compatibility_remain_distinct() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("fixtures/model_compatibility.json")).unwrap();
        let absent: Model = serde_json::from_value(cases[0].input.clone()).unwrap();
        assert!(
            serde_json::to_value(absent)
                .unwrap()
                .get("compat")
                .is_none()
        );
        assert_eq!(
            serde_json::to_value(descriptor(serde_json::json!({}))).unwrap()["compat"],
            serde_json::json!({})
        );
        assert_eq!(
            serde_json::to_value(descriptor(serde_json::json!({"open":null}))).unwrap()["compat"],
            serde_json::json!({"open":null})
        );
    }

    #[test]
    fn compatibility_values_keep_decimal_precision_and_zero_sign() {
        let expected = [0.845_512_408_225_570_1_f64, -0.0, 9_007_199_254_740_991.0];
        let model = descriptor(serde_json::json!({"numbers":expected}));
        let wire = serde_json::to_string(&model).unwrap();
        let restored: Model = serde_json::from_str(&wire).unwrap();
        for (actual, expected) in restored.compat.unwrap().0["numbers"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected)
        {
            assert_eq!(actual.as_f64().unwrap().to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn compatibility_edits_preserve_unrelated_values() {
        let mut model = descriptor(
            serde_json::json!({"old":false,"remove":1,"nested":{"values":["b","a","b"]}}),
        );
        let compat = &mut model.compat.as_mut().unwrap().0;
        compat.insert("old".into(), Value::Bool(true));
        compat.insert("new".into(), Value::Null);
        compat.remove("remove");
        assert_eq!(
            serde_json::to_value(model).unwrap()["compat"],
            serde_json::json!({"old":true,"nested":{"values":["b","a","b"]},"new":null})
        );
    }

    #[test]
    fn typed_compatibility_builders_preserve_declared_fields() {
        use maestro_request::types::*;
        let cases = [
            serde_json::json!({"supportsStore":false,"supportsDeveloperRole":true,"supportsReasoningEffort":false,"supportsUsageInStreaming":true,"maxTokensField":"max_tokens","requiresToolResultName":true,"requiresAssistantAfterToolResult":true,"requiresThinkingAsText":false,"requiresReasoningContentOnAssistantMessages":true,"thinkingFormat":"qwen-chat-template","openRouterRouting":{"allow_fallbacks":false,"require_parameters":true,"data_collection":"deny","zdr":true,"enforce_distillable_text":true,"order":["b","a","b"],"only":[],"ignore":["c"],"quantizations":["fp16"],"sort":{"by":"price","partition":null},"max_price":{"prompt":1.0,"completion":"2","image":3.0,"audio":"4","request":5.0},"preferred_min_throughput":{"p50":1.0,"p75":2.0,"p90":3.0,"p99":4.0},"preferred_max_latency":5.0},"vercelGatewayRouting":{"only":[],"order":["b","a","b"]},"zaiToolStream":false,"supportsStrictMode":false,"cacheControlFormat":"anthropic","sendSessionAffinityHeaders":true,"supportsLongCacheRetention":false}),
            serde_json::json!({"sendSessionIdHeader":false,"supportsLongCacheRetention":true}),
            serde_json::json!({"supportsEagerToolInputStreaming":false,"supportsLongCacheRetention":true}),
        ];
        let converted: [ModelCompat; 3] = [
            serde_json::from_value::<OpenAICompletionsCompat>(cases[0].clone())
                .unwrap()
                .into(),
            serde_json::from_value::<OpenAIResponsesCompat>(cases[1].clone())
                .unwrap()
                .into(),
            serde_json::from_value::<AnthropicMessagesCompat>(cases[2].clone())
                .unwrap()
                .into(),
        ];
        for (actual, expected) in converted.into_iter().zip(cases) {
            assert_eq!(serde_json::to_value(actual).unwrap(), expected);
        }
        for format in [
            "openai",
            "openrouter",
            "deepseek",
            "zai",
            "qwen",
            "qwen-chat-template",
        ] {
            let expected = serde_json::json!({"thinkingFormat":format});
            let typed: OpenAICompletionsCompat = serde_json::from_value(expected.clone()).unwrap();
            assert_eq!(
                serde_json::to_value(ModelCompat::from(typed)).unwrap(),
                expected
            );
        }
        for routing in [
            serde_json::json!({"sort":"price","data_collection":"allow","preferred_min_throughput":3.0,"preferred_max_latency":{"p50":1.0}}),
            serde_json::json!({"sort":{}}),
            serde_json::json!({"sort":{"partition":"model"}}),
        ] {
            let expected = serde_json::json!({"maxTokensField":"max_completion_tokens","openRouterRouting":routing});
            let typed: OpenAICompletionsCompat = serde_json::from_value(expected.clone()).unwrap();
            assert_eq!(
                serde_json::to_value(ModelCompat::from(typed)).unwrap(),
                expected
            );
        }
        assert_eq!(
            ModelCompat::from(OpenAICompletionsCompat::default()),
            ModelCompat::default()
        );
    }
}
