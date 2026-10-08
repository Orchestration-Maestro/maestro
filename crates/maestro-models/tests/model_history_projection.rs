#[cfg(test)]
mod tests {
    use maestro_models::*;
    use serde_json::json;

    fn model() -> Model {
        Model {
            id: "target".into(),
            name: "Target".into(),
            api: "anthropic-messages".into(),
            provider: "target-provider".into(),
            base_url: "https://example.invalid".into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost::default(),
            context_window: 1000.0,
            max_tokens: 100.0,
            headers: None,
            compat: None,
        }
    }

    fn assistant(content: Vec<AssistantContent>) -> AssistantMessage {
        let mut message: AssistantMessage = serde_json::from_value(json!({
            "role":"assistant", "content":[], "api":"openai-completions", "provider":"source",
            "model":"source-model", "stopReason":"toolUse", "timestamp":12,
            "responseId":"response", "responseModel":"reported",
        "diagnostics":[{"type":"fixture", "timestamp":9, "details":{"kept":true}}],
            "usage":{"input":1,"output":2,"cacheRead":3,"cacheWrite":4,"totalTokens":10,
            "cost":{"input":1,"output":2,"cacheRead":3,"cacheWrite":4,"total":10}}
        }))
        .unwrap();
        message.content = content;
        message
    }

    fn call(id: &str, name: &str) -> AssistantContent {
        AssistantContent::ToolCall(ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: serde_json::from_value(json!({"nested":{"items":[1,2,3]}})).unwrap(),
            thought_signature: Some("reasoning".into()),
        })
    }

    fn text(value: &str) -> TextContent {
        TextContent {
            text: value.into(),
            text_signature: None,
        }
    }

    fn thinking(value: &str, signature: Option<&str>, redacted: Option<bool>) -> AssistantContent {
        AssistantContent::Thinking(ThinkingContent {
            thinking: value.into(),
            thinking_signature: signature.map(str::to_owned),
            redacted,
        })
    }

    fn result(id: &str, name: &str) -> Message {
        Message::ToolResult(ToolResultMessage {
            tool_call_id: id.into(),
            tool_name: name.into(),
            content: vec![UserBlock::Text(text("answered"))],
            details: Some(json!({"retained":true})),
            is_error: false,
            timestamp: 7.0,
        })
    }

    fn as_assistant(message: &Message) -> &AssistantMessage {
        let Message::Assistant(message) = message else {
            panic!("expected assistant")
        };
        message
    }

    fn as_result(message: &Message) -> &ToolResultMessage {
        let Message::ToolResult(message) = message else {
            panic!("expected result")
        };
        message
    }

    fn assert_missing(message: &Message, id: &str, name: &str) {
        let message = as_result(message);
        assert_eq!(message.tool_call_id, id);
        assert_eq!(message.tool_name, name);
        assert_eq!(
            message.content,
            vec![UserBlock::Text(text("No result provided"))]
        );
        assert!(message.is_error);
        assert_eq!(message.details, None);
    }

    fn normalize_character(c: char) -> char {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            c
        } else {
            '_'
        }
    }

    #[test]
    fn maestro_projects_account_tool_history_for_message_models() {
        let target = model();
        let normalize = |id: &str, _: &Model, _: &AssistantMessage| {
            id.chars()
                .map(normalize_character)
                .take(64)
                .collect::<String>()
        };
        let source = assistant(vec![
            thinking("plan", Some("signed"), None),
            call("account|1", "account"),
        ]);
        let history = vec![Message::Assistant(source)];
        let output = transform_messages(&history, &target, Some(&normalize));
        assert_eq!(
            as_assistant(&output[0]).content[0],
            AssistantContent::Text(text("plan"))
        );
        let AssistantContent::ToolCall(projected_call) = &as_assistant(&output[0]).content[1]
        else {
            panic!()
        };
        assert_eq!(projected_call.id, "account_1");
        assert_eq!(projected_call.thought_signature, None);
        assert_missing(&output[1], "account_1", "account");

        let history = vec![
            Message::Assistant(assistant(vec![
                call("account|1", "account"),
                call("balance|2", "balance"),
            ])),
            result("account|1", "account"),
        ];
        let output = transform_messages(&history, &target, Some(&normalize));
        assert_eq!(output.len(), 3);
        assert_eq!(as_result(&output[1]).tool_call_id, "account_1");
        assert_missing(&output[2], "balance_2", "balance");
        let long = format!("call|{}", "a".repeat(500));
        let output = transform_messages(
            &[Message::Assistant(assistant(vec![call(&long, "long")]))],
            &target,
            Some(&normalize),
        );
        assert_missing(&output[1], &format!("call_{}", "a".repeat(59)), "long");
    }

    #[test]
    fn maestro_replay_preserves_only_compatible_thinking() {
        let target = model();
        for difference in 0..4 {
            for signature in [None, Some(""), Some("signed")] {
                check_thinking_redaction(&target, difference, signature);
            }
        }
        check_mixed_replay_content(&target);
        check_optional_content_signatures(&target);
    }

    fn check_thinking_redaction(target: &Model, difference: u8, signature: Option<&str>) {
        for redacted in [None, Some(false), Some(true)] {
            for value in [
                "",
                " \t\r\n",
                "\u{feff}",
                "\u{00a0}",
                "\u{2028}\u{2029}",
                "\u{0085}",
                "\u{200b}",
                " plan ",
            ] {
                check_thinking_case(target, difference, signature, redacted, value);
            }
        }
    }

    fn check_thinking_case(
        target: &Model,
        difference: u8,
        signature: Option<&str>,
        redacted: Option<bool>,
        value: &str,
    ) {
        let mut source = compatible(
            assistant(vec![thinking(value, signature, redacted)]),
            target,
        );
        match difference {
            1 => source.provider = "other".into(),
            2 => source.api = "other".into(),
            3 => source.model = "other".into(),
            _ => {}
        }
        let output = transform_messages(&[Message::Assistant(source.clone())], target, None);
        let same = difference == 0;
        let blanks = ["", " \t\r\n", "\u{feff}", "\u{00a0}", "\u{2028}\u{2029}"];
        let expected = match (
            redacted == Some(true),
            same,
            signature == Some("signed"),
            blanks.contains(&value),
        ) {
            (true, true, _, _) | (false, true, true, _) | (false, true, _, false) => source.content,
            (true, false, _, _) | (false, _, _, true) => vec![],
            (false, false, _, false) => vec![AssistantContent::Text(text(value))],
        };
        assert_eq!(
            as_assistant(&output[0]).content,
            expected,
            "{difference} {signature:?} {redacted:?} {value:?}"
        );
    }

    fn check_mixed_replay_content(target: &Model) {
        let mut source = assistant(vec![
            AssistantContent::Text(TextContent {
                text: String::new(),
                text_signature: Some("text-signature".into()),
            }),
            thinking(" plan ", None, None),
            call("a", "first"),
            call("b", "second"),
        ]);
        let AssistantContent::ToolCall(second) = &mut source.content[3] else {
            panic!()
        };
        second.thought_signature = Some(String::new());
        let output = transform_messages(&[Message::Assistant(source.clone())], target, None);
        let blocks = &as_assistant(&output[0]).content;
        assert_eq!(blocks[0], AssistantContent::Text(text("")));
        assert_eq!(blocks[1], AssistantContent::Text(text(" plan ")));
        let AssistantContent::ToolCall(second) = &blocks[3] else {
            panic!()
        };
        assert_eq!(second.thought_signature.as_deref(), Some(""));
        let projected = as_assistant(&output[0]);
        let mut metadata = projected.clone();
        metadata.content = source.content.clone();
        assert_eq!(metadata, source);
        source.provider.clone_from(&target.provider);
        source.api.clone_from(&target.api);
        source.model.clone_from(&target.id);
        assert_eq!(
            as_assistant(
                &transform_messages(&[Message::Assistant(source.clone())], target, None)[0]
            ),
            &source
        );
    }

    fn check_optional_content_signatures(target: &Model) {
        for signature in [None, Some(""), Some("signed")] {
            let mut tool = call("a", "tool");
            let AssistantContent::ToolCall(value) = &mut tool else {
                panic!()
            };
            value.thought_signature = signature.map(str::to_owned);
            let source = assistant(vec![
                AssistantContent::Text(TextContent {
                    text: " ".into(),
                    text_signature: signature.map(str::to_owned),
                }),
                tool,
            ]);
            let output = transform_messages(&[Message::Assistant(source.clone())], target, None);
            let blocks = &as_assistant(&output[0]).content;
            assert_eq!(blocks[0], AssistantContent::Text(text(" ")));
            let AssistantContent::ToolCall(value) = &blocks[1] else {
                panic!()
            };
            assert_eq!(
                value.thought_signature.as_deref(),
                signature.filter(|s| s.is_empty())
            );
            let same = compatible(source, target);
            let output = transform_messages(&[Message::Assistant(same.clone())], target, None);
            assert_eq!(as_assistant(&output[0]), &same);
        }
    }

    fn image() -> UserBlock {
        UserBlock::Image(ImageContent {
            data: "payload".repeat(1000),
            mime_type: "image/png".into(),
        })
    }

    #[test]
    fn maestro_projection_downgrades_unsupported_images_once() {
        for (placeholder, tool) in [
            ("(image omitted: model does not support images)", false),
            ("(tool image omitted: model does not support images)", true),
        ] {
            check_image_downgrade(placeholder, tool);
        }
        let user = Message::User(UserMessage {
            content: UserContent::Text("plain".into()),
            timestamp: 1.0,
        });
        assert_eq!(
            transform_messages(std::slice::from_ref(&user), &model(), None),
            vec![user]
        );
    }

    fn check_image_downgrade(placeholder: &str, tool: bool) {
        let mut target = model();
        let signed = UserBlock::Text(TextContent {
            text: "ordinary".into(),
            text_signature: Some("signed".into()),
        });
        let blocks = vec![
            UserBlock::Text(text(placeholder)),
            image(),
            image(),
            signed.clone(),
            image(),
            UserBlock::Text(text(placeholder)),
            image(),
        ];
        let message = if tool {
            let mut value = as_result(&result("unmatched", "tool")).clone();
            value.content = blocks.clone();
            Message::ToolResult(value)
        } else {
            Message::User(UserMessage {
                content: UserContent::Blocks(blocks.clone()),
                timestamp: 12.0,
            })
        };
        target.input = vec![ModelInput::Text];
        let output = transform_messages(std::slice::from_ref(&message), &target, None);
        let expected = vec![
            UserBlock::Text(text(placeholder)),
            signed,
            UserBlock::Text(text(placeholder)),
            UserBlock::Text(text(placeholder)),
        ];
        match (&message, &output[0]) {
            (Message::User(source), Message::User(value)) => {
                assert_eq!(value.content, UserContent::Blocks(expected));
                assert_eq!(value.timestamp.to_bits(), source.timestamp.to_bits());
            }
            (Message::ToolResult(source), Message::ToolResult(value)) => {
                assert_eq!(value.content, expected);
                let mut value = value.clone();
                value.content = blocks;
                assert_eq!(&value, source);
            }
            _ => panic!(),
        }
        target.input.push(ModelInput::Image);
        assert_eq!(
            transform_messages(std::slice::from_ref(&message), &target, None),
            vec![message]
        );
    }

    fn user() -> Message {
        Message::User(UserMessage {
            content: UserContent::Text("next".into()),
            timestamp: 10.0,
        })
    }

    fn compatible(mut source: AssistantMessage, target: &Model) -> AssistantMessage {
        source.provider.clone_from(&target.provider);
        source.api.clone_from(&target.api);
        source.model.clone_from(&target.id);
        source
    }

    #[test]
    fn maestro_projection_pairs_tool_occurrences() {
        let target = model();
        assert!(transform_messages(&[], &target, None).is_empty());
        check_normalizer_inputs(&target);
        check_batch_boundaries(&target);
        check_assistant_outcomes(&target);
        check_empty_original_id(&target);
        check_failed_without_results(&target);
        let source = assistant(vec![call("a", "first"), call("b", "second")]);
        let history = vec![
            Message::Assistant(source),
            result("b", "second"),
            result("a", "first"),
        ];
        let output = transform_messages(&history, &target, None);
        assert_eq!(&output[1..], &history[1..]);
    }

    fn check_empty_original_id(target: &Model) {
        let history = vec![
            Message::Assistant(assistant(vec![call("", "empty")])),
            result("", "empty"),
        ];
        let output = transform_messages(
            &history,
            target,
            Some(&|id, _, _| {
                assert!(id.is_empty());
                "normalized-empty".into()
            }),
        );
        assert_eq!(output.len(), 2);
        assert_eq!(as_result(&output[1]).tool_call_id, "normalized-empty");
    }

    fn check_failed_without_results(target: &Model) {
        for reason in [StopReason::Error, StopReason::Aborted] {
            let mut failed = assistant(vec![call("b", "failed")]);
            failed.stop_reason = reason;
            let history = vec![
                Message::Assistant(assistant(vec![call("a", "prior")])),
                Message::Assistant(failed),
            ];
            let seen = std::cell::RefCell::new(Vec::new());
            let output = transform_messages(
                &history,
                target,
                Some(&|id, _, _| {
                    seen.borrow_mut().push(id.to_owned());
                    id.to_owned()
                }),
            );
            assert_eq!(*seen.borrow(), vec!["a", "b"]);
            assert_eq!(output.len(), 2);
            assert_missing(&output[1], "a", "prior");
        }
    }

    fn check_normalizer_inputs(target: &Model) {
        for normalized in ["a", "", "x"] {
            let first = assistant(vec![call("a", "first")]);
            let second = compatible(assistant(vec![call("a", "second")]), target);
            let seen = std::cell::RefCell::new(Vec::new());
            let normalize = |id: &str, actual: &Model, source: &AssistantMessage| {
                assert_eq!(actual, target);
                assert_eq!(source, &first);
                seen.borrow_mut().push(id.to_owned());
                normalized.into()
            };
            let history = vec![
                Message::Assistant(first.clone()),
                result("a", "first"),
                Message::Assistant(second.clone()),
                result("a", "second"),
            ];
            let output = transform_messages(&history, target, Some(&normalize));
            assert_eq!(*seen.borrow(), vec!["a"]);
            assert_eq!(output.len(), 4);
            assert_eq!(as_result(&output[1]).tool_call_id, normalized);
            assert_eq!(output[2], Message::Assistant(second));
            assert_eq!(output[3], history[3]);
        }
    }

    fn check_batch_boundaries(target: &Model) {
        for count in 0..=2 {
            let mut history = vec![Message::Assistant(assistant(vec![
                call("a", "first"),
                call("a", "second"),
            ]))];
            history.extend((0..count).map(|_| result("a", "supplied")));
            let output = transform_messages(&history, target, None);
            assert_eq!(output.len(), 3);
            if count == 0 {
                assert_missing(&output[1], "a", "first");
            }
            if count < 2 {
                assert_missing(&output[2], "a", "second");
            }
        }
        for boundary in [user(), Message::Assistant(assistant(vec![]))] {
            let history = vec![
                Message::Assistant(assistant(vec![call("a", "first")])),
                boundary.clone(),
                result("a", "late"),
            ];
            let output = transform_messages(&history, target, Some(&|_, _, _| "x".into()));
            assert_eq!(output.len(), 4);
            assert_missing(&output[1], "x", "first");
            assert_eq!(output[2], boundary);
            assert_eq!(output[3], history[2]);
        }
    }

    fn check_assistant_outcomes(target: &Model) {
        for reason in [
            StopReason::Error,
            StopReason::Aborted,
            StopReason::Stop,
            StopReason::Length,
            StopReason::ToolUse,
        ] {
            let failed = matches!(reason, StopReason::Error | StopReason::Aborted);
            let mut source = assistant(vec![call("b", "later")]);
            source.stop_reason = reason;
            let seen = std::cell::RefCell::new(Vec::new());
            let normalize = |id: &str, _: &Model, _: &AssistantMessage| {
                seen.borrow_mut().push(id.to_owned());
                assert!(seen.borrow().len() <= 2);
                format!("{id}-normalized")
            };
            let history = vec![
                Message::Assistant(assistant(vec![call("a", "prior")])),
                Message::Assistant(source),
                result("b", "real"),
                result("extra", "extra"),
                result("b", "duplicate"),
            ];
            let output = transform_messages(&history, target, Some(&normalize));
            assert_eq!(*seen.borrow(), vec!["a", "b"]);
            assert_missing(&output[1], "a-normalized", "prior");
            assert_eq!(output.len(), if failed { 5 } else { 6 });
            let start = if failed { 2 } else { 3 };
            assert_eq!(as_result(&output[start]).tool_call_id, "b-normalized");
            assert_eq!(output[start + 1], history[3]);
            assert_eq!(output[start + 2], history[4]);
        }
    }

    #[test]
    fn maestro_projection_preserves_colliding_call_associations() {
        let target = model();
        for already_equal in [false, true] {
            let ids = if already_equal {
                ["x", "x"]
            } else {
                ["a", "b"]
            };
            for answers in [vec![], vec![1], vec![0], vec![0, 1], vec![1, 0]] {
                check_collision_case(&target, ids, &answers, already_equal);
            }
        }
    }

    fn check_collision_case(
        target: &Model,
        ids: [&str; 2],
        answers: &[usize],
        already_equal: bool,
    ) {
        let mut history = vec![Message::Assistant(assistant(vec![
            call(ids[0], "tool_a"),
            call(ids[1], "tool_b"),
        ]))];
        for &index in answers {
            history.push(result(ids[index], ["real_a", "real_b"][index]));
        }
        let output = transform_messages(&history, target, Some(&|_, _, _| "x".into()));
        assert_eq!(output.len(), 3);
        for (position, &index) in answers.iter().enumerate() {
            let actual = as_result(&output[position + 1]);
            assert_eq!(actual.tool_call_id, "x");
            assert_eq!(actual.tool_name, ["real_a", "real_b"][index]);
            assert_eq!(actual.details, Some(json!({"retained":true})));
        }
        if answers.is_empty() {
            assert_missing(&output[1], "x", "tool_a");
            assert_missing(&output[2], "x", "tool_b");
        } else if answers.len() == 1 {
            let missing = if already_equal || answers == [0] {
                "tool_b"
            } else {
                "tool_a"
            };
            assert_missing(&output[2], "x", missing);
        }
    }

    fn mutate_projected_content(output: &mut [Message]) {
        let Message::Assistant(assistant) = &mut output[1] else {
            panic!()
        };
        let AssistantContent::ToolCall(call) = &mut assistant.content[1] else {
            panic!()
        };
        call.arguments
            .insert("nested".into(), json!({"changed":true}));
        assistant.response_id = Some("changed".into());
        let Message::User(user) = &mut output[0] else {
            panic!()
        };
        user.content = UserContent::Text("changed".into());
    }

    fn now() -> f64 {
        (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            * 1000.0)
            .floor()
    }

    #[test]
    fn maestro_projection_never_mutates_history() {
        let source = assistant(vec![
            thinking("plan", Some("signed"), None),
            call("a", "first"),
            call("b", "second"),
        ]);
        let context = Context {
            system_prompt: Some("system".into()),
            messages: vec![
                Message::User(UserMessage {
                    content: UserContent::Blocks(vec![image()]),
                    timestamp: 1.0,
                }),
                Message::Assistant(source.clone()),
            ],
            tools: Some(vec![Tool {
                name: "first".into(),
                description: "supplied".into(),
                parameters: json!({"type":"object", "properties":{"nested":{"type":"object"}}}),
            }]),
        };
        let original = context.clone();
        let seen = std::cell::RefCell::new(Vec::new());
        let normalize = |id: &str, _: &Model, assistant: &AssistantMessage| {
            assert_eq!(assistant, &source);
            seen.borrow_mut().push(id.to_owned());
            format!("normalized-{id}")
        };
        let before = now();
        let mut output = transform_messages(&context.messages, &model(), Some(&normalize));
        let after = now();
        assert_eq!(*seen.borrow(), vec!["a", "b"]);
        assert_eq!(context, original);
        assert_eq!(output.len(), 4);
        for (position, id, name) in [(2, "normalized-a", "first"), (3, "normalized-b", "second")] {
            assert_missing(&output[position], id, name);
            let result = as_result(&output[position]);
            assert!(result.timestamp >= before && result.timestamp <= after);
            assert!(result.timestamp.fract().abs() < f64::EPSILON);
        }
        mutate_projected_content(&mut output);
        assert_eq!(context, original);
        let mut target = model();
        target.input.push(ModelInput::Image);
        let mut supported = transform_messages(&context.messages, &target, None);
        let Message::User(user) = &mut supported[0] else {
            panic!()
        };
        let UserContent::Blocks(blocks) = &mut user.content else {
            panic!()
        };
        let UserBlock::Image(image) = &mut blocks[0] else {
            panic!()
        };
        image.data.clear();
        assert_eq!(context, original);
    }
}
