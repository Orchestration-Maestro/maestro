#[cfg(test)]
/// Offline public-interface behavior checks.
mod tests {
    use maestro_models::*;

    #[test]
    /// Exercise builders combine content through the public invocation boundary.
    fn maestro_builders_combine_content() {
        let message = faux_assistant_message(
            vec![
                AssistantContent::Thinking(faux_thinking("plan")),
                AssistantContent::ToolCall(faux_tool_call(
                    "lookup",
                    JsonObject::new(),
                    FauxToolCallOptions::default(),
                )),
                AssistantContent::Text(faux_text("answer")),
            ],
            FauxAssistantMessageOptions {
                stop_reason: Some(StopReason::ToolUse),
                ..Default::default()
            },
        );
        assert!(
            matches!(&message.content[0], AssistantContent::Thinking(value) if value.thinking == "plan")
        );
        assert!(
            matches!(&message.content[1], AssistantContent::ToolCall(value) if value.name == "lookup" && value.arguments.is_empty())
        );
        assert!(
            matches!(&message.content[2], AssistantContent::Text(value) if value.text == "answer")
        );
        assert_eq!(message.stop_reason, StopReason::ToolUse);
    }

    /// Exercise wait through the public invocation boundary.
    fn wait<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future)
    }
    /// Exercise context through the public invocation boundary.
    fn context(text: &str) -> Context {
        Context {
            system_prompt: None,
            messages: vec![Message::User(UserMessage {
                content: UserContent::Text(text.into()),
                timestamp: 0.0,
            })],
            tools: None,
        }
    }
    /// Exercise scripted through the public invocation boundary.
    fn scripted(text: &str) -> FauxResponseStep {
        FauxResponseStep::Message(Box::new(faux_assistant_message(
            text,
            FauxAssistantMessageOptions::default(),
        )))
    }
    #[test]
    /// Exercise registration estimates usage through the public invocation boundary.
    fn maestro_registration_estimates_usage() {
        let registration = register_faux_provider(RegisterFauxProviderOptions::default());
        registration.set_responses(vec![scripted("answer")]);
        let message = wait(complete(
            registration.get_model(None).unwrap().clone(),
            context("hello"),
            None,
        ))
        .unwrap();
        let message = message.read().unwrap();
        assert_eq!(
            message.content,
            vec![AssistantContent::Text(faux_text("answer"))]
        );
        number(message.usage.input, 3.0);
        number(message.usage.output, 2.0);
        number(message.usage.total_tokens, 5.0);
        assert_eq!(registration.state.call_count(), 1);
        registration.unregister();
    }

    /// Exercise number through the public invocation boundary.
    fn number(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < f64::EPSILON,
            "{actual} != {expected}"
        );
    }
    /// Exercise model through the public invocation boundary.
    fn model(registration: &FauxProviderRegistration) -> Model {
        registration.get_model(None).unwrap().clone()
    }
    /// Exercise invoke through the public invocation boundary.
    fn invoke(
        registration: &FauxProviderRegistration,
        context: Context,
        options: Option<ProviderStreamOptions>,
    ) -> AssistantMessage {
        wait(complete(model(registration), context, options))
            .unwrap()
            .read()
            .unwrap()
            .clone()
    }
    /// Exercise events through the public invocation boundary.
    fn events(
        registration: &FauxProviderRegistration,
        options: Option<ProviderStreamOptions>,
    ) -> Vec<AssistantMessageEvent> {
        let stream = stream(model(registration), context("hello"), options).unwrap();
        wait(async {
            let mut events = Vec::new();
            while let Some(event) = stream.next().await {
                events.push(event);
            }
            events
        })
    }
    /// Exercise fixed through the public invocation boundary.
    fn fixed(speed: Option<f64>) -> FauxProviderRegistration {
        register_faux_provider(RegisterFauxProviderOptions {
            token_size: Some(FauxTokenSize {
                min: Some(1.0),
                max: Some(1.0),
            }),
            tokens_per_second: speed,
            ..RegisterFauxProviderOptions::default()
        })
    }
    /// Exercise session through the public invocation boundary.
    fn session(id: &str, retention: Option<CacheRetention>) -> ProviderStreamOptions {
        ProviderStreamOptions {
            common: StreamOptions {
                session_id: Some(id.into()),
                cache_retention: retention,
                ..StreamOptions::default()
            },
            ..ProviderStreamOptions::default()
        }
    }
    /// Exercise failure through the public invocation boundary.
    fn failure(text: &str) -> DiagnosticErrorInfo {
        DiagnosticErrorInfo {
            message: text.into(),
            name: None,
            stack: None,
            code: None,
        }
    }

    #[test]
    /// Exercise builders keep defaults and options through the public invocation boundary.
    fn maestro_builders_keep_defaults_and_options() {
        let options = FauxAssistantMessageOptions {
            stop_reason: Some(StopReason::Length),
            error_message: Some(String::new()),
            response_id: Some("id".into()),
            timestamp: Some(0.0),
        };
        let message = faux_assistant_message("", options);
        assert_eq!(message.content, vec![AssistantContent::Text(faux_text(""))]);
        assert_eq!(message.stop_reason, StopReason::Length);
        assert_eq!(message.error_message.as_deref(), Some(""));
        assert_eq!(message.response_id.as_deref(), Some("id"));
        number(message.timestamp, 0.0);
        let text = faux_text("text");
        let thinking = faux_thinking("thinking");
        let tool = faux_tool_call("tool", JsonObject::new(), FauxToolCallOptions::default());
        for content in [
            FauxAssistantContent::from(String::from("text")),
            FauxAssistantContent::from(text.clone()),
            FauxAssistantContent::from(AssistantContent::Text(text)),
            FauxAssistantContent::from(thinking),
            FauxAssistantContent::from(tool),
        ] {
            let message = faux_assistant_message(content, FauxAssistantMessageOptions::default());
            assert_eq!(message.content.len(), 1);
            assert_eq!(message.stop_reason, StopReason::Stop);
            assert!(message.timestamp > 0.0);
            number(message.usage.total_tokens, 0.0);
        }
        assert!(
            faux_assistant_message(Vec::new(), FauxAssistantMessageOptions::default())
                .content
                .is_empty()
        );
    }

    #[test]
    /// Exercise builder usage is owned through the public invocation boundary.
    fn maestro_builder_usage_is_owned() {
        let mut first = faux_assistant_message("one", FauxAssistantMessageOptions::default());
        let second = faux_assistant_message("two", FauxAssistantMessageOptions::default());
        first.usage.input = 9.0;
        first.usage.cost.total = 4.0;
        number(second.usage.input, 0.0);
        number(second.usage.cost.total, 0.0);
        number(second.usage.output, 0.0);
        number(second.usage.cache_read, 0.0);
        number(second.usage.cache_write, 0.0);
    }

    #[test]
    /// Exercise registration defaults and overrides through the public invocation boundary.
    fn maestro_registration_defaults_and_overrides() {
        for models in [None, Some(vec![])] {
            let registration = register_faux_provider(RegisterFauxProviderOptions {
                models,
                ..RegisterFauxProviderOptions::default()
            });
            let model = model(&registration);
            assert_eq!(model.id, "faux-1");
            assert_eq!(model.name, "Faux Model");
            assert_eq!(model.input, vec![ModelInput::Text, ModelInput::Image]);
            assert!(!model.reasoning);
            number(model.context_window, 128_000.0);
            number(model.max_tokens, 16_384.0);
            assert_eq!(model.cost, ModelCost::default());
            assert_eq!(model.base_url, "http://localhost:0");
            registration.unregister();
        }
        let registration = register_faux_provider(RegisterFauxProviderOptions {
            provider: Some("custom".into()),
            models: Some(vec![FauxModelDefinition {
                id: String::new(),
                name: Some(String::new()),
                reasoning: Some(false),
                input: Some(vec![]),
                cost: Some(ModelCost {
                    input: 1.0,
                    output: 2.0,
                    cache_read: 3.0,
                    cache_write: 4.0,
                }),
                context_window: Some(0.0),
                max_tokens: Some(0.0),
            }]),
            ..RegisterFauxProviderOptions::default()
        });
        let model = model(&registration);
        assert_eq!(model.id, "");
        assert_eq!(model.name, "");
        assert_eq!(model.provider, "custom");
        assert!(model.input.is_empty());
        number(model.cost.output, 2.0);
        number(model.context_window, 0.0);
        number(model.max_tokens, 0.0);
        registration.unregister();
    }

    #[test]
    /// Exercise identifier defaults are independent through the public invocation boundary.
    fn maestro_identifier_defaults_are_independent() {
        let first = register_faux_provider(RegisterFauxProviderOptions::default());
        let second = register_faux_provider(RegisterFauxProviderOptions::default());
        assert_ne!(first.api, second.api);
        let message = faux_assistant_message("clock", FauxAssistantMessageOptions::default());
        assert_eq!(
            message.timestamp.fract().classify(),
            std::num::FpCategory::Zero,
            "clock uses integral milliseconds"
        );
        let tool = faux_tool_call("t", JsonObject::new(), FauxToolCallOptions::default());
        for (id, prefix) in [(&first.api, "faux"), (&tool.id, "tool")] {
            let parts: Vec<_> = id.split(':').collect();
            assert_eq!(parts.len(), 3);
            assert_eq!(parts[0], prefix);
            assert!(parts[1].parse::<i64>().unwrap() > 0);
            assert!(!parts[2].is_empty());
        }
        let empty = register_faux_provider(RegisterFauxProviderOptions {
            api: Some(String::new()),
            provider: Some(String::new()),
            ..RegisterFauxProviderOptions::default()
        });
        assert_eq!(empty.api, "");
        assert_eq!(model(&empty).provider, "");
        assert_eq!(
            faux_tool_call(
                "t",
                JsonObject::new(),
                FauxToolCallOptions {
                    id: Some(String::new())
                }
            )
            .id,
            ""
        );
        first.unregister();
        second.unregister();
        empty.unregister();
    }

    #[test]
    /// Exercise responses keep invocation identity through the public invocation boundary.
    fn maestro_responses_keep_invocation_identity() {
        let registration = register_faux_provider(RegisterFauxProviderOptions {
            provider: Some("custom-provider".into()),
            models: Some(vec![FauxModelDefinition {
                id: "custom-model".into(),
                ..FauxModelDefinition::default()
            }]),
            ..RegisterFauxProviderOptions::default()
        });
        let mut authored = faux_assistant_message(
            "ok",
            FauxAssistantMessageOptions {
                response_id: Some("r".into()),
                timestamp: Some(42.0),
                ..FauxAssistantMessageOptions::default()
            },
        );
        authored.response_model = Some("actual".into());
        if let AssistantContent::Text(text) = &mut authored.content[0] {
            text.text_signature = Some("signature".into());
        }
        registration.set_responses(vec![FauxResponseStep::Message(Box::new(authored.clone()))]);
        let result = invoke(&registration, context("hi"), None);
        assert_eq!(result.api, registration.api);
        assert_eq!(result.provider, model(&registration).provider);
        assert_eq!(result.model, model(&registration).id);
        assert_eq!(authored.api, "faux");
        assert_eq!(result.response_model, authored.response_model);
        assert_eq!(result.response_id, authored.response_id);
        assert_eq!(result.content, authored.content);
        number(result.timestamp, 42.0);
        registration.unregister();
    }

    #[test]
    /// Exercise queue exhaustion counts requests through the public invocation boundary.
    fn maestro_queue_exhaustion_counts_requests() {
        let registration = register_faux_provider(RegisterFauxProviderOptions::default());
        registration.set_responses(vec![scripted("first"), scripted("second")]);
        for text in ["first", "second"] {
            assert_eq!(
                invoke(&registration, context("hi"), None).content,
                vec![AssistantContent::Text(faux_text(text))]
            );
        }
        let result = invoke(&registration, context("hi"), Some(session("exhaust", None)));
        assert_eq!(result.stop_reason, StopReason::Error);
        assert_eq!(
            result.error_message.as_deref(),
            Some("No more faux responses queued")
        );
        number(result.usage.cache_write, 2.0);
        number(result.usage.total_tokens, 2.0);
        assert_eq!(registration.state.call_count(), 3);
        assert_eq!(registration.get_pending_response_count(), 0);
        let result = invoke(&registration, context("hi"), Some(session("exhaust", None)));
        number(result.usage.cache_read, 2.0);
        registration.unregister();
    }

    #[test]
    /// Exercise queue replacement and append through the public invocation boundary.
    fn maestro_queue_replacement_and_append() {
        let registration = register_faux_provider(RegisterFauxProviderOptions::default());
        registration.set_responses(vec![scripted("discard")]);
        registration.set_responses(vec![scripted("first")]);
        registration.append_responses(vec![scripted("second")]);
        assert_eq!(registration.get_pending_response_count(), 2);
        for text in ["first", "second"] {
            assert_eq!(
                invoke(&registration, context("hi"), Some(session("q", None))).content,
                vec![AssistantContent::Text(faux_text(text))]
            );
        }
        registration.set_responses(vec![scripted("third")]);
        let message = invoke(&registration, context("hi"), Some(session("q", None)));
        number(message.usage.cache_read, 2.0);
        registration.append_responses(vec![scripted("unused")]);
        registration.set_responses(vec![]);
        assert_eq!(registration.get_pending_response_count(), 0);
        assert_eq!(registration.state.call_count(), 3);
        registration.unregister();
    }

    #[test]
    /// Exercise caches do not cross sessions through the public invocation boundary.
    fn maestro_caches_do_not_cross_sessions() {
        let first = register_faux_provider(RegisterFauxProviderOptions::default());
        let second = register_faux_provider(RegisterFauxProviderOptions::default());
        for options in [
            Some(session("a", None)),
            Some(session("b", None)),
            None,
            Some(session("", None)),
            None,
            Some(session("", None)),
        ] {
            first.append_responses(vec![scripted("ok")]);
            let result = invoke(&first, context("hi"), options);
            number(result.usage.cache_read, 0.0);
        }
        second.set_responses(vec![scripted("ok")]);
        number(
            invoke(&second, context("hi"), Some(session("a", None)))
                .usage
                .cache_read,
            0.0,
        );
        first.unregister();
        second.unregister();
    }

    #[test]
    /// Exercise session prefix cache reuses prompt through the public invocation boundary.
    fn maestro_session_prefix_cache_reuses_prompt() {
        let registration = register_faux_provider(RegisterFauxProviderOptions::default());
        for (text, read, write) in [
            ("abc", 0.0, 2.0),
            ("abc", 2.0, 0.0),
            ("abcdefgh", 2.0, 2.0),
            ("abcXYZ", 2.0, 1.0),
            ("a", 2.0, 0.0),
        ] {
            registration.set_responses(vec![scripted("ok")]);
            let usage = invoke(&registration, context(text), Some(session("prefix", None))).usage;
            number(usage.cache_read, read);
            number(usage.cache_write, write);
            number(usage.input, 0.0);
        }
        registration.unregister();
    }

    #[test]
    /// Exercise disabled cache leaves no state through the public invocation boundary.
    fn maestro_disabled_cache_leaves_no_state() {
        let registration = register_faux_provider(RegisterFauxProviderOptions::default());
        for (text, retention, read, write, input) in [
            ("abc", None, 0.0, 2.0, 0.0),
            ("DIFFERENT", Some(CacheRetention::None), 0.0, 0.0, 4.0),
            ("abc", Some(CacheRetention::Short), 2.0, 0.0, 0.0),
            ("abc", Some(CacheRetention::Long), 2.0, 0.0, 0.0),
        ] {
            registration.set_responses(vec![scripted("ok")]);
            let usage = invoke(
                &registration,
                context(text),
                Some(session("enabled", retention)),
            )
            .usage;
            number(usage.cache_read, read);
            number(usage.cache_write, write);
            number(usage.input, input);
        }
        registration.unregister();
    }

    #[test]
    /// Exercise cache totals count prompt once through the public invocation boundary.
    fn maestro_cache_totals_count_prompt_once() {
        let registration = register_faux_provider(RegisterFauxProviderOptions::default());
        for (text, prompt, read) in [
            ("abc", 2.0, 0.0),
            ("abc", 2.0, 2.0),
            ("abcd", 3.0, 2.0),
            ("abXY", 3.0, 2.0),
            ("a", 2.0, 2.0),
        ] {
            registration.set_responses(vec![scripted("12345")]);
            let usage = invoke(
                &registration,
                context(text),
                Some(session("rounding", None)),
            )
            .usage;
            number(usage.cache_read, read);
            number(usage.cache_write, prompt - read);
            number(usage.input, 0.0);
            number(usage.total_tokens, prompt + 2.0);
            number(usage.cost.total, 0.0);
        }
        registration.unregister();
    }

    /// Exercise event name through the public invocation boundary.
    fn event_name(event: &AssistantMessageEvent) -> String {
        serde_json::to_value(event).unwrap()["type"]
            .as_str()
            .unwrap()
            .into()
    }
    /// Exercise partial through the public invocation boundary.
    fn partial(event: &AssistantMessageEvent) -> SharedAssistantMessage {
        match event {
            AssistantMessageEvent::Start { partial }
            | AssistantMessageEvent::TextStart { partial, .. }
            | AssistantMessageEvent::TextDelta { partial, .. }
            | AssistantMessageEvent::TextEnd { partial, .. }
            | AssistantMessageEvent::ThinkingStart { partial, .. }
            | AssistantMessageEvent::ThinkingDelta { partial, .. }
            | AssistantMessageEvent::ThinkingEnd { partial, .. }
            | AssistantMessageEvent::ToolcallStart { partial, .. }
            | AssistantMessageEvent::ToolcallDelta { partial, .. }
            | AssistantMessageEvent::ToolcallEnd { partial, .. } => partial.clone(),
            _ => unreachable!(),
        }
    }
    /// Exercise mixed through the public invocation boundary.
    fn mixed() -> AssistantMessage {
        faux_assistant_message(
            vec![
                AssistantContent::Thinking(faux_thinking("think123")),
                AssistantContent::Text(faux_text("text1234")),
                AssistantContent::ToolCall(faux_tool_call(
                    "lookup",
                    serde_json::from_value(serde_json::json!({"key":"value"})).unwrap(),
                    FauxToolCallOptions {
                        id: Some("tool-id".into()),
                    },
                )),
            ],
            FauxAssistantMessageOptions {
                stop_reason: Some(StopReason::ToolUse),
                ..FauxAssistantMessageOptions::default()
            },
        )
    }
    /// Exercise collect cancel through the public invocation boundary.
    async fn collect_cancel(
        stream: &AssistantMessageEventStream,
        signal: &Cancellation,
        delta_name: &str,
    ) -> Vec<AssistantMessageEvent> {
        let mut observed = Vec::new();
        while let Some(event) = stream.next().await {
            if event_name(&event) == delta_name {
                signal.abort();
            }
            observed.push(event);
        }
        observed
    }
    /// Poll the consumer synchronously when the producer publishes an event.
    struct InlineObserver(std::sync::Mutex<Option<BoxFuture<()>>>);

    impl std::task::Wake for InlineObserver {
        fn wake(self: std::sync::Arc<Self>) {
            self.wake_by_ref();
        }

        fn wake_by_ref(self: &std::sync::Arc<Self>) {
            let waker = std::task::Waker::from(self.clone());
            let mut context = std::task::Context::from_waker(&waker);
            let mut future = self.0.lock().unwrap();
            if future
                .as_mut()
                .is_some_and(|future| future.as_mut().poll(&mut context).is_ready())
            {
                *future = None;
            }
        }
    }

    /// Suspend the actual response factory until the observer is ready.
    fn gated_response(
        message: AssistantMessage,
    ) -> (
        FauxResponseStep,
        std::sync::mpsc::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let release = std::sync::Mutex::new(Some(release_rx));
        let step = FauxResponseStep::Factory(std::sync::Arc::new(move |_, _, _, _| {
            let release = release.lock().unwrap().take().unwrap();
            let entered = entered_tx.clone();
            let message = message.clone();
            Box::pin(async move {
                entered.send(()).unwrap();
                release.await.unwrap();
                Ok(message)
            })
        }));
        (step, entered_rx, release_tx)
    }

    /// Cancel on the producer's publishing thread before it can schedule another chunk.
    fn observe_cancel(
        stream: AssistantMessageEventStream,
        signal: Cancellation,
        delta_name: String,
        release: tokio::sync::oneshot::Sender<()>,
    ) -> Vec<AssistantMessageEvent> {
        let (sent, received) = std::sync::mpsc::channel();
        let observer = std::sync::Arc::new(InlineObserver(std::sync::Mutex::new(Some(Box::pin(
            async move {
                sent.send(collect_cancel(&stream, &signal, &delta_name).await)
                    .unwrap();
            },
        )))));
        std::task::Wake::wake_by_ref(&observer);
        release.send(()).unwrap();
        received.recv().unwrap()
    }

    /// Exercise check cancel through the public invocation boundary.
    fn check_cancel(block: AssistantContent, delta_name: &str) {
        let registration = fixed(Some(100.0));
        let signal = Cancellation::new();
        let (step, entered, release) = gated_response(faux_assistant_message(
            block,
            FauxAssistantMessageOptions::default(),
        ));
        registration.set_responses(vec![step]);
        let options = Some(ProviderStreamOptions {
            common: StreamOptions {
                signal: Some(signal.clone()),
                ..StreamOptions::default()
            },
            ..ProviderStreamOptions::default()
        });
        let stream = stream(model(&registration), context("hi"), options).unwrap();
        entered.recv().unwrap();
        let observed = observe_cancel(stream.clone(), signal, delta_name.to_owned(), release);
        assert_eq!(
            observed
                .iter()
                .filter(|event| event_name(event) == delta_name)
                .count(),
            1
        );
        assert!(
            !observed
                .iter()
                .any(|event| event_name(event).ends_with("_end") || event_name(event) == "done")
        );
        let result = wait(stream.result());
        let message = result.read().unwrap();
        assert_eq!(message.stop_reason, StopReason::Aborted);
        assert_eq!(
            message.timestamp.fract().classify(),
            std::num::FpCategory::Zero
        );
        assert_eq!(
            message.error_message.as_deref(),
            Some("Request was aborted")
        );
        match &message.content[0] {
            AssistantContent::Text(value) => assert_eq!(value.text, "abcd"),
            AssistantContent::Thinking(value) => assert_eq!(value.thinking, "abcd"),
            AssistantContent::ToolCall(value) => assert!(value.arguments.is_empty()),
        }
        registration.unregister();
    }

    #[test]
    /// Exercise mixed content streams argument chunks through the public invocation boundary.
    fn maestro_mixed_content_streams_argument_chunks() {
        let registration = fixed(None);
        let authored = mixed();
        registration.set_responses(vec![FauxResponseStep::Message(Box::new(authored.clone()))]);
        let observed = events(&registration, None);
        assert!(
            observed
                .iter()
                .any(|event| matches!(event, AssistantMessageEvent::ThinkingDelta { .. }))
        );
        assert!(
            observed
                .iter()
                .any(|event| matches!(event, AssistantMessageEvent::TextDelta { .. }))
        );
        let arguments: String = observed
            .iter()
            .filter_map(|event| match event {
                AssistantMessageEvent::ToolcallDelta { delta, .. } => Some(delta.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(arguments, r#"{"key":"value"}"#);
        assert!(
            matches!(observed.last(), Some(AssistantMessageEvent::Done {reason: DoneReason::ToolUse, message}) if message.read().unwrap().content == authored.content)
        );
        registration.unregister();
    }

    #[test]
    /// Exercise fixed chunks have ordered events through the public invocation boundary.
    fn maestro_fixed_chunks_have_ordered_events() {
        let registration = fixed(None);
        for reason in [StopReason::Stop, StopReason::Length, StopReason::ToolUse] {
            registration.set_responses(vec![FauxResponseStep::Message(Box::new(
                faux_assistant_message(
                    "abcdefghij",
                    FauxAssistantMessageOptions {
                        stop_reason: Some(reason.clone()),
                        ..FauxAssistantMessageOptions::default()
                    },
                ),
            ))]);
            let observed = events(&registration, None);
            assert_eq!(
                observed.iter().map(event_name).collect::<Vec<_>>(),
                [
                    "start",
                    "text_start",
                    "text_delta",
                    "text_delta",
                    "text_delta",
                    "text_end",
                    "done"
                ]
            );
            let chunks: Vec<_> = observed
                .iter()
                .filter_map(|event| match event {
                    AssistantMessageEvent::TextDelta { delta, .. } => Some(delta.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(chunks, ["abcd", "efgh", "ij"]);
            assert!(
                matches!(observed.last(), Some(AssistantMessageEvent::Done {message,..}) if message.read().unwrap().stop_reason == reason)
            );
        }
        registration.unregister();
    }

    #[test]
    /// Exercise tool blocks keep distinct indices through the public invocation boundary.
    fn maestro_tool_blocks_keep_distinct_indices() {
        let registration = fixed(None);
        let blocks: Vec<_> = ["one", "two"]
            .into_iter()
            .map(|id| {
                AssistantContent::ToolCall(faux_tool_call(
                    id,
                    JsonObject::new(),
                    FauxToolCallOptions {
                        id: Some(id.into()),
                    },
                ))
            })
            .collect();
        registration.set_responses(vec![FauxResponseStep::Message(Box::new(
            faux_assistant_message(blocks, FauxAssistantMessageOptions::default()),
        ))]);
        let observed = events(&registration, None);
        let ends: Vec<_> = observed
            .iter()
            .filter_map(|event| match event {
                AssistantMessageEvent::ToolcallEnd {
                    content_index,
                    tool_call,
                    ..
                } => Some((*content_index, tool_call.id.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(ends, [(0, "one"), (1, "two")]);
        let starts: Vec<_> = observed
            .iter()
            .filter_map(|event| match event {
                AssistantMessageEvent::ToolcallStart { content_index, .. } => Some(*content_index),
                _ => None,
            })
            .collect();
        assert_eq!(starts, [0, 1]);
        registration.unregister();
    }

    #[test]
    /// Exercise explicit error retains message through the public invocation boundary.
    fn maestro_explicit_error_retains_message() {
        let registration = fixed(None);
        let authored = faux_assistant_message(
            "fail",
            FauxAssistantMessageOptions {
                stop_reason: Some(StopReason::Error),
                error_message: Some("upstream failed".into()),
                ..FauxAssistantMessageOptions::default()
            },
        );
        registration.set_responses(vec![FauxResponseStep::Message(Box::new(authored))]);
        let stream = stream(model(&registration), context("hi"), None).unwrap();
        let result = wait(stream.result());
        let observed = wait(async {
            let mut observed = Vec::new();
            while let Some(event) = stream.next().await {
                observed.push(event);
            }
            observed
        });
        assert_eq!(
            observed.iter().map(event_name).collect::<Vec<_>>(),
            ["start", "text_start", "text_delta", "text_end", "error"]
        );
        assert!(
            matches!(observed.last(), Some(AssistantMessageEvent::Error {reason: ErrorReason::Error,error}) if std::sync::Arc::ptr_eq(error, &result))
        );
        assert_eq!(
            result.read().unwrap().error_message.as_deref(),
            Some("upstream failed")
        );
        registration.unregister();
    }

    #[test]
    /// Exercise explicit abort retains message through the public invocation boundary.
    fn maestro_explicit_abort_retains_message() {
        let registration = fixed(None);
        registration.set_responses(vec![FauxResponseStep::Message(Box::new(
            faux_assistant_message(
                "stop",
                FauxAssistantMessageOptions {
                    stop_reason: Some(StopReason::Aborted),
                    error_message: Some("Request was aborted".into()),
                    timestamp: Some(123.0),
                    ..FauxAssistantMessageOptions::default()
                },
            ),
        ))]);
        let observed = events(&registration, None);
        assert_eq!(
            observed.iter().map(event_name).collect::<Vec<_>>(),
            ["start", "text_start", "text_delta", "text_end", "error"]
        );
        match observed.last().unwrap() {
            AssistantMessageEvent::Error {
                reason: ErrorReason::Aborted,
                error,
            } => {
                let message = error.read().unwrap();
                number(message.timestamp, 123.0);
                assert_eq!(
                    message.error_message.as_deref(),
                    Some("Request was aborted")
                );
                assert_eq!(
                    message.content,
                    vec![AssistantContent::Text(faux_text("stop"))]
                );
            }
            _ => unreachable!(),
        }
        registration.unregister();
    }

    #[test]
    /// Exercise paced text cancel keeps prefix through the public invocation boundary.
    fn maestro_paced_text_cancel_keeps_prefix() {
        check_cancel(
            AssistantContent::Text(faux_text("abcdefghijkl")),
            "text_delta",
        );
    }

    #[test]
    /// Exercise paced thinking cancel keeps prefix through the public invocation boundary.
    fn maestro_paced_thinking_cancel_keeps_prefix() {
        check_cancel(
            AssistantContent::Thinking(faux_thinking("abcdefghijkl")),
            "thinking_delta",
        );
    }

    #[test]
    /// Exercise paced tool cancel keeps empty arguments through the public invocation boundary.
    fn maestro_paced_tool_cancel_keeps_empty_arguments() {
        check_cancel(
            AssistantContent::ToolCall(faux_tool_call(
                "tool",
                serde_json::from_value(serde_json::json!({"argument":"long value"})).unwrap(),
                FauxToolCallOptions::default(),
            )),
            "toolcall_delta",
        );
    }

    #[test]
    /// Exercise partials retain block membership through the public invocation boundary.
    fn maestro_partials_retain_block_membership() {
        let registration = fixed(None);
        registration.set_responses(vec![FauxResponseStep::Message(Box::new(mixed()))]);
        let observed = events(&registration, None);
        assert!(partial(&observed[0]).read().unwrap().content.is_empty());
        let thinking: Vec<_> = observed
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    AssistantMessageEvent::ThinkingStart { .. }
                        | AssistantMessageEvent::ThinkingDelta { .. }
                        | AssistantMessageEvent::ThinkingEnd { .. }
                )
            })
            .map(partial)
            .collect();
        for handle in &thinking {
            assert!(std::sync::Arc::ptr_eq(handle, &thinking[0]));
            assert_eq!(
                handle.read().unwrap().content,
                vec![AssistantContent::Thinking(faux_thinking("think123"))]
            );
        }
        let text = observed
            .iter()
            .find(|event| matches!(event, AssistantMessageEvent::TextEnd { .. }))
            .map(partial)
            .unwrap();
        assert_eq!(text.read().unwrap().content.len(), 2);
        let tool = observed
            .iter()
            .find(|event| matches!(event, AssistantMessageEvent::ToolcallEnd { .. }))
            .map(partial)
            .unwrap();
        assert_eq!(tool.read().unwrap().content.len(), 3);
        assert!(
            matches!(&tool.read().unwrap().content[2],AssistantContent::ToolCall(value) if value.arguments["key"]=="value")
        );
        registration.unregister();
    }

    #[test]
    /// Exercise empty blocks still finish through the public invocation boundary.
    fn maestro_empty_blocks_still_finish() {
        let registration = fixed(None);
        registration.set_responses(vec![FauxResponseStep::Message(Box::new(
            faux_assistant_message(Vec::new(), FauxAssistantMessageOptions::default()),
        ))]);
        assert_eq!(
            events(&registration, None)
                .iter()
                .map(event_name)
                .collect::<Vec<_>>(),
            ["start", "done"]
        );
        registration.set_responses(vec![FauxResponseStep::Message(Box::new(
            faux_assistant_message(
                vec![
                    AssistantContent::Text(faux_text("")),
                    AssistantContent::Thinking(faux_thinking("")),
                    AssistantContent::ToolCall(faux_tool_call(
                        "t",
                        JsonObject::new(),
                        FauxToolCallOptions::default(),
                    )),
                ],
                FauxAssistantMessageOptions::default(),
            ),
        ))]);
        let observed = events(&registration, None);
        let deltas: Vec<_> = observed
            .iter()
            .filter_map(|event| match event {
                AssistantMessageEvent::TextDelta { delta, .. }
                | AssistantMessageEvent::ThinkingDelta { delta, .. }
                | AssistantMessageEvent::ToolcallDelta { delta, .. } => Some(delta.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(deltas, ["", "", "{}"]);
        assert_eq!(
            observed
                .iter()
                .filter(|event| event_name(event).ends_with("_end"))
                .count(),
            3
        );
        registration.unregister();
    }

    #[test]
    /// Exercise scalar boundaries keep text valid through the public invocation boundary.
    fn maestro_scalar_boundaries_keep_text_valid() {
        let registration = fixed(None);
        let text = "abc😀e\u{301}\u{feff}\u{85}";
        registration.set_responses(vec![scripted(text)]);
        let observed = events(&registration, None);
        let chunks: Vec<_> = observed
            .iter()
            .filter_map(|event| match event {
                AssistantMessageEvent::TextDelta { delta, .. } => Some(delta.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(chunks, ["abc😀", "e\u{301}\u{feff}\u{85}"]);
        for (text, read, write) in [("abc😀", 0.0, 3.0), ("abc😁", 2.0, 1.0)] {
            registration.set_responses(vec![scripted("😀😀😀😀😀")]);
            let usage = invoke(&registration, context(text), Some(session("unicode", None))).usage;
            number(usage.cache_read, read);
            number(usage.cache_write, write);
            number(usage.output, 2.0);
            number(usage.total_tokens, 5.0);
        }
        registration.unregister();
    }

    #[test]
    /// Exercise chunk bounds preserve unpaced delivery through the public invocation boundary.
    fn maestro_chunk_bounds_preserve_unpaced_delivery() {
        let text = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz";
        for (min, max, speed, lower, upper) in [
            (None, None, None, 12, 20),
            (Some(5.0), Some(2.0), Some(0.0), 8, 8),
            (Some(0.0), Some(-1.0), Some(-1.0), 4, 4),
        ] {
            let registration = register_faux_provider(RegisterFauxProviderOptions {
                token_size: Some(FauxTokenSize { min, max }),
                tokens_per_second: speed,
                ..RegisterFauxProviderOptions::default()
            });
            registration.set_responses(vec![scripted(text)]);
            let observed = events(&registration, None);
            let chunks: Vec<_> = observed
                .iter()
                .filter_map(|event| match event {
                    AssistantMessageEvent::TextDelta { delta, .. } => Some(delta.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(chunks.concat(), text);
            for chunk in &chunks[..chunks.len() - 1] {
                assert!((lower..=upper).contains(&chunk.chars().count()));
            }
            registration.unregister();
        }
    }

    #[test]
    /// Exercise factories observe selected model through the public invocation boundary.
    fn maestro_factories_observe_selected_model() {
        let registration = register_faux_provider(RegisterFauxProviderOptions {
            models: Some(vec![
                FauxModelDefinition {
                    id: "fast".into(),
                    ..FauxModelDefinition::default()
                },
                FauxModelDefinition {
                    id: "deep".into(),
                    reasoning: Some(true),
                    ..FauxModelDefinition::default()
                },
            ]),
            ..RegisterFauxProviderOptions::default()
        });
        assert_eq!(registration.get_model(None).unwrap().id, "fast");
        assert_eq!(registration.get_model(Some("")).unwrap().id, "fast");
        assert!(registration.get_model(Some("missing")).is_none());
        for id in ["fast", "deep"] {
            registration.append_responses(vec![FauxResponseStep::Factory(std::sync::Arc::new(
                |_, _, _, model| {
                    Box::pin(std::future::ready(Ok(faux_assistant_message(
                        format!("{}:{}", model.id, model.reasoning),
                        FauxAssistantMessageOptions::default(),
                    ))))
                },
            ))]);
            let result = wait(complete(
                registration.get_model(Some(id)).unwrap().clone(),
                context("hi"),
                None,
            ))
            .unwrap();
            assert_eq!(
                result.read().unwrap().content,
                vec![AssistantContent::Text(faux_text(if id == "fast" {
                    "fast:false"
                } else {
                    "deep:true"
                }))]
            );
        }
        registration.unregister();
    }

    #[test]
    /// Exercise async factories observe live state through the public invocation boundary.
    fn maestro_async_factories_observe_live_state() {
        let registration = fixed(None);
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let release = std::sync::Arc::new(std::sync::Mutex::new(Some(release_rx)));
        registration.set_responses(vec![
            FauxResponseStep::Factory(std::sync::Arc::new(move |context, options, state, _| {
                let release = release.lock().unwrap().take().unwrap();
                let entered = entered_tx.clone();
                Box::pin(async move {
                    assert_eq!(context.messages.len(), 1);
                    assert!(options.is_none());
                    entered.send(()).unwrap();
                    release.await.unwrap();
                    Ok(faux_assistant_message(
                        state.call_count().to_string(),
                        FauxAssistantMessageOptions::default(),
                    ))
                })
            })),
            scripted("second"),
        ]);
        let first = complete(model(&registration), context("hi"), None);
        entered_rx.recv().unwrap();
        let second = complete(model(&registration), context("hi"), None);
        assert_eq!(registration.state.call_count(), 2);
        release_tx.send(()).unwrap();
        assert_eq!(
            wait(first).unwrap().read().unwrap().content,
            vec![AssistantContent::Text(faux_text("2"))]
        );
        assert_eq!(
            wait(second).unwrap().read().unwrap().content,
            vec![AssistantContent::Text(faux_text("second"))]
        );
        registration.unregister();
    }

    #[test]
    /// Exercise factory failure is terminal through the public invocation boundary.
    fn maestro_factory_failure_is_terminal() {
        let registration = fixed(None);
        registration.set_responses(vec![FauxResponseStep::Factory(std::sync::Arc::new(
            |_, _, _, _| Box::pin(async { Err(failure("boom")) }),
        ))]);
        let stream = stream(model(&registration), context("hi"), None).unwrap();
        let observed = wait(async {
            let mut events = Vec::new();
            while let Some(event) = stream.next().await {
                events.push(event);
            }
            events
        });
        assert_eq!(observed.len(), 1);
        let result = wait(stream.result());
        let message = result.read().unwrap();
        assert_eq!(message.stop_reason, StopReason::Error);
        assert_eq!(message.error_message.as_deref(), Some("boom"));
        assert!(message.content.is_empty());
        number(message.usage.total_tokens, 0.0);
        registration.unregister();
    }

    #[test]
    /// Exercise preaborted request consumes response through the public invocation boundary.
    fn maestro_preaborted_request_consumes_response() {
        let registration = fixed(None);
        let signal = Cancellation::new();
        signal.abort();
        let called = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let factory_called = called.clone();
        let hook_called = called.clone();
        registration.set_responses(vec![FauxResponseStep::Factory(std::sync::Arc::new(
            move |_, _, _, _| {
                factory_called.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Box::pin(async {
                    Ok(faux_assistant_message(
                        "abcdef",
                        FauxAssistantMessageOptions::default(),
                    ))
                })
            },
        ))]);
        let options = Some(ProviderStreamOptions {
            common: StreamOptions {
                signal: Some(signal),
                session_id: Some("abort".into()),
                on_response: Some(std::sync::Arc::new(move |_, _| {
                    hook_called.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Box::pin(async { Ok(()) })
                })),
                ..StreamOptions::default()
            },
            ..ProviderStreamOptions::default()
        });
        let observed = events(&registration, options);
        assert_eq!(observed.len(), 1);
        assert_eq!(called.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(registration.state.call_count(), 1);
        assert_eq!(registration.get_pending_response_count(), 0);
        match &observed[0] {
            AssistantMessageEvent::Error {
                error,
                reason: ErrorReason::Aborted,
            } => {
                let message = error.read().unwrap();
                assert!(message.content.is_empty());
                number(message.usage.output, 2.0);
                number(message.usage.cache_write, 3.0);
            }
            _ => unreachable!(),
        }
        registration.append_responses(vec![scripted("ok")]);
        number(
            invoke(
                &registration,
                context("hello"),
                Some(session("abort", None)),
            )
            .usage
            .cache_read,
            3.0,
        );
        registration.unregister();
    }

    #[test]
    /// Exercise unregister only removes owned source through the public invocation boundary.
    fn maestro_unregister_only_removes_owned_source() {
        let first = fixed(None);
        let api = first.api.clone();
        let selected = model(&first);
        first.set_responses(vec![scripted("pending")]);
        let pending = complete(selected.clone(), context("hi"), None);
        first.unregister();
        first.unregister();
        assert_eq!(
            stream(selected.clone(), context("hi"), None)
                .err()
                .unwrap()
                .message,
            format!("No API provider registered for api: {api}")
        );
        let second = register_faux_provider(RegisterFauxProviderOptions {
            api: Some(api),
            ..RegisterFauxProviderOptions::default()
        });
        second.set_responses(vec![scripted("replacement")]);
        first.unregister();
        assert_eq!(
            invoke(&second, context("hi"), None).content,
            vec![AssistantContent::Text(faux_text("replacement"))]
        );
        assert_eq!(
            wait(pending).unwrap().read().unwrap().content,
            vec![AssistantContent::Text(faux_text("pending"))]
        );
        second.unregister();
    }

    /// Signal when the producer releases its response hook after publication.
    struct ProducerFinished(std::sync::mpsc::Sender<()>);

    impl Drop for ProducerFinished {
        fn drop(&mut self) {
            self.0.send(()).unwrap();
        }
    }

    /// Dropped observers leave production and the detached session cache intact.
    #[test]
    fn maestro_production_survives_dropped_observers() {
        let registration = fixed(None);
        let (step, entered, release) = gated_response(faux_assistant_message(
            "done",
            FauxAssistantMessageOptions::default(),
        ));
        registration.set_responses(vec![step]);
        let (finished_tx, finished_rx) = std::sync::mpsc::channel();
        let witness = ProducerFinished(finished_tx);
        let mut options = session("detached", None);
        options.common.on_response = Some(std::sync::Arc::new(move |_, _| {
            let _ = &witness;
            Box::pin(async { Ok(()) })
        }));
        let stream = stream(model(&registration), context("hi"), Some(options)).unwrap();
        entered.recv().unwrap();
        drop(stream.result());
        drop(stream);
        release.send(()).unwrap();
        finished_rx.recv().unwrap();
        registration.append_responses(vec![scripted("cached")]);
        let cached = invoke(
            &registration,
            context("hi"),
            Some(session("detached", None)),
        );
        number(cached.usage.cache_read, 2.0);
        number(cached.usage.cache_write, 0.0);
        registration.unregister();
    }

    /// Exercise context with all roles through the public invocation boundary.
    fn context_with_all_roles() -> Context {
        let image = |mime: &str, data: &str| {
            UserBlock::Image(ImageContent {
                mime_type: mime.into(),
                data: data.into(),
            })
        };
        let assistant = faux_assistant_message(
            vec![
                AssistantContent::Thinking(faux_thinking("thought")),
                AssistantContent::Text(faux_text("reply")),
                AssistantContent::ToolCall(faux_tool_call(
                    "lookup",
                    serde_json::from_value(serde_json::json!({"key":"value"})).unwrap(),
                    FauxToolCallOptions::default(),
                )),
            ],
            FauxAssistantMessageOptions::default(),
        );
        Context {
            system_prompt: Some("rules".into()),
            messages: vec![
                Message::User(UserMessage {
                    content: UserContent::Text("plain".into()),
                    timestamp: 99.0,
                }),
                Message::User(UserMessage {
                    content: UserContent::Blocks(vec![
                        UserBlock::Text(faux_text("alpha")),
                        image("image/png", "abc"),
                    ]),
                    timestamp: 0.0,
                }),
                Message::Assistant(assistant),
                Message::ToolResult(ToolResultMessage {
                    tool_call_id: "ignored".into(),
                    tool_name: "lookup".into(),
                    content: vec![
                        UserBlock::Text(faux_text("result")),
                        image("image/jpeg", "xy"),
                    ],
                    details: Some(serde_json::json!({"ignored":true})),
                    is_error: false,
                    timestamp: 0.0,
                }),
            ],
            tools: Some(vec![Tool {
                name: "lookup".into(),
                description: "Find".into(),
                parameters: serde_json::json!({"type":"object"}),
            }]),
        }
    }

    #[test]
    /// Exercise serialized context drives usage through the public invocation boundary.
    fn maestro_serialized_context_drives_usage() {
        let registration = fixed(None);
        let context = context_with_all_roles();
        registration.set_responses(vec![FauxResponseStep::Message(Box::new(mixed()))]);
        let usage = invoke(&registration, context, None).usage;
        number(usage.input, 58.0);
        number(usage.output, 10.0);
        number(usage.total_tokens, 68.0);
        assert_eq!(
            usage.cost,
            UsageCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total: 0.0
            }
        );
        registration.set_responses(vec![scripted("")]);
        let empty = Context {
            system_prompt: Some(String::new()),
            messages: vec![],
            tools: Some(vec![]),
        };
        number(invoke(&registration, empty, None).usage.total_tokens, 0.0);
        registration.unregister();
    }

    #[test]
    /// Exercise serialization retains native values through the public invocation boundary.
    fn maestro_serialization_retains_native_values() {
        let registration = fixed(None);
        let mut arguments = JsonObject::new();
        arguments.insert("10".into(), serde_json::json!(-0.0));
        arguments.insert("2".into(), serde_json::json!(1e20));
        arguments.insert(
            "a".into(),
            serde_json::json!([true,null,"line\n\"😀",{"nested":false}]),
        );
        let expected = r#"{"10":-0.0,"2":1e+20,"a":[true,null,"line\n\"😀",{"nested":false}]}"#;
        registration.set_responses(vec![FauxResponseStep::Message(Box::new(
            faux_assistant_message(
                faux_tool_call("t", arguments.clone(), FauxToolCallOptions::default()),
                FauxAssistantMessageOptions::default(),
            ),
        ))]);
        let observed = events(&registration, None);
        let serialized: String = observed
            .iter()
            .filter_map(|event| match event {
                AssistantMessageEvent::ToolcallDelta { delta, .. } => Some(delta.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(serialized, expected);
        assert!(
            matches!(observed.iter().find(|event|matches!(event,AssistantMessageEvent::ToolcallEnd{..})),Some(AssistantMessageEvent::ToolcallEnd{tool_call,..}) if tool_call.arguments==arguments)
        );
        registration.unregister();
    }

    /// Exercise check forwarded options through the public invocation boundary.
    fn check_forwarded_options(options: Option<FauxResponseOptions>) -> AssistantMessage {
        let text = match options {
            None => "absent",
            Some(FauxResponseOptions::Raw(value)) if value.extra.is_empty() => "defaults",
            Some(FauxResponseOptions::Raw(value)) => {
                assert_eq!(value.extra["custom"], "retained");
                assert_eq!(value.common.session_id.as_deref(), Some("raw"));
                assert!(value.common.signal.is_some());
                assert!(value.common.on_response.is_some());
                "raw"
            }
            Some(FauxResponseOptions::Simple(value)) => {
                assert_eq!(value.reasoning, Some(ThinkingLevel::High));
                number(value.thinking_budgets.unwrap().high.unwrap(), 123.0);
                assert_eq!(value.common.session_id.as_deref(), Some("simple"));
                "simple"
            }
        };
        faux_assistant_message(text, FauxAssistantMessageOptions::default())
    }

    #[test]
    /// Exercise raw and simple options reach factory through the public invocation boundary.
    fn maestro_raw_and_simple_options_reach_factory() {
        let registration = fixed(None);
        let signal = Cancellation::new();
        let factory: FauxResponseFactory = std::sync::Arc::new(|_, options, _, _| {
            Box::pin(std::future::ready(Ok(check_forwarded_options(options))))
        });
        registration.set_responses(vec![
            FauxResponseStep::Factory(factory.clone()),
            FauxResponseStep::Factory(factory.clone()),
            FauxResponseStep::Factory(factory.clone()),
            FauxResponseStep::Factory(factory),
        ]);
        assert_eq!(
            invoke(&registration, context("hi"), None).content,
            vec![AssistantContent::Text(faux_text("absent"))]
        );
        let raw = ProviderStreamOptions {
            extra: serde_json::from_value(serde_json::json!({"custom":"retained"})).unwrap(),
            common: StreamOptions {
                signal: Some(signal),
                session_id: Some("raw".into()),
                on_response: Some(std::sync::Arc::new(|_, _| Box::pin(async { Ok(()) }))),
                ..StreamOptions::default()
            },
        };
        assert_eq!(
            invoke(&registration, context("hi"), Some(raw)).content,
            vec![AssistantContent::Text(faux_text("raw"))]
        );
        let simple = SimpleStreamOptions {
            common: StreamOptions {
                session_id: Some("simple".into()),
                ..StreamOptions::default()
            },
            reasoning: Some(ThinkingLevel::High),
            thinking_budgets: Some(ThinkingBudgets {
                high: Some(123.0),
                ..ThinkingBudgets::default()
            }),
        };
        let result = wait(complete_simple(
            model(&registration),
            context("hi"),
            Some(simple),
        ))
        .unwrap();
        assert_eq!(
            result.read().unwrap().content,
            vec![AssistantContent::Text(faux_text("simple"))]
        );
        assert_eq!(
            invoke(
                &registration,
                context("hi"),
                Some(ProviderStreamOptions::default())
            )
            .content,
            vec![AssistantContent::Text(faux_text("defaults"))]
        );
        registration.unregister();
    }

    /// Exercise check exhausted hook through the public invocation boundary.
    fn check_exhausted_hook(registration: &FauxProviderRegistration) {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let hook_calls = calls.clone();
        let options = Some(ProviderStreamOptions {
            common: StreamOptions {
                on_response: Some(std::sync::Arc::new(move |response, _| {
                    number(response.status, 200.0);
                    assert!(response.headers.is_empty());
                    hook_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Box::pin(async { Ok(()) })
                })),
                ..StreamOptions::default()
            },
            ..ProviderStreamOptions::default()
        });
        assert_eq!(
            invoke(registration, context("hi"), options)
                .error_message
                .as_deref(),
            Some("No more faux responses queued")
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
    /// Exercise check rejected hook through the public invocation boundary.
    fn check_rejected_hook(registration: &FauxProviderRegistration) {
        registration.set_responses(vec![scripted("discard")]);
        let options = Some(ProviderStreamOptions {
            common: StreamOptions {
                on_response: Some(std::sync::Arc::new(|_, _| {
                    Box::pin(async { Err(failure("hook failed")) })
                })),
                ..StreamOptions::default()
            },
            ..ProviderStreamOptions::default()
        });
        let observed = events(registration, options);
        assert_eq!(observed.len(), 1);
        assert_eq!(registration.get_pending_response_count(), 0);
        match &observed[0] {
            AssistantMessageEvent::Error { error, .. } => {
                let error = error.read().unwrap();
                assert_eq!(error.error_message.as_deref(), Some("hook failed"));
                number(error.usage.total_tokens, 0.0);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    /// Exercise response hook precedes factory through the public invocation boundary.
    fn maestro_response_hook_precedes_factory() {
        let registration = fixed(None);
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let factory_order = order.clone();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let release = std::sync::Arc::new(std::sync::Mutex::new(Some(release_rx)));
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let hook_order = order.clone();
        registration.set_responses(vec![FauxResponseStep::Factory(std::sync::Arc::new(
            move |_, _, _, _| {
                factory_order.lock().unwrap().push("factory");
                Box::pin(async {
                    Ok(faux_assistant_message(
                        "ok",
                        FauxAssistantMessageOptions::default(),
                    ))
                })
            },
        ))]);
        let options = Some(ProviderStreamOptions {
            common: StreamOptions {
                on_response: Some(std::sync::Arc::new(move |response, model| {
                    number(response.status, 200.0);
                    assert!(response.headers.is_empty());
                    assert_eq!(model.id, "faux-1");
                    let release = release.lock().unwrap().take().unwrap();
                    let order = hook_order.clone();
                    let entered = entered_tx.clone();
                    Box::pin(async move {
                        order.lock().unwrap().push("hook start");
                        entered.send(()).unwrap();
                        release.await.unwrap();
                        order.lock().unwrap().push("hook end");
                        Ok(())
                    })
                })),
                on_payload: Some(std::sync::Arc::new(|_, _| {
                    unreachable!("payload hook must not run")
                })),
                ..StreamOptions::default()
            },
            ..ProviderStreamOptions::default()
        });
        let result = complete(model(&registration), context("hi"), options);
        entered_rx.recv().unwrap();
        assert_eq!(*order.lock().unwrap(), ["hook start"]);
        release_tx.send(()).unwrap();
        drop(wait(result));
        assert_eq!(
            *order.lock().unwrap(),
            ["hook start", "hook end", "factory"]
        );
        check_exhausted_hook(&registration);
        check_rejected_hook(&registration);
        registration.unregister();
    }

    /// Exercise timed message through the public invocation boundary.
    async fn timed_message() -> Result<AssistantMessage, DiagnosticErrorInfo> {
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        Ok(faux_assistant_message(
            "abcdefgh",
            FauxAssistantMessageOptions::default(),
        ))
    }
    /// Exercise timed hook through the public invocation boundary.
    async fn timed_hook() -> Result<(), DiagnosticErrorInfo> {
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        Ok(())
    }
    /// Exercise complete in caller through the public invocation boundary.
    async fn complete_in_caller(
        registration: &FauxProviderRegistration,
        options: Option<ProviderStreamOptions>,
    ) -> AssistantMessage {
        complete(model(registration), context("hi"), options)
            .await
            .unwrap()
            .read()
            .unwrap()
            .clone()
    }
    #[test]
    /// Exercise pacing needs no caller timer through the public invocation boundary.
    fn maestro_pacing_needs_no_caller_timer() {
        for caller_runtime in [false, true] {
            let registration = fixed(Some(50.0));
            registration.set_responses(vec![FauxResponseStep::Factory(std::sync::Arc::new(
                |_, _, _, _| Box::pin(timed_message()),
            ))]);
            let options = Some(ProviderStreamOptions {
                common: StreamOptions {
                    on_response: Some(std::sync::Arc::new(|_, _| Box::pin(timed_hook()))),
                    ..StreamOptions::default()
                },
                ..ProviderStreamOptions::default()
            });
            let start = std::time::Instant::now();
            let result = if caller_runtime {
                wait(complete_in_caller(&registration, options))
            } else {
                invoke(&registration, context("hi"), options)
            };
            assert_eq!(
                result.content,
                vec![AssistantContent::Text(faux_text("abcdefgh"))]
            );
            assert!(start.elapsed() >= std::time::Duration::from_millis(40));
            registration.unregister();
        }
    }

    /// Exercise documented context through the public invocation boundary.
    fn documented_context() -> Context {
        Context {
            system_prompt: None,
            messages: vec![Message::User(UserMessage {
                content: UserContent::Text("Summarize package.json and then call echo".into()),
                timestamp: 0.0,
            })],
            tools: None,
        }
    }
    /// Exercise documented tool response through the public invocation boundary.
    fn documented_tool_response() -> Vec<FauxResponseStep> {
        let arguments =
            serde_json::from_value(serde_json::json!({"text": "package.json"})).unwrap();
        vec![FauxResponseStep::Message(Box::new(faux_assistant_message(
            vec![
                AssistantContent::Thinking(faux_thinking(
                    "Need to inspect package metadata first.",
                )),
                AssistantContent::ToolCall(faux_tool_call(
                    "echo",
                    arguments,
                    FauxToolCallOptions::default(),
                )),
            ],
            FauxAssistantMessageOptions {
                stop_reason: Some(StopReason::ToolUse),
                ..FauxAssistantMessageOptions::default()
            },
        )))]
    }
    /// Exercise documented summary response through the public invocation boundary.
    fn documented_summary_response() -> Vec<FauxResponseStep> {
        vec![FauxResponseStep::Message(Box::new(faux_assistant_message(
            vec![
                AssistantContent::Thinking(faux_thinking("Now I can summarize the tool output.")),
                AssistantContent::Text(faux_text("Here is the summary.")),
            ],
            FauxAssistantMessageOptions::default(),
        )))]
    }
    /// Exercise documented multi model through the public invocation boundary.
    fn documented_multi_model() -> FauxProviderRegistration {
        register_faux_provider(RegisterFauxProviderOptions {
            models: Some(vec![
                FauxModelDefinition {
                    id: "faux-fast".into(),
                    reasoning: Some(false),
                    ..FauxModelDefinition::default()
                },
                FauxModelDefinition {
                    id: "faux-thinker".into(),
                    reasoning: Some(true),
                    ..FauxModelDefinition::default()
                },
            ]),
            ..RegisterFauxProviderOptions::default()
        })
    }
    /// Exercise append documented result through the public invocation boundary.
    fn append_documented_result(
        context: &mut Context,
        first: AssistantMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tool_id = first
            .content
            .iter()
            .find_map(|block| match block {
                AssistantContent::ToolCall(tool) => Some(tool.id.clone()),
                _ => None,
            })
            .ok_or("missing tool")?;
        context.messages.push(Message::Assistant(first));
        context
            .messages
            .push(Message::ToolResult(ToolResultMessage {
                tool_call_id: tool_id,
                tool_name: "echo".into(),
                content: vec![UserBlock::Text(faux_text("package.json contents here"))],
                details: None,
                is_error: false,
                timestamp: 0.0,
            }));
        Ok(())
    }
    /// Exercise documented flow through the public invocation boundary.
    async fn documented_flow() -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut output = Vec::new();
        let registration = register_faux_provider(RegisterFauxProviderOptions {
            tokens_per_second: None,
            ..RegisterFauxProviderOptions::default()
        });
        let model = registration.get_model(None).ok_or("missing model")?.clone();
        let mut context = documented_context();
        registration.set_responses(documented_tool_response());
        let first = complete(
            model.clone(),
            context.clone(),
            Some(ProviderStreamOptions {
                common: StreamOptions {
                    session_id: Some("session-1".into()),
                    cache_retention: Some(CacheRetention::Short),
                    ..StreamOptions::default()
                },
                ..ProviderStreamOptions::default()
            }),
        )
        .await?;
        let first = first.read().map_err(|_| "message lock poisoned")?.clone();
        append_documented_result(&mut context, first)?;
        registration.set_responses(documented_summary_response());
        let events = stream(model, context, None)?;
        while let Some(event) = events.next().await {
            output.push(
                serde_json::to_value(event)?["type"]
                    .as_str()
                    .ok_or("missing event type")?
                    .to_owned(),
            );
        }
        let multi_model = documented_multi_model();
        output.push(
            multi_model
                .get_model(Some("faux-thinker"))
                .is_some_and(|model| model.reasoning)
                .to_string(),
        );
        output.push(registration.get_pending_response_count().to_string());
        output.push(registration.state.call_count().to_string());
        registration.unregister();
        multi_model.unregister();
        Ok(output)
    }

    #[test]
    /// Exercise documented flow runs offline through the public invocation boundary.
    fn maestro_documented_flow_runs_offline() {
        let output = wait(documented_flow()).unwrap();
        assert_eq!(&output[output.len() - 3..], ["true", "0", "2"]);
        assert_eq!(output[0], "start");
        assert_eq!(output[1], "thinking_start");
        let thinking_end = output
            .iter()
            .position(|line| line == "thinking_end")
            .unwrap();
        assert!(
            output[2..thinking_end]
                .iter()
                .all(|line| line == "thinking_delta")
        );
        assert_eq!(output[thinking_end + 1], "text_start");
        let text_end = output.iter().position(|line| line == "text_end").unwrap();
        assert!(
            output[thinking_end + 2..text_end]
                .iter()
                .all(|line| line == "text_delta")
        );
        assert_eq!(output[text_end + 1], "done");
    }
}
