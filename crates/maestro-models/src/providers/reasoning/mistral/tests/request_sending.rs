//! Controlled transports and callback phases.

#[cfg(not(target_arch = "wasm32"))]
use super::super::request;
#[cfg(not(target_arch = "wasm32"))]
use serde_json::Value;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[cfg(not(target_arch = "wasm32"))]
/// Execute cases against a transport whose producer finishes before inspection.
pub(super) async fn check(input: &Value, expected: &Value) {
    let (model, mut options) = configuration(input);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let hooks = Arc::new(AtomicUsize::new(0));
    let responses = Arc::new(AtomicUsize::new(0));
    let signal = input["signal"]
        .as_bool()
        .unwrap_or(false)
        .then(crate::Cancellation::new);
    if input["aborted"].as_bool().unwrap_or(false) {
        signal.as_ref().unwrap().abort();
    }
    options.common.signal = signal;
    nonfinite(&mut options, input);
    if input["simple"].as_bool() == Some(true) {
        options = request::simple_options(
            &model,
            &serde_json::from_value(serde_json::json!({"messages":[]})).unwrap(),
            &crate::SimpleStreamOptions {
                common: options.common,
                ..Default::default()
            },
        )
        .0;
    }
    install_hooks(
        &mut options,
        input,
        Arc::clone(&hooks),
        Arc::clone(&responses),
    );
    options.common.fetch = Some(fetch(input, Arc::clone(&requests)));
    let context = serde_json::from_value(serde_json::json!({"messages":[]})).unwrap();
    let result = request::prepare(Arc::new(model), &context, &options, None).await;
    let result = match result {
        Ok(prepared) => request::send(prepared).await.map(|_| ()),
        Err(error) => Err(error),
    };
    assert_eq!(
        hooks.load(Ordering::SeqCst),
        usize::try_from(expected["hook_calls"].as_u64().unwrap()).unwrap()
    );
    assert_eq!(
        responses.load(Ordering::SeqCst),
        usize::try_from(expected["response_calls"].as_u64().unwrap()).unwrap()
    );
    let actual = std::mem::take(&mut *requests.lock().unwrap());
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        expected["requests"],
        "{input}"
    );
    outcome(result, &expected["outcome"], input);
}

#[cfg(not(target_arch = "wasm32"))]
/// Native cause fixture.
fn error_info(message: &str) -> crate::DiagnosticErrorInfo {
    crate::DiagnosticErrorInfo {
        name: None,
        message: message.into(),
        stack: None,
        code: None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Set the host-only selected numeric value.
fn nonfinite(options: &mut crate::MistralOptions, input: &Value) {
    if let Some(field) = input["nonfinite"]["field"].as_str() {
        let value = match input["nonfinite"]["value"].as_str().unwrap() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            other => panic!("{other}"),
        };
        match field {
            "temperature" => options.common.temperature = Some(value),
            "maxTokens" => options.common.max_tokens = Some(value),
            other => panic!("{other}"),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Capture a completed attempt and return a controlled response.
fn fetch(input: &Value, requests: Arc<Mutex<Vec<Value>>>) -> crate::Fetch {
    let input = input.clone();
    Arc::new(move |request| {
        let observed = serde_json::json!({"url":request.url,"method":request.method,"headers":request.headers,"body":String::from_utf8(request.body).unwrap(),"aborted":request.signal.as_ref().is_some_and(crate::Cancellation::is_aborted)});
        requests.lock().unwrap().push(observed);
        let input = input.clone();
        Box::pin(async move {
            if let Some(message) = input["connectionFailure"].as_str() {
                return Err(crate::FetchError::Connection(error_info(message)));
            }
            let bytes = body_bytes(&input);
            let body = if input["bodyFailure"].as_bool() == Some(true) {
                Err(crate::FetchError::Connection(error_info(
                    "controlled body failure",
                )))
            } else {
                Ok(bytes)
            };
            Ok(crate::HttpResponse {
                status: input["status"]
                    .as_u64()
                    .map_or(200, |v| u16::try_from(v).unwrap()),
                status_text: String::new(),
                headers: response_headers(&input),
                body: Box::pin(futures_util::stream::iter([body])),
            })
        })
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Check authored text or a nonempty native boundary failure.
fn outcome(
    result: Result<(), crate::providers::http::RequestFailure>,
    expected: &Value,
    input: &Value,
) {
    if expected["admitted"].as_bool() == Some(true) {
        assert!(result.is_ok(), "{result:?}");
        return;
    }
    let error = result.unwrap_err().message;
    if let Some(text) = expected["error"].as_str() {
        assert_eq!(error, text, "{input}");
    } else if let Some(prefix) = expected["error_prefix"].as_str() {
        assert!(error.starts_with(prefix), "{error}");
        assert_eq!(expected["native_cause"], true);
        assert!(error.len() > prefix.len());
    } else {
        assert_eq!(expected["native_error"], true);
        assert!(!error.is_empty());
        assert!(!error.starts_with("Mistral API error"), "{error}");
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Install callbacks with independent counters and no owner capture.
fn install_hooks(
    options: &mut crate::MistralOptions,
    input: &Value,
    hooks: Arc<AtomicUsize>,
    responses: Arc<AtomicUsize>,
) {
    let selected = input.clone();
    let seen = hooks;
    options.common.on_payload = Some(Arc::new(
        move |crate::PayloadHookInput {
                  payload,
                  signal: hook_signal,
              },
              _| {
            seen.fetch_add(1, Ordering::SeqCst);
            let selected = selected.clone();
            Box::pin(async move {
                if let Some(error) = selected["hookError"].as_str() {
                    return Err(error_info(error));
                }
                if selected["repairNonfinite"].as_bool().unwrap_or(false) {
                    return Ok(crate::PayloadHookInput {
                        payload: serde_json::json!({"model":"m","messages":[],"temperature":0,"maxTokens":1}),
                        signal: hook_signal,
                    });
                }
                Ok(crate::PayloadHookInput {
                    payload: selected.get("replace").cloned().unwrap_or(payload),
                    signal: hook_signal,
                })
            })
        },
    ));
    let seen = responses;
    options.common.on_response = Some(Arc::new(move |_, _| {
        seen.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }));
}

#[cfg(not(target_arch = "wasm32"))]
/// Decode only the controlled body producer's fixture operands.
fn body_bytes(input: &Value) -> Vec<u8> {
    input.get("bodyBytes").map_or_else(
        || {
            input
                .get("body")
                .map_or("data: [DONE]\n\n", |v| v.as_str().unwrap_or(""))
                .as_bytes()
                .to_vec()
        },
        |v| serde_json::from_value(v.clone()).unwrap(),
    )
}

#[cfg(not(target_arch = "wasm32"))]
/// The oracle's default media type depends on its producer path.
fn response_headers(input: &Value) -> std::collections::BTreeMap<String, String> {
    let default = if input["bodyFailure"].as_bool() == Some(true) {
        "text/plain"
    } else {
        "text/event-stream"
    };
    input.get("headers").map_or_else(
        || std::collections::BTreeMap::from([("content-type".into(), default.into())]),
        |v| serde_json::from_value(v.clone()).unwrap(),
    )
}

#[cfg(not(target_arch = "wasm32"))]
/// Construct the typed descriptor and credential operand for an effect query.
fn configuration(input: &Value) -> (crate::Model, crate::MistralOptions) {
    let mut model = super::request_corpus::model(input);
    model.id = input["model"]["id"].as_str().unwrap_or("m").into();
    model.provider = input["model"]["provider"]
        .as_str()
        .unwrap_or("oracle-missing-key")
        .into();
    model.base_url = input["model"]["baseUrl"]
        .as_str()
        .unwrap_or("https://example.test/root")
        .into();
    let mut options = super::request_corpus::options(input);
    if input["options"].get("apiKey").is_none() {
        options.common.api_key = Some("synthetic".into());
    }

    (model, options)
}

#[cfg(not(target_arch = "wasm32"))]
/// Replayed arguments reach the sent body under the source's truthy default.
#[test]
fn replayed_argument_defaults_reach_the_transport() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(argument_defaults());
}

#[cfg(not(target_arch = "wasm32"))]
/// One sent call per root, compared with the argument text the source states.
async fn argument_defaults() {
    let model = super::request_corpus::model(&serde_json::json!({}));
    for (authored, sent) in [
        (Value::Null, "{}"),
        (serde_json::json!([]), "[]"),
        (serde_json::json!(false), "{}"),
    ] {
        assert_eq!(
            sent_argument_text(&model, &authored).await,
            sent,
            "{authored}"
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// The encoded argument text of one replayed call, read from the sent body.
async fn sent_argument_text(model: &crate::Model, authored: &Value) -> String {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let fetch: crate::Fetch = Arc::new(move |request| {
        captured
            .lock()
            .unwrap()
            .push(String::from_utf8(request.body).unwrap());
        Box::pin(async {
            Ok(crate::HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: std::collections::BTreeMap::from([(
                    "content-type".into(),
                    "text/event-stream".into(),
                )]),
                body: Box::pin(futures_util::stream::empty()),
            })
        })
    });
    let options: crate::MistralOptions = crate::MistralOptions {
        common: crate::StreamOptions {
            api_key: Some("synthetic".into()),
            fetch: Some(fetch),
            ..Default::default()
        },
        ..Default::default()
    };
    let context = replay_context(model, authored);
    let outcome = match request::prepare(Arc::new(model.clone()), &context, &options, None).await {
        Ok(prepared) => request::send(prepared).await.map(|_| ()),
        Err(error) => Err(error),
    };
    assert!(outcome.is_ok(), "{outcome:?}");
    let requests = requests.lock().unwrap();
    let body: Value = serde_json::from_str(&requests[0]).unwrap();
    body["messages"][0]["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[cfg(not(target_arch = "wasm32"))]
/// History holding one answered call with the supplied argument root.
fn replay_context(model: &crate::Model, authored: &Value) -> crate::Context {
    serde_json::from_value(serde_json::json!({
        "messages": [
            {
                "role": "assistant",
                "content": [
                    {"type": "toolCall", "id": "call-a", "name": "inspect", "arguments": authored}
                ],
                "api": model.api,
                "provider": model.provider,
                "model": model.id,
                "usage": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0,
                    "totalTokens": 0,
                    "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}},
                "stopReason": "toolUse",
                "timestamp": 2
            },
            {
                "role": "toolResult",
                "toolCallId": "call-a",
                "toolName": "inspect",
                "content": [{"type": "text", "text": "ok"}],
                "isError": false,
                "timestamp": 3
            }
        ]
    }))
    .unwrap()
}
