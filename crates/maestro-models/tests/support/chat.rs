#![allow(dead_code)]
use maestro_models::*;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};
pub fn dialect() -> ChatDialect {
    ChatDialect {
        store: false,
        developer_role: false,
        usage_in_stream: false,
        output_field: ChatOutputField::MaxTokens,
        tool_result_name: false,
        assistant_after_tool_result: false,
        thinking_as_text: false,
        empty_reasoning_content: false,
        thinking_format: None,
        provider_routing: None,
        provider_options: None,
        tool_stream: false,
        strict_tools: false,
        cache_control: None,
        prompt_cache_key: false,
        session_affinity_headers: false,
        long_cache_retention: false,
        truncate_plain_call_ids: false,
        auth_header: "authorization".into(),
        auth_prefix: "Bearer ".into(),
    }
}
pub struct Body(pub VecDeque<Result<Vec<u8>, Failure>>);
impl ChatHttpBody for Body {
    fn next(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, Failure>> + Send + '_>> {
        Box::pin(async move { self.0.pop_front().transpose() })
    }
}
#[derive(Default)]
pub struct Transport {
    pub requests: Mutex<Vec<ChatHttpRequest>>,
    pub timeouts: Mutex<Vec<u64>>,
    pub responses: Mutex<VecDeque<Result<ChatHttpResponse, Failure>>>,
    pub delays: Mutex<Vec<Duration>>,
    pub now: u64,
    pub jitter: f64,
    pub send_gate: Option<super::conformance::Gate>,
    pub wait_gate: Option<super::conformance::Gate>,
}
impl ChatTransport for Transport {
    fn send(
        &self,
        r: ChatHttpRequest,
        t: u64,
        c: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<ChatHttpResponse, Failure>> + Send + '_>> {
        Box::pin(async move {
            if let Some(g) = &self.send_gate {
                g.wait().await;
            }
            if c.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            self.requests.lock().unwrap().push(r);
            self.timeouts.lock().unwrap().push(t);
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Err(Failure::Transport))
        })
    }
    fn wait(
        &self,
        d: Duration,
        c: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<(), Failure>> + Send + '_>> {
        Box::pin(async move {
            self.delays.lock().unwrap().push(d);
            if let Some(g) = &self.wait_gate {
                g.wait().await;
            }
            if c.is_cancelled() {
                Err(Failure::Cancelled)
            } else {
                Ok(())
            }
        })
    }
    fn now_unix_millis(&self) -> u64 {
        self.now
    }
    fn jitter(&self) -> f64 {
        self.jitter
    }
}
pub fn response(status: u16, chunks: Vec<Result<Vec<u8>, Failure>>) -> ChatHttpResponse {
    ChatHttpResponse {
        status,
        headers: BTreeMap::new(),
        body: Box::new(Body(chunks.into())),
    }
}
pub fn frames(values: Vec<Value>) -> Vec<u8> {
    values
        .into_iter()
        .map(|v| format!("data: {v}\n\n"))
        .collect::<String>()
        .into_bytes()
}
pub fn success() -> Vec<u8> {
    frames(vec![
        json!({"choices":[{"delta":{"content":"hello"},"finish_reason":"stop"}]}),
    ])
}
pub fn fixture(bytes: Vec<u8>, d: ChatDialect) -> (Models, Model, Arc<Transport>) {
    let t = Arc::new(Transport::default());
    t.responses
        .lock()
        .unwrap()
        .push_back(Ok(response(200, vec![Ok(bytes)])));
    let m = model();
    let mut models = Models::new(Arc::new(|| 73));
    models
        .register(m.clone(), Arc::new(ChatConnection::new(t.clone(), d)))
        .unwrap();
    (models, m, t)
}
pub fn model() -> Model {
    Model::custom(
        ModelIdentity {
            provider: "local".into(),
            model: "test".into(),
            operation: "chat".into(),
        },
        "chat-completions".into(),
        "http://localhost/v1".into(),
    )
}
pub fn run(bytes: Vec<u8>) -> (Vec<ModelEvent>, Arc<Transport>) {
    let (models, m, t) = fixture(bytes, dialect());
    (
        super::conformance::collect(
            models.stream(
                m.clone(),
                super::conformance::context(),
                super::auth::local(),
            ),
            &m,
            73,
        ),
        t,
    )
}
pub fn payload(t: &Transport) -> Value {
    serde_json::from_slice(&t.requests.lock().unwrap()[0].body).unwrap()
}

pub struct GateBody(
    pub super::conformance::Gate,
    pub VecDeque<Result<Vec<u8>, Failure>>,
);
impl ChatHttpBody for GateBody {
    fn next(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, Failure>> + Send + '_>> {
        Box::pin(async move {
            self.0.wait().await;
            self.1.pop_front().transpose()
        })
    }
}
