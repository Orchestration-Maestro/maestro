#[cfg(test)]
mod tests {
    use maestro_models::{StringEnumOptions, headers_to_record, short_hash, string_enum};
    use serde_json::json;
    #[test]
    fn maestro_headers_and_schema_keep_supplied_values() {
        assert!(headers_to_record([]).is_empty());
        let headers = headers_to_record([
            ("content-type", "text/plain"),
            ("x", "first"),
            ("x", "last"),
        ]);
        assert_eq!(headers["content-type"], "text/plain");
        assert_eq!(headers["x"], "last");
        assert_eq!(string_enum(&[], None), json!({"type":"string", "enum":[]}));
        assert_eq!(
            string_enum(
                &["z", "a"],
                Some(StringEnumOptions {
                    description: Some(""),
                    default: Some("")
                })
            ),
            json!({"type":"string", "enum":["z", "a"]})
        );
        for value in [" ", "\u{feff}", "\u{0085}", "outside"] {
            assert_eq!(
                string_enum(
                    &["inside"],
                    Some(StringEnumOptions {
                        description: Some(value),
                        default: Some(value)
                    })
                ),
                json!({"type":"string", "enum":["inside"], "description":value, "default":value})
            );
        }
    }

    #[test]
    fn maestro_hash_matches_utf16_vectors() {
        for (input, expected) in [
            ("", "k4n83c7h0j2b"),
            ("hello", "1h6qa0qrowduu"),
            ("\0", "tlnwb21t8w38a"),
            ("\u{feff}a", "dlr9atvhhasa"),
            ("\u{0085}", "b78s7412emlzt"),
            ("😀", "13wj7r7usi372"),
            ("a😀b", "12yrce3kjl8pw"),
        ] {
            assert_eq!(short_hash(input), expected);
        }
        assert_eq!(short_hash(&"x".repeat(1000)), "zykls21ciz8e3");
    }

    #[test]
    fn maestro_assistant_stream_is_exported_from_record_types() {
        use maestro_models::records::event_stream::create_assistant_message_event_stream;
        use maestro_models::records::types::{
            AssistantMessageEvent, AssistantMessageEventStream, DoneReason, ErrorReason,
        };
        fn accept(stream: AssistantMessageEventStream) -> AssistantMessageEventStream {
            stream
        }
        let fixture = json!({"role":"assistant","content":[],"api":"custom","provider":"custom","model":"m","usage":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"totalTokens":0.0,"cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"total":0.0}},"stopReason":"stop","timestamp":1.0});
        let message = std::sync::Arc::new(std::sync::RwLock::new(
            serde_json::from_value(fixture).unwrap(),
        ));
        for event in [
            AssistantMessageEvent::Done {
                reason: DoneReason::Stop,
                message: std::sync::Arc::clone(&message),
            },
            AssistantMessageEvent::Error {
                reason: ErrorReason::Aborted,
                error: std::sync::Arc::clone(&message),
            },
        ] {
            let stream = accept(create_assistant_message_event_stream());
            stream.push(event);
            let mut result = std::pin::pin!(stream.result());
            let std::task::Poll::Ready(actual) = std::future::Future::poll(
                result.as_mut(),
                &mut std::task::Context::from_waker(std::task::Waker::noop()),
            ) else {
                panic!("terminal result pending")
            };
            assert!(std::sync::Arc::ptr_eq(&actual, &message));
        }
    }

    #[test]
    fn maestro_diagnostics_append_with_current_timestamp() {
        use maestro_models::records::diagnostics::{
            DiagnosticInput, append_assistant_message_diagnostic,
            create_assistant_message_diagnostic,
        };
        use maestro_models::records::types::AssistantMessage;
        let now = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64()
                * 1000.0
        };
        let before = now().floor();
        let details = json!({"attempt":2});
        let diagnostic = create_assistant_message_diagnostic(
            "retry",
            DiagnosticInput::Text("failure"),
            Some(details.as_object().unwrap().clone()),
        );
        let after = now();
        assert!(diagnostic.timestamp >= before && diagnostic.timestamp <= after);
        assert_eq!(diagnostic.r#type, "retry");
        assert_eq!(
            diagnostic.details,
            Some(details.as_object().unwrap().clone())
        );
        assert_eq!(diagnostic.error.as_ref().unwrap().message, "failure");
        let fixture = json!({"role":"assistant","content":[],"api":"custom","provider":"custom","model":"m","usage":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"totalTokens":0.0,"cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"total":0.0}},"stopReason":"stop","timestamp":1.0});
        let mut message: AssistantMessage = serde_json::from_value(fixture).unwrap();
        append_assistant_message_diagnostic(&mut message, diagnostic.clone());
        let second =
            create_assistant_message_diagnostic("recovered", DiagnosticInput::Text("ok"), None);
        append_assistant_message_diagnostic(&mut message, second.clone());
        assert_eq!(message.diagnostics, Some(vec![diagnostic, second]));
    }
    fn assert_literals<T: serde::de::DeserializeOwned + serde::Serialize>(literals: &[&str]) {
        for literal in literals {
            let record: T = serde_json::from_value(json!(literal)).unwrap();
            assert_eq!(serde_json::to_value(record).unwrap(), json!(literal));
        }
    }

    #[test]
    fn maestro_runtime_option_records_keep_wire_contracts() {
        use maestro_models::records::types::{CacheRetention, ThinkingBudgets, Transport};
        assert_literals::<CacheRetention>(&["none", "short", "long"]);
        assert_literals::<Transport>(&["sse", "websocket", "websocket-cached", "auto"]);
        let budget = json!({"minimal":1.0,"low":2.0,"medium":3.0,"high":4.0});
        let record: ThinkingBudgets = serde_json::from_value(budget.clone()).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), budget);
    }
}
