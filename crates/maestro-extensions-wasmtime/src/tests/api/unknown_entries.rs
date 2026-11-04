//! The model-stream import keeps the guest's request data: entries the records do not
//! know stay out, and a chosen simple protocol runs while the model keeps its own API.
use super::api;
use crate::local_resources::{Admission, Pump};
use crate::model_calls::Controls;
use maestro_extensions::{AssistantMessageEvent, Local, create_extension_runtime};
use maestro_models::{
    ApiProvider, AssistantMessageEventStream, DiagnosticErrorInfo, DoneReason,
    FauxAssistantMessageOptions, Invoke, ProviderCall, ProviderExecution, StreamOptionsInput,
    faux_assistant_message, register_api_provider, unregister_api_providers,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, RwLock};

/// The protocol of the recording provider, kept apart from the registered providers.
const API: &str = "component-unknown-entries";

/// The protocol a guest selects for one request, distinct from the model's own API.
const CHOSEN: &str = "component-selected-backend";

/// Consuming adapter recording the conversation the provider actually received.
#[derive(Default)]
struct Recording(Mutex<Vec<Value>>);

impl<E: ProviderExecution, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>>
    for Recording
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;

    /// Record the call, then answer with an ordinary completed stream.
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        self.0
            .lock()
            .unwrap()
            .push(json!({"api": call.model.api, "context": call.context}));
        let stream = AssistantMessageEventStream::new();
        stream.push(AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message: Arc::new(RwLock::new(faux_assistant_message(
                "native result",
                FauxAssistantMessageOptions::default(),
            ))),
        });
        Ok(stream)
    }
}

/// A model record on the supplied protocol identity.
fn model_data(api: &str) -> Value {
    json!({
        "id": "skip-model",
        "name": "Skip",
        "api": api,
        "provider": "custom",
        "baseUrl": "https://example.invalid",
        "reasoning": false,
        "input": ["text"],
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
        "contextWindow": 1000,
        "maxTokens": 10
    })
}

/// The guest's request text: a model on the recording protocol, a conversation holding an
/// unknown role and an unknown block, and absent options.
fn request_data() -> String {
    let model = model_data(API);
    let context = json!({"messages": [
        {"role": "other", "content": "hidden", "timestamp": 1},
        {
            "role": "user",
            "content": [
                {"type": "text", "text": "kept"},
                {"type": "unknown", "text": "hidden"}
            ],
            "timestamp": 1
        }
    ]});
    json!([model, context, null, null]).to_string()
}

/// Invocation without a signal, payload selector, response observer or opaque values.
fn no_controls() -> Controls {
    Controls {
        signal: None,
        payload: None,
        response: None,
        objects: None,
    }
}

/// Start one request from a worker thread and drain the local owner.
async fn run_request(
    pump: Pump,
    admission: Admission,
    endpoint: crate::api_reads::Endpoint,
    simple: bool,
    data: String,
) {
    let worker = std::thread::spawn(move || {
        endpoint
            .model_stream(simple, data, no_controls())
            .map(|_| ())
    });
    drop(admission);
    pump.serve().await;
    worker.join().unwrap().unwrap();
}

#[test]
fn model_stream_import_leaves_out_unknown_entries() {
    let recording = Arc::new(Recording::default());
    register_api_provider(
        ApiProvider {
            api: API.into(),
            stream: recording.clone(),
            stream_simple: recording.clone(),
        },
        Some(API.into()),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        let session = create_extension_runtime::<Local>();
        let (_owner, api) = api(session);
        let (pump, admission) = Pump::new(crate::tests::support::widgets());
        let endpoint = pump.registry().api(api).unwrap();
        run_request(pump, admission, endpoint, false, request_data()).await;
    });
    let observed = recording.0.lock().unwrap();
    assert_eq!(observed.len(), 1);
    let messages = observed[0]["context"]["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(
        messages[0]["content"],
        json!([{"type": "text", "text": "kept"}])
    );
    drop(observed);
    unregister_api_providers(API);
}

/// The chosen simple protocol runs while callers and the provider see the model's API.
#[test]
fn model_stream_import_runs_the_chosen_simple_protocol() {
    let recording = Arc::new(Recording::default());
    register_api_provider(
        ApiProvider {
            api: CHOSEN.into(),
            stream: recording.clone(),
            stream_simple: recording.clone(),
        },
        Some(CHOSEN.into()),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        let session = create_extension_runtime::<Local>();
        let (_owner, api) = api(session);
        let (pump, admission) = Pump::new(crate::tests::support::widgets());
        let endpoint = pump.registry().api(api).unwrap();
        let data = json!([model_data("public-api"), {"messages": []}, null, CHOSEN]).to_string();
        run_request(pump, admission, endpoint, true, data).await;
    });
    let observed = recording.0.lock().unwrap();
    assert_eq!(observed.len(), 1);
    assert_eq!(observed[0]["api"], "public-api");
    drop(observed);
    unregister_api_providers(CHOSEN);
}
