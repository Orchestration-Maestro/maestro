//! JavaScript execution through an owned sandbox host.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support::run;
use maestro_app::browser::{components::sandboxed_iframe::*, tools::javascript_repl::*};
use maestro_app::{SandboxResult, SandboxUrlProvider};
use maestro_models::DiagnosticErrorInfo;
use std::sync::Arc;

#[test]
fn repl_preflight_preserves_error_precedence() {
    run(async {
        support::javascript_execution::rows("repl_preflight_preserves_error_precedence").await;
    });
}
/// Successful sandbox with no output.
struct QuietHost;
impl SandboxHost for QuietHost {
    fn create_hidden(
        &self,
        _: Option<SandboxUrlProvider>,
    ) -> Result<Box<dyn SandboxIframe>, DiagnosticErrorInfo> {
        Ok(Box::new(QuietHost))
    }
}
impl SandboxIframe for QuietHost {
    fn execute(
        &self,
        _: SandboxExecution,
    ) -> maestro_models::BoxFuture<Result<SandboxResult, DiagnosticErrorInfo>> {
        Box::pin(async {
            Ok(SandboxResult {
                success: true,
                console: vec![],
                files: None,
                error: None,
                return_value: None,
            })
        })
    }
    fn remove(&self) {}
}
#[test]
fn repl_accepts_whitespace_code() {
    run(async {
        support::javascript_execution::rows("repl_accepts_whitespace_code").await;
    });
}

use crate::support;
#[test]
fn repl_output_preserves_console_return_and_file_order() {
    run(async {
        support::javascript_execution::rows("repl_output_preserves_console_return_and_file_order")
            .await;
    });
}

#[test]
fn repl_factory_declares_schema_and_exports() {
    run(async {
        let repl = maestro_app::create_javascript_repl_tool(Arc::new(QuietHost));
        let (definition, execute) = {
            let tool = repl.tool.read().unwrap();
            let metadata = support::javascript_execution::corpus().metadata;
            assert_eq!(tool.definition.name, metadata["name"]);
            assert_eq!(tool.label, metadata["label"]);
            assert_eq!(
                serde_json::to_string(&tool.definition.parameters).unwrap(),
                metadata["schemaText"]
            );
            assert_eq!(*tool.definition.parameters, metadata["parameters"]);
            assert_eq!(
                repl.sandbox_url_provider.read().unwrap().is_some(),
                metadata["defaultSandboxUrlPresent"].as_bool().unwrap()
            );
            let factory = repl
                .runtime_providers_factory
                .read()
                .unwrap()
                .clone()
                .unwrap();
            assert_eq!(
                factory().unwrap().len(),
                metadata["defaultProviders"].as_array().unwrap().len()
            );
            let definition = tool.definition.clone();
            let execute = Arc::clone(&tool.execute);
            (definition, execute)
        };
        for (name, row) in support::javascript_execution::corpus().argument_validation {
            let call = maestro_models::ToolCall {
                id: name.clone(),
                name: definition.name.clone(),
                arguments: row["input"].clone(),
                thought_signature: None,
            };
            let result = maestro_models::arguments::validate_tool_arguments(&definition, &call);
            if row["expected"]["branch"] == "success" {
                assert_eq!(result.unwrap(), row["expected"]["arguments"], "{name}");
            } else {
                assert!(result.is_err(), "{name}");
            }
            if !row["input"]["title"].is_string() || !row["input"]["code"].is_string() {
                assert!(
                    execute(name, row["input"].clone(), None, None)
                        .await
                        .is_err()
                );
            }
        }
    });
}

#[test]
fn repl_uses_shared_whitespace_semantics() {
    run(async {
        support::javascript_execution::rows("repl_uses_shared_whitespace_semantics").await;
    });
}

#[test]
fn repl_return_domain_uses_canonical_text() {
    run(async {
        support::javascript_execution::rows("repl_return_domain_uses_canonical_text").await;
    });
}

#[test]
fn repl_file_hints_follow_exact_code_match() {
    run(async {
        support::javascript_execution::rows("repl_file_hints_follow_exact_code_match").await;
    });
}

#[test]
fn repl_optional_factory_runs_at_each_phase() {
    run(async {
        support::javascript_execution::rows("repl_optional_factory_runs_at_each_phase").await;
    });
}

#[test]
fn repl_failed_results_preserve_messages_and_cleanup() {
    run(async {
        support::javascript_execution::rows("repl_failed_results_preserve_messages_and_cleanup")
            .await;
    });
}

#[test]
fn repl_host_errors_remove_sandbox() {
    run(async {
        support::javascript_execution::rows("repl_host_errors_remove_sandbox").await;
    });
}

#[test]
fn repl_files_keep_utf8_bytes_metadata_and_order() {
    run(async {
        support::javascript_execution::rows("repl_files_keep_utf8_bytes_metadata_and_order").await;
    });
}

#[test]
fn repl_base64_crosses_old_chunk_boundary() {
    run(async {
        support::javascript_execution::rows("repl_base64_crosses_old_chunk_boundary").await;
    });
}

/// Snapshot the callback without retaining the tool guard while invoking it.
fn description(tool: &maestro_agent::SharedAgentTool) -> Result<String, DiagnosticErrorInfo> {
    let (definition, callback) = {
        let tool = tool.read().unwrap();
        (tool.definition.clone(), tool.description.clone())
    };
    maestro_agent::AgentTool::resolve_definition(definition, callback).map(|tool| tool.description)
}
#[test]
fn repl_description_rechecks_factory_and_preserves_order() {
    use support::javascript_execution::{corpus, providers};
    let repl = maestro_app::create_javascript_repl_tool(Arc::new(QuietHost));
    for row in corpus().descriptions {
        let values: Vec<String> = serde_json::from_value(row["values"].clone()).unwrap();
        *repl.runtime_providers_factory.write().unwrap() =
            Some(Arc::new(move || Ok(providers(&values))));
        assert_eq!(description(&repl.tool).unwrap(), row["description"]);
    }
    let state = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let current = Arc::clone(&state);
    *repl.runtime_providers_factory.write().unwrap() =
        Some(Arc::new(move || Ok(providers(&current.lock().unwrap()))));
    let dynamic = corpus().dynamic;
    assert_eq!(description(&repl.tool).unwrap(), dynamic["initial"]);
    *state.lock().unwrap() = vec!["external state changed".into()];
    assert_eq!(description(&repl.tool).unwrap(), dynamic["changed"]);
    *state.lock().unwrap() = vec!["later external state".into()];
    assert_eq!(description(&repl.tool).unwrap(), dynamic["later"]);
}

/// Test-owned execution operation.
type SandboxOperation<V> = Arc<
    dyn Fn(
            SandboxExecution<V>,
        ) -> maestro_models::BoxFuture<Result<SandboxResult<V>, DiagnosticErrorInfo>>
        + Send
        + Sync,
>;
/// Sandbox with test-supplied owned operations.
struct Controlled<V: SandboxValue> {
    /// Creation observation.
    create: Arc<dyn Fn(Option<SandboxUrlProvider>) + Send + Sync>,
    /// Execution observation.
    execute: SandboxOperation<V>,
    /// Removal observation.
    remove: Arc<dyn Fn() + Send + Sync>,
}
impl<V: SandboxValue> SandboxHost<V> for Controlled<V> {
    fn create_hidden(
        &self,
        url: Option<SandboxUrlProvider>,
    ) -> Result<Box<dyn SandboxIframe<V>>, DiagnosticErrorInfo> {
        (self.create)(url);
        Ok(Box::new(Self {
            create: Arc::clone(&self.create),
            execute: Arc::clone(&self.execute),
            remove: Arc::clone(&self.remove),
        }))
    }
}
impl<V: SandboxValue> SandboxIframe<V> for Controlled<V> {
    fn execute(
        &self,
        request: SandboxExecution<V>,
    ) -> maestro_models::BoxFuture<Result<SandboxResult<V>, DiagnosticErrorInfo>> {
        (self.execute)(request)
    }
    fn remove(&self) {
        (self.remove)();
    }
}
/// A successful empty result for any caller-owned value domain.
fn empty<V: SandboxValue>() -> SandboxResult<V> {
    SandboxResult {
        success: true,
        console: vec![],
        files: None,
        error: None,
        return_value: None,
    }
}
#[test]
fn repl_configuration_reaches_host_after_registration() {
    run(async {
        use serde_json::json;
        use std::sync::Mutex;
        use support::javascript_execution::corpus;
        let recording = corpus().configuration;
        let observed = Arc::new(Mutex::new(Vec::<(
            Option<SandboxUrlProvider>,
            SandboxExecution,
        )>::new()));
        let host = recording_host(&observed);
        let repl = maestro_app::create_javascript_repl_tool(Arc::new(host));
        let registered = Arc::clone(&repl.tool);
        let execute = Arc::clone(&registered.read().unwrap().execute);

        let count = Arc::new(Mutex::new(0_usize));
        let mut previous = (vec![], None);
        for index in 0..5 {
            let row = &recording["observations"][index];
            configure(index, &recording, &repl, &mut previous, &count);
            let signal = maestro_models::Cancellation::new();
            assert_eq!(signal.is_aborted(), recording["input"]["aborted"]);
            let before = *count.lock().unwrap();
            let result = execute(format!("call-{index}"), json!({"title":recording["input"]["titles"][index],"code":recording["input"]["codes"][index]}), Some(signal.clone()), None).await.unwrap();
            assert_eq!(
                json!({"content":result.content,"details":result.details}),
                recording["results"][index]
            );
            assert_eq!(
                json!(*count.lock().unwrap() - before),
                recording["factoryCalls"][index]
            );
            let requests = observed.lock().unwrap();
            let (url, request) = &requests[index];
            check_configuration(
                (url, request),
                (&previous.0, &previous.1),
                row,
                recording["input"]["urls"][index].as_str(),
            );
            assert_eq!(
                request.signal.as_ref().unwrap().is_aborted(),
                signal.is_aborted()
            );
            signal.abort();
            assert_eq!(
                request.signal.as_ref().unwrap().is_aborted(),
                row["signalSame"]
            );
        }
        let requests = observed.lock().unwrap();
        let ids: std::collections::HashSet<_> =
            requests.iter().map(|(_, r)| &r.sandbox_id).collect();
        assert_eq!(
            ids.len() == requests.len(),
            recording["identifiersDistinct"]
        );
    });
}

#[test]
fn repl_configuration_lifetime_drops_without_tool_cycle() {
    run(async {
        let host = Arc::new(QuietHost);
        let weak_host = Arc::downgrade(&host);
        let repl = maestro_app::create_javascript_repl_tool(host);
        let weak_factory = Arc::downgrade(&repl.runtime_providers_factory);
        let weak_url = Arc::downgrade(&repl.sandbox_url_provider);
        let registered = Arc::clone(&repl.tool);
        drop(repl);
        assert!(weak_host.upgrade().is_some());
        let execute = Arc::clone(&registered.read().unwrap().execute);
        execute(
            "call".into(),
            serde_json::json!({"title":"run","code":"1"}),
            None,
            None,
        )
        .await
        .unwrap();
        drop(registered);
        assert!(weak_host.upgrade().is_some());
        drop(execute);
        assert!(weak_host.upgrade().is_none());
        assert!(weak_factory.upgrade().is_none());
        assert!(weak_url.upgrade().is_none());
    });
}
/// Non-JSON SDK object with identity and an owned payload.
struct Payload {
    /// Caller-owned value retained across suspension.
    value: Arc<Vec<u8>>,
    /// Cleanup state observed during display.
    removed: Arc<std::sync::atomic::AtomicBool>,
    /// Select the fallible formatting route.
    fail: bool,
}
impl SandboxValue for Payload {
    fn format_repl_value(&self) -> Result<std::borrow::Cow<'_, str>, DiagnosticErrorInfo> {
        assert!(self.removed.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(self.value.as_slice(), [42, 7]);
        if self.fail {
            Err(DiagnosticErrorInfo::error("display failed"))
        } else {
            Ok("owned payload".into())
        }
    }
}
#[test]
fn repl_generic_return_keeps_non_json_caller_value() {
    run(async {
        retained_payload().await;
    });
}
#[test]
fn repl_hidden_lifetime_closes_before_projection_and_ids_differ() {
    run(async {
        use std::sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
        };
        let ids = Arc::new(Mutex::new(vec![]));
        for mode in 0..3 {
            let removed = Arc::new(AtomicBool::new(false));
            let captured = Arc::clone(&ids);
            let closed = Arc::clone(&removed);
            let host = Controlled {
                create: Arc::new(|_| {}),
                execute: Arc::new(move |request: SandboxExecution<Payload>| {
                    captured.lock().unwrap().push(request.sandbox_id);
                    let removed = Arc::clone(&closed);
                    let result = payload_result(mode, removed);
                    Box::pin(std::future::ready(result))
                }),
                remove: {
                    let removed = Arc::clone(&removed);
                    Arc::new(move || removed.store(true, Ordering::SeqCst))
                },
            };
            let result = execute_javascript(
                "1".into(),
                vec![],
                None,
                SandboxEnvironment {
                    host: Arc::new(host),
                    sandbox_url_provider: None,
                },
            )
            .await;
            match mode {
                0 => assert_eq!(result.unwrap().output, "=> owned payload"),
                1 => assert_eq!(result.err().unwrap().message, "display failed"),
                _ => assert_eq!(result.err().unwrap().message, "host failed"),
            }
            assert!(removed.load(Ordering::SeqCst));
        }
        let ids = ids.lock().unwrap();
        assert!(
            ids.iter()
                .all(|id| id.starts_with("repl-") && id.split('-').count() == 3)
        );
        assert_eq!(
            ids.iter().collect::<std::collections::HashSet<_>>().len(),
            3
        );
    });
}

/// Provider of caller-owned values and recorded lifecycle notifications.
struct OwnedProvider {
    /// Shared payload, not a JSON projection.
    value: Arc<Vec<u8>>,
    /// Hook order.
    hooks: Arc<std::sync::Mutex<Vec<String>>>,
}
impl maestro_app::SandboxRuntimeProvider<Payload> for OwnedProvider {
    fn get_data(&self) -> Result<indexmap::IndexMap<String, Payload>, DiagnosticErrorInfo> {
        Ok([(
            "retained".into(),
            Payload {
                value: Arc::clone(&self.value),
                removed: Arc::new(std::sync::atomic::AtomicBool::new(true)),
                fail: false,
            },
        )]
        .into())
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok("function(){ return 42; }".into())
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Err(DiagnosticErrorInfo::error("description unavailable"))
    }
    fn handle_message(
        &self,
        message: Payload,
        respond: maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRespond<Payload>,
    ) -> maestro_models::BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(async move { respond(message) })
    }
    fn on_execution_start(
        &self,
        id: &str,
        signal: Option<maestro_models::Cancellation>,
    ) -> Result<(), DiagnosticErrorInfo> {
        assert!(signal.unwrap().is_aborted());
        self.hooks.lock().unwrap().push(format!("start:{id}"));
        Ok(())
    }
    fn on_execution_end(&self, id: &str) -> Result<(), DiagnosticErrorInfo> {
        self.hooks.lock().unwrap().push(format!("end:{id}"));
        Ok(())
    }
}
#[test]
fn repl_provider_contract_keeps_owned_data_runtime_and_hooks() {
    run(async {
        let value = Arc::new(vec![42, 7]);
        let hooks = Arc::new(std::sync::Mutex::new(vec![]));
        let provider: Arc<dyn maestro_app::SandboxRuntimeProvider<Payload>> =
            Arc::new(OwnedProvider {
                value: Arc::clone(&value),
                hooks: Arc::clone(&hooks),
            });
        let mut data = provider.get_data().unwrap();
        assert_eq!(data.keys().collect::<Vec<_>>(), ["retained"]);
        let payload = data.shift_remove("retained").unwrap();
        assert!(Arc::ptr_eq(&payload.value, &value));
        assert_eq!(provider.get_runtime().unwrap(), "function(){ return 42; }");
        assert_eq!(
            provider.get_description().unwrap_err().message,
            "description unavailable"
        );
        let received = Arc::new(std::sync::Mutex::new(None));
        let response = Arc::clone(&received);
        let respond = Arc::new(move |message: Payload| {
            *response.lock().unwrap() = Some(message);
            Err(DiagnosticErrorInfo::error("respond failed"))
        });
        assert_eq!(
            provider
                .handle_message(payload, respond)
                .await
                .unwrap_err()
                .message,
            "respond failed"
        );
        assert!(Arc::ptr_eq(
            &received.lock().unwrap().as_ref().unwrap().value,
            &value
        ));
        let signal = maestro_models::Cancellation::new();
        signal.abort();
        provider.on_execution_start("owned", Some(signal)).unwrap();
        provider.on_execution_end("owned").unwrap();
        assert_eq!(*hooks.lock().unwrap(), ["start:owned", "end:owned"]);
        let default: Arc<dyn maestro_app::SandboxRuntimeProvider> =
            Arc::new(support::javascript_execution::Provider("default".into()));
        default
            .handle_message(
                serde_json::Value::Null,
                Arc::new(|_| panic!("default must not respond")),
            )
            .await
            .unwrap();
        default.on_execution_start("default", None).unwrap();
        default.on_execution_end("default").unwrap();
        assert_eq!(default.get_data().unwrap()["id"], "default");
    });
}

/// Authoritative controlled response, optionally requesting the REPL once.
fn response(call: bool) -> maestro_models::SharedAssistantMessage {
    use maestro_models::{AssistantContent, AssistantMessage, StopReason, ToolCall};
    Arc::new(std::sync::RwLock::new(AssistantMessage {
        content: if call { vec![AssistantContent::ToolCall(ToolCall { id: "controlled".into(), name: "javascript_repl".into(), arguments: serde_json::from_value(serde_json::json!({"title":"run","code":"1"})).unwrap(), thought_signature: None })] } else { vec![] },
        api: "fixture".into(), provider: "fixture".into(), model: "fixture".into(), response_model: None, response_id: None, diagnostics: None,
        usage: serde_json::from_value(serde_json::json!({"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}})).unwrap(),
        stop_reason: if call { StopReason::ToolUse } else { StopReason::Stop }, error_message: None, timestamp: 1.0,
    }))
}
#[test]
fn repl_requests_use_current_description_after_auth() {
    run(async {
        use std::sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        };
        use support::javascript_execution::{corpus, providers};
        let recording = corpus().between_turns;
        let state = Arc::new(Mutex::new(
            recording["input"]["beforeKey"].as_str().unwrap().to_owned(),
        ));
        let supplied = Arc::clone(&state);
        let repl = maestro_app::create_javascript_repl_tool(Arc::new(QuietHost));
        *repl.runtime_providers_factory.write().unwrap() = Some(Arc::new(move || {
            Ok(providers(&[supplied.lock().unwrap().clone()]))
        }));
        let key_calls = Arc::new(AtomicUsize::new(0));
        let keys = Arc::clone(&key_calls);
        let after_key = recording["input"]["afterKey"].as_str().unwrap().to_owned();
        let current = Arc::clone(&state);
        let executions = Arc::new(AtomicUsize::new(0));
        let calls = Arc::clone(&executions);
        let after_tool = recording["input"]["afterTool"].as_str().unwrap().to_owned();
        change_after_execution(&repl.tool, state, after_tool, calls);
        let requests = Arc::new(Mutex::new(vec![]));
        let seen = Arc::clone(&requests);
        let static_tool = static_tool();
        let tasks = Arc::new(Mutex::new(Vec::new()));
        let submitted = Arc::clone(&tasks);
        let agent =
            maestro_agent::Agent::<std::convert::Infallible>::new(maestro_agent::AgentOptions {
                spawn: Some(Arc::new(move |task| submitted.lock().unwrap().push(task))),
                get_api_key: Some(Arc::new(move |_| changing_key(&keys, &current, &after_key))),
                stream_fn: Some(observe_requests(seen)),
                ..Default::default()
            });
        agent.state().write().unwrap().set_tools(vec![
            repl.tool,
            Arc::new(std::sync::RwLock::new(static_tool)),
        ]);
        let agent = Arc::new(agent);
        let completion = agent.prompt(maestro_agent::PromptInput::Text {
            text: "controlled".into(),
            images: vec![],
        });
        let task = tasks.lock().unwrap().pop().unwrap();
        task.await;
        completion.await.unwrap();
        assert_eq!(
            serde_json::json!(*requests.lock().unwrap()),
            recording["descriptions"]
        );
        assert_eq!(
            serde_json::json!(key_calls.load(Ordering::SeqCst)),
            recording["keyLookups"]
        );
        assert_eq!(
            serde_json::json!(executions.load(Ordering::SeqCst)),
            recording["toolExecutions"]
        );
    });
}
#[test]
fn repl_callback_errors_precede_preflight_or_invocation() {
    run(async {
        use support::javascript_execution::corpus;
        for row in corpus().callback_errors {
            let admissions = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let opened = Arc::clone(&admissions);
            let host = Controlled::<serde_json::Value> {
                create: Arc::new(move |_| {
                    opened.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }),
                execute: Arc::new(|_| Box::pin(async { Ok(empty()) })),
                remove: Arc::new(|| {}),
            };
            let repl = maestro_app::create_javascript_repl_tool(Arc::new(host));
            let expected = row["message"].as_str().unwrap().to_owned();
            let supplied = expected.clone();
            if row["mode"] == "provider_description" {
                let provider: Arc<dyn maestro_app::SandboxRuntimeProvider> =
                    Arc::new(FailingDescription(supplied));
                *repl.runtime_providers_factory.write().unwrap() =
                    Some(Arc::new(move || Ok(vec![Arc::clone(&provider)])));
            } else {
                *repl.runtime_providers_factory.write().unwrap() = Some(failing_factory(supplied));
            }
            if row["mode"] == "factory_execution" {
                let execute = Arc::clone(&repl.tool.read().unwrap().execute);
                assert_eq!(
                    execute(
                        "call".into(),
                        serde_json::json!({"title":"","code":""}),
                        None,
                        None
                    )
                    .await
                    .unwrap_err()
                    .message,
                    expected
                );
            } else if row["mode"] == "conversion_snapshot" {
                let tool = Arc::try_unwrap(repl.tool)
                    .ok()
                    .unwrap()
                    .into_inner()
                    .unwrap();
                let error = maestro_tools::create_tool_definition_from_agent_tool::<()>(tool)
                    .err()
                    .unwrap();
                assert_eq!(error.message, expected);
                assert_eq!(error.stack.as_deref(), Some("retained"));
            } else {
                assert_eq!(description(&repl.tool).unwrap_err().message, expected);
                failed_description_run(repl.tool, &expected).await;
            }
            assert_eq!(admissions.load(std::sync::atomic::Ordering::SeqCst), 0);
        }
    });
}
/// Provider whose description is a caller failure.
struct FailingDescription(String);
impl maestro_app::SandboxRuntimeProvider for FailingDescription {
    fn get_data(
        &self,
    ) -> Result<indexmap::IndexMap<String, serde_json::Value>, DiagnosticErrorInfo> {
        Ok(indexmap::IndexMap::new())
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Err(DiagnosticErrorInfo::error(self.0.clone()))
    }
}

/// Expected identity-bearing configuration.
type Configuration = (
    Vec<Arc<dyn maestro_app::SandboxRuntimeProvider>>,
    Option<SandboxUrlProvider>,
);
/// Change independent settings at their recorded calls.
fn configure(
    index: usize,
    recording: &serde_json::Value,
    repl: &JavaScriptReplTool,
    previous: &mut Configuration,
    count: &Arc<std::sync::Mutex<usize>>,
) {
    use support::javascript_execution::providers;
    if index == 0 || index == 1 || index == 3 {
        previous.0 = providers(
            &serde_json::from_value::<Vec<String>>(
                recording["input"]["providerIds"][index].clone(),
            )
            .unwrap(),
        );
        let supplied = previous.0.clone();
        let count = Arc::clone(count);
        let factory: RuntimeProvidersFactory<serde_json::Value> = Arc::new(move || {
            *count.lock().unwrap() += 1;
            Ok(supplied.clone())
        });
        *repl.runtime_providers_factory.write().unwrap() = (index != 3).then_some(factory);
    }
    if index == 0 || index == 2 || index == 4 {
        previous.1 = recording["input"]["urls"][index].as_str().map(|url| {
            let url = url.to_owned();
            Arc::new(move || Ok(url.clone())) as SandboxUrlProvider
        });
        *repl.sandbox_url_provider.write().unwrap() = previous.1.clone();
    }
}
/// Assert the observable code, provider aliases and URL callback identity.
fn check_configuration(
    actual: (&Option<SandboxUrlProvider>, &SandboxExecution),
    expected: (
        &[Arc<dyn maestro_app::SandboxRuntimeProvider>],
        &Option<SandboxUrlProvider>,
    ),
    row: &serde_json::Value,
    expected_url: Option<&str>,
) {
    use serde_json::json;
    let (url, request) = actual;
    assert_eq!(request.code, row["code"]);
    assert_eq!(
        json!(
            request
                .runtime_providers
                .iter()
                .map(|p| p.get_data().unwrap()["id"].clone())
                .collect::<Vec<_>>()
        ),
        row["providerIds"]
    );
    assert_eq!(
        request
            .runtime_providers
            .iter()
            .zip(expected.0)
            .all(|(a, b)| Arc::ptr_eq(a, b)),
        row["providerHandlesSame"]
    );
    assert_eq!(
        url.as_ref().map(|url| url().unwrap()),
        expected_url.map(str::to_owned)
    );
    assert_eq!(
        match (url, expected.1) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        },
        row["urlProviderSame"]
    );
}

/// Return a retained value or a host failure for cleanup qualification.
fn payload_result(
    mode: usize,
    removed: Arc<std::sync::atomic::AtomicBool>,
) -> Result<SandboxResult<Payload>, DiagnosticErrorInfo> {
    if mode == 2 {
        return Err(DiagnosticErrorInfo::error("host failed"));
    }
    let mut result = empty();
    result.return_value = Some(Payload {
        value: Arc::new(vec![42, 7]),
        removed,
        fail: mode == 1,
    });
    Ok(result)
}

/// Pause and recover a non-JSON caller value through the public host contract.
async fn retained_payload() {
    let removed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let value = Arc::new(vec![42, 7]);
    let supplied = Arc::clone(&value);
    let closed = Arc::clone(&removed);
    let (started, entered) = tokio::sync::oneshot::channel();
    let started = Arc::new(std::sync::Mutex::new(Some(started)));
    let (resume, resumed) = tokio::sync::oneshot::channel();
    let resumed = Arc::new(std::sync::Mutex::new(Some(resumed)));
    let cleanup = Arc::clone(&removed);
    let host = Controlled {
        create: Arc::new(|_| {}),
        execute: Arc::new(move |_| {
            let value = Arc::clone(&supplied);
            let removed = Arc::clone(&closed);
            let resumed = resumed.lock().unwrap().take().unwrap();
            started.lock().unwrap().take().unwrap().send(()).unwrap();
            Box::pin(async move {
                resumed.await.unwrap();
                let mut result = empty();
                result.return_value = Some(Payload {
                    value,
                    removed,
                    fail: false,
                });
                Ok(result)
            })
        }),
        remove: Arc::new(move || cleanup.store(true, std::sync::atomic::Ordering::SeqCst)),
    };
    let task = tokio::spawn(async move {
        execute_javascript(
            "1".into(),
            vec![],
            None,
            SandboxEnvironment {
                host: Arc::new(host),
                sandbox_url_provider: None,
            },
        )
        .await
        .unwrap()
    });
    entered.await.unwrap();
    assert!(!task.is_finished());
    assert!(!removed.load(std::sync::atomic::Ordering::SeqCst));
    resume.send(()).unwrap();
    let result = task.await.unwrap();
    assert_eq!(result.output, "=> owned payload");
    assert!(removed.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(Arc::strong_count(&value), 1);
}

/// A description failure prevents the supplied stream callback from running.
async fn failed_description_run(tool: maestro_agent::SharedAgentTool, expected: &str) {
    let tasks = Arc::new(std::sync::Mutex::new(Vec::new()));
    let submitted = Arc::clone(&tasks);
    let agent =
        maestro_agent::Agent::<std::convert::Infallible>::new(maestro_agent::AgentOptions {
            spawn: Some(Arc::new(move |task| submitted.lock().unwrap().push(task))),
            stream_fn: Some(Arc::new(|_, _, _, _| {
                panic!("description failure must precede model invocation")
            })),
            ..Default::default()
        });
    agent.state().write().unwrap().set_tools(vec![tool]);
    let agent = Arc::new(agent);
    let completion = agent.prompt(maestro_agent::PromptInput::Text {
        text: "controlled".into(),
        images: vec![],
    });
    let task = tasks.lock().unwrap().pop().unwrap();
    task.await;
    completion.await.unwrap();
    assert_eq!(
        agent.state().read().unwrap().error_message(),
        Some(expected)
    );
}

/// Retained host observations for configuration runs.
type Requests = Arc<std::sync::Mutex<Vec<(Option<SandboxUrlProvider>, SandboxExecution)>>>;
/// Construct a host that records the actual supplied callback and execution handles.
fn recording_host(observed: &Requests) -> Controlled<serde_json::Value> {
    use std::sync::Mutex;
    let pending_url = Arc::new(Mutex::new(None));
    let url = Arc::clone(&pending_url);
    let requests = Arc::clone(observed);
    Controlled {
        create: Arc::new(move |callback| *url.lock().unwrap() = callback),
        execute: Arc::new(move |request| {
            let url = pending_url.lock().unwrap().take();
            requests.lock().unwrap().push((url, request));
            Box::pin(async { Ok(empty()) })
        }),
        remove: Arc::new(|| {}),
    }
}

/// Supply a stable caller diagnostic, including its stack.
fn failing_factory(message: String) -> RuntimeProvidersFactory<serde_json::Value> {
    Arc::new(move || {
        Err(DiagnosticErrorInfo {
            message: message.clone(),
            name: Some("CallerFailure".into()),
            stack: Some("retained".into()),
            code: None,
        })
    })
}

/// Change external factory state after the first tool settles.
fn change_after_execution(
    tool: &maestro_agent::SharedAgentTool,
    state: Arc<std::sync::Mutex<String>>,
    after_tool: String,
    calls: Arc<std::sync::atomic::AtomicUsize>,
) {
    use std::sync::atomic::Ordering;
    let old_execute = Arc::clone(&tool.read().unwrap().execute);
    tool.write().unwrap().execute = Arc::new(move |id, args, signal, update| {
        let future = old_execute(id, args, signal, update);
        let current = Arc::clone(&state);
        let text = after_tool.clone();
        let calls = Arc::clone(&calls);
        Box::pin(async move {
            let result = future.await?;
            *current.lock().unwrap() = text;
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(result)
        })
    });
}

/// A declaration whose description must stay unchanged.
fn static_tool() -> maestro_agent::AgentTool {
    maestro_agent::AgentTool::typed::<serde_json::Value, ()>(
        maestro_models::Tool {
            name: "static".into(),
            description: "unchanged".into(),
            parameters: serde_json::json!({"type":"object"}).into(),
        },
        "Static".into(),
        Arc::new(|_, _, _, _| {
            Box::pin(async {
                Ok(maestro_agent::AgentToolResult {
                    content: vec![],
                    details: None,
                    terminate: None,
                    usage: None,
                })
            })
        }),
    )
}

/// Observe both model requests and return a single tool-call turn.
fn observe_requests(seen: Arc<std::sync::Mutex<Vec<String>>>) -> maestro_agent::StreamFn {
    Arc::new(move |_, context, _, _| {
        let tools = context.tools.unwrap();
        assert_eq!(tools[1].description, "unchanged");
        let index = {
            let mut seen = seen.lock().unwrap();
            seen.push(tools[0].description.clone());
            seen.len()
        };
        Box::pin(async move {
            let stream = maestro_models::AssistantMessageEventStream::new();
            stream.end(Some(response(index == 1)));
            Ok(stream)
        })
    })
}

/// Mutate external description state during the first credential lookup.
fn changing_key(
    keys: &std::sync::atomic::AtomicUsize,
    current: &std::sync::Mutex<String>,
    after_key: &str,
) -> maestro_models::BoxFuture<Result<Option<String>, DiagnosticErrorInfo>> {
    use std::sync::atomic::Ordering;
    if keys.fetch_add(1, Ordering::SeqCst) == 0 {
        *current.lock().unwrap() = after_key.into();
    }
    Box::pin(async { Ok(None) })
}
