//! Cloud conversation registration through generic dispatch and replaceable callbacks.
use crate::bedrock;
use crate::bedrock_cases as cases;
use crate::bundled;
use crate::chat;
use crate::child_process;
use crate::transport;
use bedrock::diagnostic as error;
use chat::{TestResult, block_on};
use maestro_models::{
    ProviderStreamOptions, SimpleStreamOptions, StreamOptions, stream, stream_simple,
};
use serde_json::{Value, json};

const ENDPOINT: &str = "https://bedrock-runtime.us-east-1.amazonaws.com";

#[test]
fn bedrock_generic_routes_reach_the_default_pair() -> TestResult {
    let _registry = bundled::registry();
    let _restore = Restore;
    if child_process::child_case().is_none() {
        return child_process::rerun(
            "bedrock_generic_routes_reach_the_default_pair",
            "routes",
            &[("AWS_BEARER_TOKEN_BEDROCK", "fixture-token")],
        );
    }
    block_on(false, async {
        maestro_models::set_bedrock_provider_module(pair("replacement"));
        maestro_models::set_bedrock_provider_module(
            maestro_models::bedrock_provider::bedrock_provider_module(),
        );
        bedrock::corpus(include_str!("fixtures/cloud_conversation/routes.json")).await?;
        explicit_routes().await
    })
}

/// Raw, completion, factory and absent-options calls each reach the default adapter.
async fn explicit_routes() -> TestResult {
    let scripted = bedrock::serve(bedrock::answer("registered")?);
    let common = StreamOptions {
        fetch: Some(scripted.fetch.clone()),
        ..StreamOptions::default()
    };
    let raw = ProviderStreamOptions {
        common: common.clone(),
        ..ProviderStreamOptions::default()
    };
    let simple = SimpleStreamOptions {
        common,
        ..SimpleStreamOptions::default()
    };
    let factory = maestro_models::bedrock_provider::bedrock_provider_module();
    let streams = [
        stream(
            bedrock::model(ENDPOINT)?,
            bedrock::context("Hello")?,
            Some(raw.clone()),
        )?,
        (factory.stream_bedrock)(
            bedrock::model(ENDPOINT)?,
            bedrock::context("Hello")?,
            Some(raw.clone()),
        )?,
        (factory.stream_simple_bedrock)(
            bedrock::model(ENDPOINT)?,
            bedrock::context("Hello")?,
            Some(simple.clone()),
        )?,
    ];
    for call in streams {
        let (labels, message) = transport::drain(&call).await?;
        assert_eq!(
            labels,
            ["start", "text_start", "text_delta", "text_end", "done"]
        );
        assert_eq!(
            serde_json::to_value(message)?["content"][0]["text"],
            "registered"
        );
    }
    let completions = [
        maestro_models::complete(
            bedrock::model(ENDPOINT)?,
            bedrock::context("Hello")?,
            Some(raw),
        ),
        maestro_models::complete_simple(
            bedrock::model(ENDPOINT)?,
            bedrock::context("Hello")?,
            Some(simple),
        ),
    ];
    for completion in completions {
        assert_eq!(bundled::text_of(&completion.await?), "registered");
    }
    absent_options().await
}

/// Restores the default pair and bundled registry after an override test.
struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        maestro_models::set_bedrock_provider_module(
            maestro_models::bedrock_provider::bedrock_provider_module(),
        );
        maestro_models::reset_api_providers();
    }
}

#[test]
fn bedrock_override_setup_failures_settle() -> TestResult {
    let _registry = bundled::registry();
    let _restore = Restore;
    maestro_models::set_bedrock_provider_module(maestro_models::BedrockProviderModule {
        stream_bedrock: std::sync::Arc::new(|_, _, _| Err(error("raw setup"))),
        stream_simple_bedrock: std::sync::Arc::new(|_, _, _| Err(error("simple setup"))),
    });
    block_on(false, async {
        let signal = maestro_models::Cancellation::new();
        signal.abort();
        for preaborted in [false, true] {
            let common = StreamOptions {
                signal: preaborted.then(|| signal.clone()),
                ..StreamOptions::default()
            };
            let raw = stream(
                bedrock::model(ENDPOINT)?,
                bedrock::context("Hello")?,
                Some(ProviderStreamOptions {
                    common: common.clone(),
                    ..ProviderStreamOptions::default()
                }),
            )?;
            let simple = stream_simple(
                bedrock::model(ENDPOINT)?,
                bedrock::context("Hello")?,
                Some(SimpleStreamOptions {
                    common,
                    ..SimpleStreamOptions::default()
                }),
            )?;
            for (stream, text) in [(raw, "raw setup"), (simple, "simple setup")] {
                assert_setup_failure(&stream, text).await?;
            }
        }
        let mut custom =
            maestro_models::get_api_provider("bedrock-converse-stream").ok_or("provider")?;
        custom.stream = std::sync::Arc::new(|_, _, _| Err(error("custom setup")));
        maestro_models::register_api_provider(custom, None);
        assert_eq!(
            stream(bedrock::model(ENDPOINT)?, bedrock::context("Hello")?, None)
                .err()
                .ok_or("expected immediate error")?
                .message,
            "custom setup"
        );
        Ok(())
    })
}

#[test]
fn bedrock_raw_options_reach_their_consumers() -> TestResult {
    let _registry = bundled::registry();
    let _restore = Restore;
    if child_process::child_case().is_none() {
        profile_options()?;
        return child_process::rerun(
            "bedrock_raw_options_reach_their_consumers",
            "raw-options",
            &[
                ("AWS_ACCESS_KEY_ID", "controlled-access"),
                ("AWS_SECRET_ACCESS_KEY", "controlled-secret"),
                ("AWS_EC2_METADATA_DISABLED", "true"),
            ],
        );
    }
    if child_process::child_case().as_deref() == Some("profile") {
        return block_on(
            false,
            bedrock::corpus(include_str!("fixtures/cloud_conversation/profile.json")),
        );
    }
    block_on(false, async {
        bedrock::corpus(include_str!("fixtures/cloud_conversation/raw-options.json")).await?;
        invalid_default_options().await
    })
}

/// A callback pair whose setup outcome identifies the selected callback.
fn pair(tag: &str) -> maestro_models::BedrockProviderModule {
    let (raw, simple) = (format!("{tag}:raw"), format!("{tag}:simple"));
    maestro_models::BedrockProviderModule {
        stream_bedrock: std::sync::Arc::new(move |_, _, _| Err(error(&raw))),
        stream_simple_bedrock: std::sync::Arc::new(move |_, _, _| Err(error(&simple))),
    }
}

/// Observe only the result of this invocation of a retained registry callback.
async fn selected(provider: &maestro_models::ApiProvider) -> TestResult<String> {
    let stream = (provider.stream)(bedrock::model(ENDPOINT)?, bedrock::context("Hello")?, None)?;
    let (_, message) = transport::drain(&stream).await?;
    Ok(message.error_message.ok_or("no callback marker")?)
}

#[test]
fn bedrock_override_snapshots_callbacks_without_holding_locks() -> TestResult {
    let _registry = bundled::registry();
    let _restore = Restore;
    block_on(false, async {
        let retained =
            maestro_models::get_api_provider("bedrock-converse-stream").ok_or("provider")?;
        let mut caller = pair("A");
        maestro_models::set_bedrock_provider_module(caller.clone());
        caller.stream_bedrock = pair("unused").stream_bedrock;
        assert_eq!(selected(&retained).await?, "A:raw");
        let pending = maestro_models::AssistantMessageEventStream::new();
        let alias = pending.clone();
        let capture = std::sync::Arc::new(String::from("running"));
        let weak = std::sync::Arc::downgrade(&capture);
        let raw = std::sync::Arc::new(move |_, _, _| {
            assert_eq!(capture.as_str(), "running");
            Ok(alias.clone())
        });
        let mut running = pair("A");
        running.stream_bedrock = raw;
        let survivor = running.clone();
        maestro_models::set_bedrock_provider_module(running);
        let invocation =
            (retained.stream)(bedrock::model(ENDPOINT)?, bedrock::context("Hello")?, None)?;
        maestro_models::set_bedrock_provider_module(pair("B"));
        assert_eq!(selected(&retained).await?, "B:raw");
        assert!(weak.upgrade().is_some());
        let survivor_stream =
            (survivor.stream_bedrock)(bedrock::model(ENDPOINT)?, bedrock::context("Hello")?, None)?;
        let message = std::sync::Arc::new(std::sync::RwLock::new(serde_json::from_value(json!({
            "role":"assistant", "content":[{"type":"text","text":"A-running"}], "api":"bedrock-converse-stream",
            "provider":"amazon-bedrock", "model":"meta.llama-controlled", "timestamp":1, "stopReason":"stop",
            "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
                "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}
        }))?));
        pending.end(Some(message.clone()));
        assert!(std::sync::Arc::ptr_eq(&invocation.result().await, &message));
        assert!(std::sync::Arc::ptr_eq(
            &survivor_stream.result().await,
            &message
        ));
        drop(survivor);
        assert!(weak.upgrade().is_none());
        let mut reentrant = pair("C");
        reentrant.stream_bedrock = std::sync::Arc::new(|_, _, _| {
            maestro_models::set_bedrock_provider_module(pair("D"));
            Err(error("C:raw"))
        });
        maestro_models::set_bedrock_provider_module(reentrant);
        assert_eq!(selected(&retained).await?, "C:raw");
        assert_eq!(selected(&retained).await?, "D:raw");
        Ok(())
    })
}

/// Registration order as returned to generic callers.
fn registered() -> Vec<String> {
    maestro_models::get_api_providers()
        .into_iter()
        .map(|provider| provider.api)
        .collect()
}

#[test]
fn bedrock_override_survives_reset_and_registry_changes() -> TestResult {
    let _registry = bundled::registry();
    let _restore = Restore;
    block_on(false, async {
        maestro_models::reset_api_providers();
        let order: Vec<String> = bundled::registered_apis()
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(registered(), order);
        let retained =
            maestro_models::get_api_provider("bedrock-converse-stream").ok_or("provider")?;
        maestro_models::set_bedrock_provider_module(pair("B"));
        let mut custom = retained.clone();
        custom.api = "custom-cloud".into();
        maestro_models::register_api_provider(custom, Some("caller".into()));
        let mut custom_order = order.clone();
        custom_order.push("custom-cloud".into());
        assert_eq!(registered(), custom_order);
        let mut replacement = retained.clone();
        replacement.stream = pair("replacement").stream_bedrock;
        maestro_models::register_api_provider(replacement, None);
        assert_eq!(registered(), custom_order);
        assert_eq!(
            stream(bedrock::model(ENDPOINT)?, bedrock::context("Hello")?, None)
                .err()
                .ok_or("custom callback error")?
                .message,
            "replacement:raw"
        );
        maestro_models::register_built_in_api_providers();
        assert_eq!(registered(), custom_order);
        assert_eq!(selected(&retained).await?, "B:raw");
        maestro_models::clear_api_providers();
        assert!(registered().is_empty());
        assert_eq!(selected(&retained).await?, "B:raw");
        maestro_models::register_built_in_api_providers();
        assert_eq!(registered(), order);
        assert_eq!(selected(&retained).await?, "B:raw");
        maestro_models::reset_api_providers();
        assert_eq!(registered(), order);
        let simple =
            maestro_models::get_api_provider("bedrock-converse-stream").ok_or("provider")?;
        let result =
            (simple.stream_simple)(bedrock::model(ENDPOINT)?, bedrock::context("Hello")?, None)?;
        assert_eq!(
            transport::drain(&result).await?.1.error_message.as_deref(),
            Some("B:simple")
        );
        Ok(())
    })
}

/// Assert that the retained effect handles reach a replacement without default decoding.
fn assert_common(
    actual: &StreamOptions,
    expected: &StreamOptions,
) -> Result<(), maestro_models::DiagnosticErrorInfo> {
    assert_eq!(actual.temperature, expected.temperature);
    assert_eq!(actual.max_tokens, expected.max_tokens);
    assert_eq!(actual.session_id, expected.session_id);
    assert!(std::sync::Arc::ptr_eq(
        actual.on_payload.as_ref().ok_or_else(|| error("payload"))?,
        expected
            .on_payload
            .as_ref()
            .ok_or_else(|| error("payload"))?
    ));
    assert!(std::sync::Arc::ptr_eq(
        actual
            .on_response
            .as_ref()
            .ok_or_else(|| error("response"))?,
        expected
            .on_response
            .as_ref()
            .ok_or_else(|| error("response"))?
    ));
    assert!(std::sync::Arc::ptr_eq(
        actual.fetch.as_ref().ok_or_else(|| error("fetch"))?,
        expected.fetch.as_ref().ok_or_else(|| error("fetch"))?
    ));
    assert!(
        !expected
            .signal
            .as_ref()
            .ok_or_else(|| error("signal"))?
            .is_aborted()
    );
    actual
        .signal
        .as_ref()
        .ok_or_else(|| error("signal"))?
        .abort();
    assert!(
        expected
            .signal
            .as_ref()
            .ok_or_else(|| error("signal"))?
            .is_aborted()
    );
    Ok(())
}

#[test]
fn bedrock_overrides_receive_open_options_unchanged() -> TestResult {
    let _registry = bundled::registry();
    let _restore = Restore;
    block_on(false, async {
        let scripted = bedrock::serve(vec![]);
        let common = open_options(&scripted);
        let simple_common = open_options(&scripted);
        let object = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let nested = json!({"region":42,"future":{"z":[2,1,2],"a":null}});
        maestro_models::set_bedrock_provider_module(maestro_models::BedrockProviderModule {
            stream_bedrock: raw_override(common.clone(), object.clone(), nested.clone()),
            stream_simple_bedrock: simple_override(simple_common.clone()),
        });
        let mut raw = ProviderStreamOptions {
            common: common.clone(),
            extra: nested.as_object().ok_or("extras")?.clone(),
            ..ProviderStreamOptions::default()
        };
        raw.objects.insert(object.clone());
        let simple = SimpleStreamOptions {
            common: simple_common,
            reasoning: Some(maestro_models::ThinkingLevel::Low),
            thinking_budgets: Some(maestro_models::ThinkingBudgets {
                low: Some(1234.0),
                high: Some(4321.0),
                ..maestro_models::ThinkingBudgets::default()
            }),
            ..SimpleStreamOptions::default()
        };
        for (options, marker) in [(None, "raw absent"), (Some(raw), "raw present")] {
            let call = stream(
                bedrock::model(ENDPOINT)?,
                bedrock::context("kept")?,
                options,
            )?;
            assert_eq!(
                transport::drain(&call).await?.1.error_message.as_deref(),
                Some(marker)
            );
        }
        for (options, marker) in [(None, "simple absent"), (Some(simple), "simple present")] {
            let call = stream_simple(
                bedrock::model(ENDPOINT)?,
                bedrock::context("kept")?,
                options,
            )?;
            assert_eq!(
                transport::drain(&call).await?.1.error_message.as_deref(),
                Some(marker)
            );
        }
        assert_eq!(object.load(std::sync::atomic::Ordering::SeqCst), 7);
        assert!(scripted.seen()?.is_empty());
        Ok(())
    })
}

/// Prove that generic profile options select real profile credentials and their endpoint.
fn profile_options() -> TestResult {
    let files = cases::AwsFiles::create_with(
        "raw-profile",
        "[profile controlled-profile]\nregion=eu-west-1\n",
        "[controlled-profile]\naws_access_key_id=profile-key\naws_secret_access_key=controlled-secret\n",
    )?;
    let case = cases::Case::new("profile", ENDPOINT, cases::Expect::Fails);
    let environment = files.environment(&case);
    let environment = environment
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    child_process::rerun(
        "bedrock_raw_options_reach_their_consumers",
        "profile",
        &environment,
    )
}

/// Default-adapter validation rejects wrong shapes before request effects, including sequence records.
async fn invalid_default_options() -> TestResult {
    let fields = [
        "region",
        "profile",
        "bearerToken",
        "toolChoice",
        "reasoning",
        "thinkingBudgets",
        "interleavedThinking",
        "thinkingDisplay",
        "requestMetadata",
    ];
    for field in fields {
        for value in [Value::Null, json!([1])] {
            let scripted = bedrock::serve(bedrock::answer("unused")?);
            let mut options = ProviderStreamOptions {
                common: StreamOptions {
                    fetch: Some(scripted.fetch.clone()),
                    ..StreamOptions::default()
                },
                ..ProviderStreamOptions::default()
            };
            options.extra.insert(field.into(), value);
            let call = stream(
                bedrock::model(ENDPOINT)?,
                bedrock::context("Hello")?,
                Some(options),
            )?;
            let (labels, message) = transport::drain(&call).await?;
            assert_eq!(labels, ["error"], "{field}");
            assert!(message.content.is_empty(), "{field}");
            assert!(scripted.seen()?.is_empty(), "{field}");
        }
    }
    Ok(())
}

/// Option-free routes select the native adapter before their local endpoint failure.
async fn absent_options() -> TestResult {
    // Without transport options, invalid endpoints fail locally after default selection.
    for simple in [false, true] {
        let model = bedrock::model("not a URL")?;
        let call = if simple {
            stream_simple(model, bedrock::context("Hello")?, None)?
        } else {
            stream(model, bedrock::context("Hello")?, None)?
        };
        let (_, message) = transport::drain(&call).await?;
        assert_eq!(message.stop_reason, maestro_models::StopReason::Error);
        assert_eq!(
            message.error_message.as_deref(),
            Some("relative URL without a base")
        );
    }
    Ok(())
}

/// A replacement setup failure preserves the supplied identity without request content or usage.
async fn assert_setup_failure(
    stream: &maestro_models::AssistantMessageEventStream,
    text: &str,
) -> TestResult {
    let (labels, message) = transport::drain(stream).await?;
    assert_eq!(labels, ["error"]);
    assert_eq!(message.api, "bedrock-converse-stream");
    assert_eq!(message.provider, "amazon-bedrock");
    assert_eq!(message.model, "meta.llama-controlled");
    assert!(message.content.is_empty());
    let usage = &message.usage;
    assert!(
        [
            usage.input,
            usage.output,
            usage.cache_read,
            usage.cache_write,
            usage.total_tokens,
            usage.cost.input,
            usage.cost.output,
            usage.cost.cache_read,
            usage.cost.cache_write,
            usage.cost.total
        ]
        .iter()
        .all(|value| *value == 0.0)
    );
    assert_eq!(message.stop_reason, maestro_models::StopReason::Error);
    assert_eq!(message.error_message.as_deref(), Some(text));
    Ok(())
}

/// Replacement raw callback that observes open values and shared object handles.
fn raw_override(
    common: StreamOptions,
    shared: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    expected: Value,
) -> maestro_models::ApiStreamFunction {
    std::sync::Arc::new(move |model, context, options| {
        assert_override_inputs(&model, &context)?;
        if let Some(options) = options {
            assert_common(&options.common, &common)?;
            assert_eq!(
                options.extra["future"]
                    .as_object()
                    .ok_or_else(|| error("nested object"))?
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                ["z", "a"]
            );
            assert_eq!(Value::Object(options.extra), expected);
            let value = options
                .objects
                .get::<std::sync::Arc<std::sync::atomic::AtomicUsize>>()
                .ok_or_else(|| error("object"))?;
            assert!(std::sync::Arc::ptr_eq(value, &shared));
            value.store(7, std::sync::atomic::Ordering::SeqCst);
            Err(error("raw present"))
        } else {
            Err(error("raw absent"))
        }
    })
}

/// Replacement simple callback observes the requested level and independent budget slots.
fn simple_override(common: StreamOptions) -> maestro_models::ApiStreamSimpleFunction {
    std::sync::Arc::new(move |model, context, options| {
        assert_override_inputs(&model, &context)?;
        if let Some(options) = options {
            assert_common(&options.common, &common)?;
            assert_eq!(options.reasoning, Some(maestro_models::ThinkingLevel::Low));
            let budgets = options.thinking_budgets.ok_or_else(|| error("budgets"))?;
            assert_eq!((budgets.low, budgets.high), (Some(1234.0), Some(4321.0)));
            Err(error("simple present"))
        } else {
            Err(error("simple absent"))
        }
    })
}

/// Check the model and context forwarded to either replacement route.
fn assert_override_inputs(
    model: &maestro_models::Model,
    context: &maestro_models::Context,
) -> Result<(), maestro_models::DiagnosticErrorInfo> {
    assert_eq!(
        model,
        &bedrock::model(ENDPOINT).map_err(|_| error("model"))?
    );
    assert_eq!(
        serde_json::to_value(context).map_err(|_| error("context"))?,
        serde_json::to_value(bedrock::context("kept").map_err(|_| error("context"))?)
            .map_err(|_| error("context"))?
    );
    Ok(())
}

/// Distinguishable common settings and effect handles supplied to replacement callbacks.
fn open_options(scripted: &transport::Transport) -> StreamOptions {
    StreamOptions {
        temperature: Some(0.25),
        max_tokens: Some(17.0),
        session_id: Some("open-session".into()),
        signal: Some(maestro_models::Cancellation::new()),
        fetch: Some(scripted.fetch.clone()),
        on_payload: Some(std::sync::Arc::new(|value, _| {
            Box::pin(std::future::ready(Ok(value)))
        })),
        on_response: Some(std::sync::Arc::new(|_, _| {
            Box::pin(std::future::ready(Ok(())))
        })),
        ..StreamOptions::default()
    }
}
