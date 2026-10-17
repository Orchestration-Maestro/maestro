//! Owned socket transcript and recorded operation replay.
#![cfg(test)]
use maestro_app::browser::utils::model_discovery::{
    DiscoveryOptions, DiscoveryProvider, discover_lm_studio_models, discover_models,
};
use maestro_app::{Model, WebSocket, WebSocketConnection, WebSocketFuture, WebSocketRequest};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex},
};

/// One recorded operation, stripped of recorder-only state.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Case {
    /// Operation label used to select this witness.
    pub name: String,
    /// Authored invocation base.
    pub base: String,
    /// Optional API key.
    pub key: Option<String>,
    /// Whether the generic dispatcher was used.
    pub dispatch: bool,
    /// Ordered server messages, including raw malformed samples.
    pub replies: Vec<Value>,
    /// Recorded final outcome.
    pub expected: Expected,
    /// Recorded sign observations for the raw signed-zero frame.
    pub negative_zero: Option<Signs>,
}
/// Independently recorded negative-zero signs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Signs {
    /// Context sign.
    pub context_window: bool,
    /// Output limit sign.
    pub max_tokens: bool,
}
/// Recorded descriptor or error outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// Whether discovery returned descriptors.
    pub success: bool,
    /// Complete descriptor values on success.
    pub models: Option<Vec<Model>>,
    /// Error observation on failure.
    pub error: Option<Failure>,
}
/// Recorded error boundary without engine-specific prose.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    /// Observable message with an optional native diagnostic marker.
    pub message: Option<String>,
    /// Corrected typed failure category.
    pub kind: Option<String>,
    /// Corrected wrapper prefix.
    pub prefix: Option<String>,
}
/// Parse a consumed fixture group.
pub fn group<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let mut root: serde_json::Map<String, Value> =
        serde_json::from_str(include_str!("../fixtures/lm_studio_discovery.json")).unwrap();
    serde_json::from_value(root.shift_remove(name).unwrap()).unwrap()
}
/// Select one recorded public operation.
pub fn case(name: &str) -> Case {
    group::<Vec<Case>>("observations")
        .into_iter()
        .find(|case| case.name == name)
        .unwrap()
}
/// Drive a native asynchronous witness.
pub fn run(future: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future);
}
/// Shared observations, with no caller code invoked under its lock.
#[derive(Default)]
pub struct Trace {
    /// Open request.
    pub request: Option<WebSocketRequest>,
    /// Complete sent frames in order.
    pub sent: Vec<Vec<u8>>,
    /// Number of best-effort releases.
    pub closes: usize,
    /// Number of released owners.
    pub drops: usize,
    /// Number of reads performed.
    pub reads: usize,
    /// Completed send and receive boundaries.
    pub phases: Vec<&'static str>,
    /// Optional release signal withholding the authentication response.
    pub authentication: Option<tokio::sync::oneshot::Receiver<()>>,
}
/// Controlled owned connection.
struct Socket {
    /// Remaining complete replies.
    replies: VecDeque<Vec<u8>>,
    /// Observable effects.
    trace: Arc<Mutex<Trace>>,
}
impl WebSocketConnection for Socket {
    fn send(&mut self, bytes: Vec<u8>) -> WebSocketFuture<'_, io::Result<()>> {
        let mut trace = self.trace.lock().unwrap();
        trace.sent.push(bytes);
        trace.phases.push("send");
        Box::pin(async { Ok(()) })
    }
    fn receive(&mut self) -> WebSocketFuture<'_, io::Result<Option<Vec<u8>>>> {
        let authentication = {
            let mut trace = self.trace.lock().unwrap();
            trace.reads += 1;
            trace.authentication.take()
        };
        Box::pin(async move {
            if let Some(authentication) = authentication {
                authentication.await.unwrap();
            }
            self.trace.lock().unwrap().phases.push("receive");
            Ok(self.replies.pop_front())
        })
    }
    fn close(&mut self) {
        self.trace.lock().unwrap().closes += 1;
    }
}
impl Drop for Socket {
    fn drop(&mut self) {
        self.trace.lock().unwrap().drops += 1;
    }
}
/// Convert recorded raw protocol text or JSON to a complete frame.
pub fn bytes(value: Value) -> Vec<u8> {
    match value {
        Value::String(raw) => raw.into_bytes(),
        value => value.to_string().into_bytes(),
    }
}
/// Supply only sockets; any HTTP operation or extra diagnostic is a failure.
pub fn options(replies: Vec<Value>) -> (DiscoveryOptions, Arc<Mutex<Trace>>) {
    let trace = Arc::new(Mutex::new(Trace::default()));
    let observed = Arc::clone(&trace);
    let replies: Vec<Vec<u8>> = replies.into_iter().map(bytes).collect();
    let options = DiscoveryOptions {
        fetch: Arc::new(|_| panic!("unexpected HTTP operation")),
        log: Arc::new(|_| panic!("unexpected diagnostic")),
        connect_web_socket: Arc::new(move |request| {
            observed.lock().unwrap().request = Some(request);
            let socket = Socket {
                replies: replies.clone().into(),
                trace: Arc::clone(&observed),
            };
            Box::pin(async move { Ok(Box::new(socket) as WebSocket) })
        }),
    };
    (options, trace)
}
/// Replay one operation and compare its complete output to the recording.
pub async fn replay(name: &str) -> Arc<Mutex<Trace>> {
    let mut case = case(name);
    case.base = case.base.replace("$PORT", "12345");
    if let Some(models) = &mut case.expected.models {
        for model in models {
            model.base_url = model.base_url.replace("$PORT", "12345");
        }
    }
    let (options, trace) = options(case.replies);
    let result = if case.dispatch {
        discover_models(
            DiscoveryProvider::Lmstudio,
            &case.base,
            case.key.as_deref(),
            &options,
        )
        .await
    } else {
        discover_lm_studio_models(&case.base, case.key.as_deref(), &options).await
    };
    assert_eq!(result.is_ok(), case.expected.success, "operation {name}");
    if let Some(models) = case.expected.models {
        let actual = result.unwrap();
        assert_eq!(actual, models, "operation {name}");
        if let Some(signs) = case.negative_zero {
            assert_eq!(
                actual[0].context_window.is_sign_negative(),
                signs.context_window
            );
            assert_eq!(actual[0].max_tokens.is_sign_negative(), signs.max_tokens);
        }
    } else {
        assert_failure(&result.err().unwrap(), case.expected.error.unwrap());
    }
    trace
}
/// Compare a recorded failure at its observable boundary.
fn assert_failure(
    error: &maestro_app::browser::utils::model_discovery::DiscoveryError,
    failure: Failure,
) {
    if let Some(kind) = failure.kind {
        assert_eq!(kind, "InvalidResponse");
        assert!(matches!(
            error.kind,
            maestro_app::browser::utils::model_discovery::DiscoveryErrorKind::InvalidResponse
        ));
    }
    if let Some(prefix) = failure.prefix {
        assert!(error.to_string().starts_with(&prefix));
    }
    if let Some(message) = failure.message {
        if message.contains("Failed to authenticate:")
            || message.contains("Failed to parse authentication result:")
        {
            assert!(matches!(
                error.kind,
                maestro_app::browser::utils::model_discovery::DiscoveryErrorKind::Authentication(_)
            ));
            assert!(std::error::Error::source(error).is_some());
        }
        if let Some((prefix, _)) = message.split_once("<native diagnostic>") {
            assert!(error.to_string().starts_with(prefix));
        } else {
            assert_eq!(error.to_string(), message);
        }
    }
}
/// A correlated empty listing following successful authentication.
pub fn empty() -> Vec<Value> {
    vec![
        json!({"success":true}),
        json!({"type":"rpcResult","callId":0,"result":[]}),
    ]
}

mod native;
pub use native::{connection_failures, failure_phase, native};

/// Downloaded schema observation used through the public operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaCase {
    /// Server record.
    input: Value,
    /// Recorded admission.
    success: bool,
    /// Complete projected descriptors.
    models: Option<Vec<Model>>,
}
/// Drive selected downloaded records through the complete public RPC decoder.
pub async fn schema_records(select: impl Fn(&Value) -> bool) {
    for (index, case) in group::<Vec<SchemaCase>>("schema")
        .into_iter()
        .enumerate()
        .filter(|(_, case)| select(&case.input))
    {
        let (options, _) = options(vec![
            json!({"success":true}),
            json!({"type":"rpcResult","callId":0,"result":[case.input]}),
        ]);
        let result = discover_lm_studio_models("http://fixture.invalid/base", None, &options).await;
        assert_eq!(result.is_ok(), case.success, "schema {index}");
        if let Some(models) = case.models {
            assert_eq!(result.unwrap(), models);
        }
    }
}
