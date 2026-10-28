//! The shipped custom provider through the production component and SDK stream route.
use maestro_app::{
    AgentSession, Context, Local, Model, ProviderStreamOptions, SimpleStreamOptions,
};
use serde_json::{Value, json};
use std::rc::Rc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Headers and decoded JSON of one actual loopback request.
type CapturedRequest = (String, Value);

/// A disposable async endpoint, cancelled on teardown even when an assertion fails.
struct Server {
    /// Supplied model endpoint, without a protocol path suffix.
    base: String,
    /// Requests answered by this endpoint, in order.
    worker: Option<tokio::task::JoinHandle<Vec<CapturedRequest>>>,
}
impl Server {
    /// Binds before starting requests and serves only the supplied responses.
    async fn new(responses: Vec<(u16, String)>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let worker = tokio::task::spawn_local(async move {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                requests.push(read_request(&mut socket).await);
                let response = format!(
                    "HTTP/1.1 {status} Controlled\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });
        Self {
            base,
            worker: Some(worker),
        }
    }
    /// Waits for actual request completion, not a timer.
    async fn requests(mut self) -> Vec<CapturedRequest> {
        self.worker.take().unwrap().await.unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(worker) = &self.worker {
            worker.abort();
        }
    }
}

/// Reads exactly the host's content-length-framed JSON without assuming TCP chunks.
async fn read_request(socket: &mut tokio::net::TcpStream) -> CapturedRequest {
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        head.push(socket.read_u8().await.unwrap());
    }
    let head = String::from_utf8(head).unwrap();
    let length = head
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|value| value.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    let mut body = vec![0; length];
    socket.read_exact(&mut body).await.unwrap();
    (head, serde_json::from_slice(&body).unwrap())
}

/// A complete controlled message, with no live service or paid request.
fn text_response() -> String {
    let events = [
        json!({"type":"message_start","message":{"usage":{"input_tokens":1,"output_tokens":0}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"controlled response"}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}),
        json!({"type":"message_stop"}),
    ];
    let mut response = String::new();
    for event in events {
        response.push_str("data: ");
        response.push_str(&event.to_string());
        response.push_str("\n\n");
    }
    response
}

/// Runs the compiled example on the existing isolated SDK session fixture.
fn run(body: impl AsyncFnOnce(Rc<AgentSession>)) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let root = super::support::Root::new();
        let session = super::support::session::component_session(
            &root,
            local.clone(),
            std::env::var("MAESTRO_CUSTOM_MESSAGE_PROVIDER_COMPONENT")
                .expect("nextest author-component setup"),
            Some(maestro_app::NoTools::All),
            super::support::widgets(),
        )
        .await;
        body(session.clone()).await;
        session.dispose();
    });
}

/// Both authored model definitions are visible through the session's real catalog.
fn check_models(session: &AgentSession) {
    for (id, name, cost) in [
        (
            "claude-opus-4-5",
            "Claude Opus 4.5 (Custom)",
            json!({"input":5.0,"output":25.0,"cacheRead":0.5,"cacheWrite":6.25}),
        ),
        (
            "claude-sonnet-4-5",
            "Claude Sonnet 4.5 (Custom)",
            json!({"input":3.0,"output":15.0,"cacheRead":0.3,"cacheWrite":3.75}),
        ),
    ] {
        let model = session
            .model_registry()
            .find("custom-anthropic", id)
            .expect("registered custom model");
        let value = serde_json::to_value(&*model.read().unwrap()).unwrap();
        assert_eq!(value["name"], name);
        assert_eq!(value["api"], "custom-anthropic-api");
        assert_eq!(value["baseUrl"], "https://api.anthropic.com");
        assert_eq!(value["reasoning"], true);
        assert_eq!(value["input"], json!(["text", "image"]));
        assert_eq!(value["cost"], cost);
        assert_eq!(value["contextWindow"], json!(200_000.0));
        assert_eq!(value["maxTokens"], json!(64_000.0));
    }
}

/// The same registration installs the subscription callbacks without logging in.
fn check_oauth(session: &AgentSession) {
    let provider = session
        .model_registry()
        .auth_storage()
        .get_oauth_providers()
        .into_iter()
        .find(|provider| provider.id() == "custom-anthropic")
        .expect("registered subscription sign-in");
    assert_eq!(provider.name(), "Custom Anthropic (Claude Pro/Max)");
    let credentials = serde_json::from_value(
        json!({"access":"component-access","refresh":"component-refresh","expires":0}),
    )
    .unwrap();
    assert_eq!(
        provider.get_api_key(&credentials).unwrap(),
        "component-access"
    );
}

/// Supplies credentials and changes only the descriptor's test transport endpoint.
fn model(session: &AgentSession, base: &str) -> Model {
    session
        .model_registry()
        .auth_storage()
        .set_runtime_api_key("custom-anthropic", "controlled-key");
    let shared = session
        .model_registry()
        .find("custom-anthropic", "claude-sonnet-4-5")
        .unwrap();
    let mut model = shared.read().unwrap().clone();
    model.base_url = base.into();
    model
}

#[test]
fn custom_message_provider_registers_models_and_streams_with_api_key() {
    run(async |session| {
        check_models(&session);
        check_oauth(&session);
        let server = Server::new(vec![(200, text_response())]).await;
        let model = model(&session, &server.base);
        let stream_fn = session.agent().stream_fn.read().unwrap().clone().unwrap();
        let context: Context = serde_json::from_value(
            json!({"messages":[{"role":"user","content":"controlled prompt","timestamp":0}]}),
        )
        .unwrap();
        let stream = stream_fn(
            model,
            context,
            SimpleStreamOptions::<Local>::default(),
            ProviderStreamOptions::<Local>::default(),
        )
        .await
        .unwrap();
        let result = stream.result().await;
        let result = serde_json::to_value(&*result.read().unwrap()).unwrap();
        assert_eq!(result["stopReason"], "stop", "{result}");
        assert_eq!(
            result["content"],
            json!([{"type":"text","text":"controlled response"}])
        );
        let requests = server.requests().await;
        assert_eq!(requests.len(), 1);
        assert!(requests[0].0.starts_with("POST /v1/messages "));
        assert!(
            requests[0]
                .0
                .to_ascii_lowercase()
                .contains("\r\nx-api-key: controlled-key\r\n")
        );
        assert_eq!(requests[0].1["model"], "claude-sonnet-4-5");
        assert_eq!(
            requests[0].1["messages"],
            json!([{"role":"user","content":"controlled prompt"}])
        );
    });
}

/// The session, not the authored transport, retries transient provider errors.
async fn session_retries(session: &Rc<AgentSession>, status: u16) {
    let server = Server::new(vec![
        (
            status,
            json!({"error":{"message":"controlled failure"}}).to_string(),
        ),
        (200, text_response()),
    ])
    .await;
    let descriptor = model(session, &server.base);
    session
        .set_model(std::sync::Arc::new(std::sync::RwLock::new(descriptor)))
        .await
        .unwrap();
    session.settings_manager().borrow_mut().apply_overrides(
        serde_json::from_value(json!({
            "retry":{"enabled":true,"maxRetries":1,"baseDelayMs":0}
        }))
        .unwrap(),
    );
    let events = Rc::new(std::cell::RefCell::new(Vec::new()));
    let captured = Rc::clone(&events);
    let unsubscribe = session.subscribe(Rc::new(move |event| {
        if matches!(
            event,
            maestro_app::AgentSessionEvent::AutoRetryStart { .. }
                | maestro_app::AgentSessionEvent::AutoRetryEnd { .. }
        ) {
            captured
                .borrow_mut()
                .push(serde_json::to_value(event).unwrap());
        }
    }));
    session
        .prompt("controlled retry", maestro_app::PromptOptions::default())
        .await
        .unwrap();
    unsubscribe();
    let requests = server.requests().await;
    let events = events.borrow();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].1["messages"], requests[1].1["messages"]);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["type"], "auto_retry_start");
    assert!(
        events[0]["errorMessage"]
            .as_str()
            .unwrap()
            .contains(&format!("HTTP {status}")),
        "{events:?}"
    );
    assert_eq!(events[0]["attempt"], 1);
    assert_eq!(events[1]["type"], "auto_retry_end");
    assert_eq!(events[1]["success"], true);
}

#[test]
fn custom_message_provider_session_retries_rate_limits() {
    run(async |session| session_retries(&session, 429).await);
}

#[test]
fn custom_message_provider_session_retries_server_errors() {
    run(async |session| session_retries(&session, 500).await);
}
